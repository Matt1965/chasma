//! Player-owned building interaction dispatch (BP4.5).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::client::commands::CommandTarget;
use crate::client::inventory_intent::{InventoryIntent, InventoryIntentQueue, InventoryOpenMode};
use crate::player::LocalPlayerOwnership;
use crate::ui::gameplay::building_panel::{
    BuildingPanelState, building_owned_by_local_player, try_open_building_menu,
};
use crate::units::input::{MoveOrdersReport, SelectedUnits, issue_move_orders_to_selection};
use crate::world::{
    AttackTargetingPolicy, BuildingCatalog, BuildingId, BuildingInteractionProfileCatalog,
    DoodadCatalog, FootprintCatalog, ItemPileSettings, NavigationConfig, UnitCatalog, UnitId,
    WeaponCatalog, WorldData, WorldPosition, building_has_inventory, is_unit_alive,
    spill_position_for_building, unit_within_building_inventory_range,
};
use crate::world::{
    InteractionQueryContext, InteractionTargetRef, InteractionType, query_world_interaction,
};

/// A unit approaching a building to complete player inventory interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingBuildingPlayerInteraction {
    pub actor_unit_id: UnitId,
    pub building_id: BuildingId,
}

/// Client-local intent for deferred building interaction UI (BP4.5 follow-up).
#[derive(Resource, Default, Debug)]
pub struct PendingBuildingPlayerInteractionState {
    pending: Option<PendingBuildingPlayerInteraction>,
}

impl PendingBuildingPlayerInteractionState {
    pub fn get(&self) -> Option<PendingBuildingPlayerInteraction> {
        self.pending
    }

    pub fn set(&mut self, actor_unit_id: UnitId, building_id: BuildingId) {
        self.pending = Some(PendingBuildingPlayerInteraction {
            actor_unit_id,
            building_id,
        });
    }

    pub fn clear(&mut self) {
        self.pending = None;
    }

    pub fn clear_for_unit(&mut self, unit_id: UnitId) {
        if self
            .pending
            .is_some_and(|pending| pending.actor_unit_id == unit_id)
        {
            self.pending = None;
        }
    }
}

/// Outcome of a player-owned building interaction attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnedBuildingInteractionOutcome {
    pub opened_menu: bool,
    pub opened_inventory: bool,
    pub issued_approach: bool,
    pub deferred_until_arrival: bool,
}

/// Cancel pending building interaction for units receiving a superseding player command.
pub fn supersede_pending_building_interaction_for_selection(
    pending: &mut PendingBuildingPlayerInteractionState,
    selection: &SelectedUnits,
) {
    for unit_id in selection.iter() {
        pending.clear_for_unit(unit_id);
    }
}

/// Resolve a player-owned building target for contextual interaction, if any.
pub fn resolve_player_owned_building_target(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    footprint_catalog: &FootprintCatalog,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    pile_settings: &ItemPileSettings,
    player: &LocalPlayerOwnership,
    target: CommandTarget,
) -> Option<BuildingId> {
    match target {
        CommandTarget::Building { building_id } => {
            player_owned_interactive_building(world, building_id, player)
        }
        CommandTarget::Terrain { position } => {
            let ctx = InteractionQueryContext::new(
                world,
                doodad_catalog,
                building_catalog,
                footprint_catalog,
                interaction_catalog,
                unit_catalog,
                weapon_catalog,
                pile_settings,
            );
            let interaction = query_world_interaction(&ctx, position)?;
            match interaction.target {
                InteractionTargetRef::Building(building_id)
                    if interaction.valid
                        && matches!(
                            interaction.interaction_type,
                            InteractionType::Workstation
                                | InteractionType::Container
                                | InteractionType::InteractableObject
                        ) =>
                {
                    player_owned_interactive_building(world, building_id, player)
                }
                _ => None,
            }
        }
        CommandTarget::Unit { .. } => None,
    }
}

fn player_owned_interactive_building(
    world: &WorldData,
    building_id: BuildingId,
    player: &LocalPlayerOwnership,
) -> Option<BuildingId> {
    let building = world.get_building(building_id)?;
    if !building_owned_by_local_player(building, player) {
        return None;
    }
    if !building_has_inventory(world, building_id) {
        return None;
    }
    Some(building_id)
}

fn pending_interaction_still_valid(
    world: &WorldData,
    player: &LocalPlayerOwnership,
    pending: PendingBuildingPlayerInteraction,
) -> bool {
    let Some(unit) = world.get_unit(pending.actor_unit_id) else {
        return false;
    };
    if !is_unit_alive(unit) || unit.inventory_id.is_none() {
        return false;
    }
    player_owned_interactive_building(world, pending.building_id, player).is_some()
}

/// Open owned building menu + actor inventory.
pub fn complete_building_player_interaction(
    building_panel: &mut BuildingPanelState,
    inventory_queue: &mut InventoryIntentQueue,
    player: &LocalPlayerOwnership,
    world: &WorldData,
    actor_unit_id: UnitId,
    building_id: BuildingId,
) -> (bool, bool) {
    let opened_menu = try_open_building_menu(building_panel, building_id, world, player);
    let opened_inventory = world
        .get_unit(actor_unit_id)
        .and_then(|unit| unit.inventory_id)
        .is_some();
    if opened_inventory {
        inventory_queue.push(InventoryIntent::Open(InventoryOpenMode::UnitOnly {
            unit_id: actor_unit_id,
        }));
    }
    (opened_menu, opened_inventory)
}

/// Open owned building menu + actor inventory; approach interaction point when out of range.
///
/// Does not assign workstation tasks or settlement work.
pub fn try_dispatch_owned_building_player_interaction(
    world: &mut WorldData,
    building_panel: &mut BuildingPanelState,
    inventory_queue: &mut InventoryIntentQueue,
    pending: &mut PendingBuildingPlayerInteractionState,
    player: &LocalPlayerOwnership,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    actor_unit_id: UnitId,
    building_id: BuildingId,
) -> Option<OwnedBuildingInteractionOutcome> {
    if player_owned_interactive_building(world, building_id, player).is_none() {
        return None;
    }

    let in_range = unit_within_building_inventory_range(
        world,
        building_catalog,
        interaction_catalog,
        actor_unit_id,
        building_id,
    );

    if in_range {
        pending.clear_for_unit(actor_unit_id);
        let (opened_menu, opened_inventory) = complete_building_player_interaction(
            building_panel,
            inventory_queue,
            player,
            world,
            actor_unit_id,
            building_id,
        );
        return Some(OwnedBuildingInteractionOutcome {
            opened_menu,
            opened_inventory,
            issued_approach: false,
            deferred_until_arrival: false,
        });
    }

    pending.set(actor_unit_id, building_id);
    let issued_approach = issue_approach_to_building_interaction(
        world,
        building_catalog,
        interaction_catalog,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        actor_unit_id,
        building_id,
    )
    .issued
        > 0;

    Some(OwnedBuildingInteractionOutcome {
        opened_menu: false,
        opened_inventory: false,
        issued_approach,
        deferred_until_arrival: true,
    })
}

fn issue_approach_to_building_interaction(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    actor_unit_id: UnitId,
    building_id: BuildingId,
) -> MoveOrdersReport {
    let Some(building) = world.get_building(building_id) else {
        return MoveOrdersReport::default();
    };
    let (target, _) =
        spill_position_for_building(world, building_catalog, interaction_catalog, building);
    let mut actor_only = SelectedUnits::default();
    actor_only.set_single(actor_unit_id);
    issue_move_orders_to_selection(
        world,
        &actor_only,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        target,
        AttackTargetingPolicy::default(),
    )
}

#[derive(SystemParam)]
pub struct PendingBuildingInteractionTickParams<'w> {
    pub pending: ResMut<'w, PendingBuildingPlayerInteractionState>,
    pub building_panel: ResMut<'w, BuildingPanelState>,
    pub inventory_queue: ResMut<'w, InventoryIntentQueue>,
    pub world: Res<'w, WorldData>,
    pub player: Res<'w, LocalPlayerOwnership>,
    pub building_catalog: Res<'w, BuildingCatalog>,
    pub interaction_catalog: Res<'w, BuildingInteractionProfileCatalog>,
}

/// Complete deferred building interaction when the actor enters authoritative range.
pub fn try_complete_pending_building_player_interaction(
    pending: &mut PendingBuildingPlayerInteractionState,
    building_panel: &mut BuildingPanelState,
    inventory_queue: &mut InventoryIntentQueue,
    world: &WorldData,
    player: &LocalPlayerOwnership,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
) -> bool {
    let Some(pending_interaction) = pending.get() else {
        return false;
    };

    if !pending_interaction_still_valid(world, player, pending_interaction) {
        pending.clear();
        return false;
    }

    if !unit_within_building_inventory_range(
        world,
        building_catalog,
        interaction_catalog,
        pending_interaction.actor_unit_id,
        pending_interaction.building_id,
    ) {
        return false;
    }

    complete_building_player_interaction(
        building_panel,
        inventory_queue,
        player,
        world,
        pending_interaction.actor_unit_id,
        pending_interaction.building_id,
    );
    pending.clear();
    true
}

/// Complete deferred building interaction when the actor enters authoritative range.
pub fn tick_pending_building_player_interactions(mut params: PendingBuildingInteractionTickParams) {
    try_complete_pending_building_player_interaction(
        &mut params.pending,
        &mut params.building_panel,
        &mut params.inventory_queue,
        &params.world,
        &params.player,
        &params.building_catalog,
        &params.interaction_catalog,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingOwnership,
        BuildingSource, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield,
        InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog,
        ItemDefinitionId, LocalPosition, OperationCatalog, UnitDefinitionId, UnitOwnership,
        UnitSource, create_building_with_inventory, create_unit_with_inventory,
        starter_building_definitions, starter_inventory_profile_definitions,
        starter_item_category_definitions, starter_item_definitions, starter_unit_definitions,
        transfer_one,
    };
    use bevy::prelude::{Quat, Vec3};

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn inventory_ctx() -> &'static InventoryCatalogCtx<'static> {
        static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
        CTX.get_or_init(|| {
            let categories =
                ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
            let items =
                ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
            let profiles =
                InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                    .unwrap();
            let items = Box::leak(Box::new(items));
            let categories = Box::leak(Box::new(categories));
            let profiles = Box::leak(Box::new(profiles));
            InventoryCatalogCtx::new(items, categories, profiles)
        })
    }

    fn player() -> LocalPlayerOwnership {
        LocalPlayerOwnership::default()
    }

    fn farm_setup(
        world: &mut WorldData,
        farm_pos: WorldPosition,
        unit_pos: WorldPosition,
    ) -> (crate::world::BuildingRecord, crate::world::UnitRecord) {
        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let ctx = inventory_ctx();
        let farm = create_building_with_inventory(
            &catalog,
            world,
            &BuildingDefinitionId::new("prispod_farm"),
            farm_pos,
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            ctx,
        )
        .unwrap();
        let unit = create_unit_with_inventory(
            &unit_catalog,
            world,
            &UnitDefinitionId::new("bandit"),
            unit_pos,
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            ctx,
        )
        .unwrap();
        (farm, unit)
    }

    #[test]
    fn owned_building_target_resolves_for_player_farm() {
        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction = BuildingInteractionProfileCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let weapon_catalog = WeaponCatalog::default();
        let pile_settings = ItemPileSettings::default();
        let mut world = flat_world();
        let farm = create_building_with_inventory(
            &catalog,
            &mut world,
            &BuildingDefinitionId::new("prispod_farm"),
            pos(10.0, 10.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            inventory_ctx(),
        )
        .unwrap();
        let resolved = resolve_player_owned_building_target(
            &world,
            &catalog,
            &interaction,
            &doodad_catalog,
            &footprint,
            &unit_catalog,
            &weapon_catalog,
            &pile_settings,
            &player(),
            CommandTarget::Building {
                building_id: farm.id,
            },
        );
        assert_eq!(resolved, Some(farm.id));
    }

    #[test]
    fn foreign_building_target_does_not_resolve() {
        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction = BuildingInteractionProfileCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let weapon_catalog = WeaponCatalog::default();
        let pile_settings = ItemPileSettings::default();
        let mut world = flat_world();
        let farm = create_building_with_inventory(
            &catalog,
            &mut world,
            &BuildingDefinitionId::new("prispod_farm"),
            pos(10.0, 10.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Hostile),
            None,
            inventory_ctx(),
        )
        .unwrap();
        assert!(
            resolve_player_owned_building_target(
                &world,
                &catalog,
                &interaction,
                &doodad_catalog,
                &footprint,
                &unit_catalog,
                &weapon_catalog,
                &pile_settings,
                &player(),
                CommandTarget::Building {
                    building_id: farm.id,
                },
            )
            .is_none()
        );
    }

    #[test]
    fn out_of_range_interaction_defers_ui_and_issues_approach_without_work_task() {
        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction = BuildingInteractionProfileCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let weapon_catalog = WeaponCatalog::default();
        let mut world = flat_world();
        let (farm, unit) = farm_setup(&mut world, pos(40.0, 40.0), pos(1.0, 1.0));
        let tasks_before = world.task_store().sorted_task_ids().len();

        let mut panel = BuildingPanelState::default();
        let mut inventory_queue = InventoryIntentQueue::default();
        let mut pending = PendingBuildingPlayerInteractionState::default();
        let outcome = try_dispatch_owned_building_player_interaction(
            &mut world,
            &mut panel,
            &mut inventory_queue,
            &mut pending,
            &player(),
            &catalog,
            &interaction,
            &unit_catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            unit.id,
            farm.id,
        )
        .expect("owned interaction");

        assert!(!outcome.opened_menu);
        assert!(!outcome.opened_inventory);
        assert!(outcome.issued_approach);
        assert!(outcome.deferred_until_arrival);
        assert!(panel.open_building_id.is_none());
        assert!(inventory_queue.is_empty());
        assert_eq!(
            pending.get(),
            Some(PendingBuildingPlayerInteraction {
                actor_unit_id: unit.id,
                building_id: farm.id,
            })
        );
        assert_eq!(
            world.task_store().sorted_task_ids().len(),
            tasks_before,
            "building interaction must not create work tasks"
        );
    }

    #[test]
    fn in_range_interaction_does_not_issue_approach() {
        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction = BuildingInteractionProfileCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let weapon_catalog = WeaponCatalog::default();
        let mut world = flat_world();
        let (farm, unit) = farm_setup(&mut world, pos(10.0, 10.0), pos(10.5, 10.5));

        let mut panel = BuildingPanelState::default();
        let mut inventory_queue = InventoryIntentQueue::default();
        let mut pending = PendingBuildingPlayerInteractionState::default();
        let outcome = try_dispatch_owned_building_player_interaction(
            &mut world,
            &mut panel,
            &mut inventory_queue,
            &mut pending,
            &player(),
            &catalog,
            &interaction,
            &unit_catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            unit.id,
            farm.id,
        )
        .expect("owned interaction");

        assert!(!outcome.issued_approach);
        assert!(!outcome.deferred_until_arrival);
        assert!(pending.get().is_none());
    }

    #[test]
    fn owned_building_interaction_opens_menu_and_unit_inventory_when_in_range() {
        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction = BuildingInteractionProfileCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let weapon_catalog = WeaponCatalog::default();
        let mut world = flat_world();
        let (farm, unit) = farm_setup(&mut world, pos(10.0, 10.0), pos(10.5, 10.5));

        let mut panel = BuildingPanelState::default();
        let mut inventory_queue = InventoryIntentQueue::default();
        let mut pending = PendingBuildingPlayerInteractionState::default();
        let outcome = try_dispatch_owned_building_player_interaction(
            &mut world,
            &mut panel,
            &mut inventory_queue,
            &mut pending,
            &player(),
            &catalog,
            &interaction,
            &unit_catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            unit.id,
            farm.id,
        )
        .expect("owned interaction");

        assert!(outcome.opened_menu);
        assert!(outcome.opened_inventory);
        assert_eq!(panel.open_building_id, Some(farm.id));
        let intents = inventory_queue.drain();
        assert_eq!(intents.len(), 1);
        assert!(matches!(
            intents[0],
            InventoryIntent::Open(InventoryOpenMode::UnitOnly { unit_id }) if unit_id == unit.id
        ));
    }

    #[test]
    fn entering_range_completes_pending_interaction_without_second_click() {
        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction = BuildingInteractionProfileCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let weapon_catalog = WeaponCatalog::default();
        let mut world = flat_world();
        let (farm, unit) = farm_setup(&mut world, pos(10.0, 10.0), pos(40.0, 40.0));

        let mut panel = BuildingPanelState::default();
        let mut inventory_queue = InventoryIntentQueue::default();
        let mut pending = PendingBuildingPlayerInteractionState::default();
        pending.set(unit.id, farm.id);

        world.mutate_unit(unit.id, |record| {
            record.placement.position = pos(10.5, 10.5);
        });

        assert!(try_complete_pending_building_player_interaction(
            &mut pending,
            &mut panel,
            &mut inventory_queue,
            &world,
            &player(),
            &catalog,
            &interaction,
        ));

        assert_eq!(panel.open_building_id, Some(farm.id));
        assert!(!inventory_queue.is_empty());
        assert!(pending.get().is_none());
    }

    #[test]
    fn superseding_move_clears_pending_interaction() {
        let mut pending = PendingBuildingPlayerInteractionState::default();
        pending.set(UnitId::new(1), BuildingId::new(2));
        let mut selection = SelectedUnits::default();
        selection.set_single(UnitId::new(1));
        supersede_pending_building_interaction_for_selection(&mut pending, &selection);
        assert!(pending.get().is_none());
    }

    #[test]
    fn removed_building_invalidates_pending_interaction() {
        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction = BuildingInteractionProfileCatalog::default();
        let mut world = flat_world();
        let (farm, unit) = farm_setup(&mut world, pos(10.0, 10.0), pos(40.0, 40.0));
        let mut pending = PendingBuildingPlayerInteractionState::default();
        pending.set(unit.id, farm.id);
        world.remove_building_by_id(farm.id);

        assert!(!pending_interaction_still_valid(
            &world,
            &player(),
            pending.get().unwrap()
        ));
    }

    #[test]
    fn out_of_range_approach_targets_canonical_spill_position() {
        use crate::world::{
            FootprintCatalog, PassabilityCatalogs, UnitState, resolve_all_pending_unit_orders,
        };

        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction = BuildingInteractionProfileCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let weapon_catalog = WeaponCatalog::default();
        let mut world = flat_world();
        let (farm, unit) = farm_setup(&mut world, pos(40.0, 40.0), pos(1.0, 1.0));
        let building = world.get_building(farm.id).unwrap();
        let (expected_target, _) =
            spill_position_for_building(&world, &catalog, &interaction, building);

        let report = issue_approach_to_building_interaction(
            &mut world,
            &catalog,
            &interaction,
            &unit_catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            unit.id,
            farm.id,
        );
        assert_eq!(report.issued, 1);
        resolve_all_pending_unit_orders(
            &mut world,
            &unit_catalog,
            PassabilityCatalogs {
                doodad: &doodad_catalog,
                building: &catalog,
                footprint: &FootprintCatalog::default(),
            },
            &nav_config,
        );
        match &world.get_unit(unit.id).unwrap().state {
            UnitState::Moving { target, .. } => assert_eq!(*target, expected_target),
            other => panic!("expected moving toward spill position, got {other:?}"),
        }
    }

    #[test]
    fn farm_output_rejects_oversized_unit_insertion_due_to_one_by_one_grid() {
        use crate::world::{
            BuildingInteractionProfileCatalog, InventoryAccessResult, TransferError,
            TransferPlacementPolicy, building_inventory_bindings,
            can_unit_access_building_inventory, place_stack,
        };

        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction = BuildingInteractionProfileCatalog::default();
        let mut world = flat_world();
        let ctx = inventory_ctx();
        let (farm, unit) = farm_setup(&mut world, pos(10.0, 10.0), pos(10.5, 10.5));
        let unit_inventory = unit.inventory_id.expect("unit inventory");
        let output_inventory =
            building_inventory_bindings(world.building_inventory_binding_store(), farm.id)
                .into_iter()
                .find(|binding| binding.binding_id.as_str() == "primary_output")
                .expect("farm output")
                .inventory_id;

        {
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            place_stack(
                inventory_store,
                instance_store,
                ctx,
                unit_inventory,
                ItemDefinitionId::new("iron_ore"),
                1,
                0,
                0,
            )
            .unwrap();
        }

        assert!(matches!(
            can_unit_access_building_inventory(&world, &catalog, &interaction, unit.id, farm.id),
            InventoryAccessResult::Allowed
        ));

        let result = {
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            transfer_one(
                inventory_store,
                instance_store,
                ctx,
                unit_inventory,
                0,
                output_inventory,
                TransferPlacementPolicy::FirstFitOnly,
            )
        };
        assert!(matches!(result, Err(TransferError::DestinationNoFit)));
    }

    #[test]
    fn farm_output_to_unit_transfer_succeeds_for_prispod() {
        use crate::world::{
            BuildingInteractionProfileCatalog, TransferPlacementPolicy,
            building_inventory_bindings, place_stack,
        };

        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction = BuildingInteractionProfileCatalog::default();
        let _ = (&catalog, &interaction);
        let mut world = flat_world();
        let ctx = inventory_ctx();
        let (farm, unit) = farm_setup(&mut world, pos(10.0, 10.0), pos(10.5, 10.5));
        let unit_inventory = unit.inventory_id.expect("unit inventory");
        let output_inventory =
            building_inventory_bindings(world.building_inventory_binding_store(), farm.id)
                .into_iter()
                .find(|binding| binding.binding_id.as_str() == "primary_output")
                .expect("farm output")
                .inventory_id;

        {
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            place_stack(
                inventory_store,
                instance_store,
                ctx,
                output_inventory,
                ItemDefinitionId::new("prispod"),
                3,
                0,
                0,
            )
            .unwrap();
        }

        let report = {
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            transfer_one(
                inventory_store,
                instance_store,
                ctx,
                output_inventory,
                0,
                unit_inventory,
                TransferPlacementPolicy::FirstFitOnly,
            )
        }
        .expect("farm to unit transfer");
        assert_eq!(report.moved, 1);
    }
}
