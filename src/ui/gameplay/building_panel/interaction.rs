//! Building panel inventory interaction eligibility (BP4).

use crate::ui::gameplay::inventory::{
    InventoryGridInteraction, InventoryPaneSide, InventoryUiState,
};
use crate::ui::gameplay::player_hud_state::primary_selected_unit;
use crate::units::input::SelectedUnits;
use crate::world::{
    BuildingCatalog, BuildingId, BuildingInteractionProfileCatalog, InventoryAccessResult, UnitId,
    WorldData, can_unit_access_building_inventory, is_unit_alive,
};

/// Inventory actor for building transfers: open panel actor, else primary selected unit.
pub fn resolve_building_inventory_actor(
    inventory_ui: &InventoryUiState,
    selected_units: &SelectedUnits,
) -> Option<UnitId> {
    if let Some(actor) = inventory_ui.actor_unit_id {
        return Some(actor);
    }
    primary_selected_unit(selected_units)
}

/// Whether the actor may mutate inventories on this building (range + policy + alive).
pub fn building_inventory_transfer_eligible(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    building_id: BuildingId,
    actor: UnitId,
) -> bool {
    let Some(unit) = world.get_unit(actor) else {
        return false;
    };
    if !is_unit_alive(unit) || unit.inventory_id.is_none() {
        return false;
    }
    matches!(
        can_unit_access_building_inventory(
            world,
            building_catalog,
            interaction_catalog,
            actor,
            building_id,
        ),
        InventoryAccessResult::Allowed
    )
}

pub fn building_inventory_grid_interaction(eligible: bool) -> InventoryGridInteraction {
    if eligible {
        InventoryGridInteraction::Interactive {
            side: InventoryPaneSide::Right,
        }
    } else {
        InventoryGridInteraction::ReadOnly
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::gameplay::inventory::{
        InventoryGridInteraction, InventoryPaneSide, InventoryUiState,
    };
    use crate::units::input::SelectedUnits;
    use crate::world::{
        Affiliation, BuildingCatalog, BuildingCategoryCatalog, BuildingDefinitionId,
        BuildingInteractionProfileCatalog, BuildingOwnership, BuildingSource, ChunkCoord,
        ChunkData, ChunkId, ChunkLayout, Heightfield, InventoryCatalogCtx, InventoryProfileCatalog,
        ItemCatalog, ItemCategoryCatalog, LocalPosition, UnitCatalog, UnitDefinitionId,
        UnitOwnership, UnitSource, WorldData, WorldPosition, create_building_with_inventory,
        create_unit_with_inventory, starter_building_definitions,
        starter_inventory_profile_definitions, starter_item_category_definitions,
        starter_item_definitions, starter_unit_definitions,
    };
    use bevy::prelude::{Quat, Vec3};

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
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

    #[test]
    fn building_grid_read_only_when_actor_out_of_range() {
        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction = BuildingInteractionProfileCatalog::default();
        let mut world = flat_world();
        let ctx = inventory_ctx();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let farm = create_building_with_inventory(
            &catalog,
            &mut world,
            &BuildingDefinitionId::new("prispod_farm"),
            pos(40.0, 40.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            ctx,
        )
        .unwrap();
        let unit = create_unit_with_inventory(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            ctx,
        )
        .unwrap();
        let mut selection = SelectedUnits::default();
        selection.set_single(unit.id);
        let actor = resolve_building_inventory_actor(&InventoryUiState::default(), &selection)
            .expect("actor");
        let eligible =
            building_inventory_transfer_eligible(&world, &catalog, &interaction, farm.id, actor);
        assert!(!eligible);
        assert_eq!(
            building_inventory_grid_interaction(eligible),
            InventoryGridInteraction::ReadOnly
        );
    }

    #[test]
    fn building_grid_interactive_when_actor_in_range() {
        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction = BuildingInteractionProfileCatalog::default();
        let mut world = flat_world();
        let ctx = inventory_ctx();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let farm = create_building_with_inventory(
            &catalog,
            &mut world,
            &BuildingDefinitionId::new("prispod_farm"),
            pos(10.0, 10.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            ctx,
        )
        .unwrap();
        let unit = create_unit_with_inventory(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(10.5, 10.5),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            ctx,
        )
        .unwrap();
        let mut selection = SelectedUnits::default();
        selection.set_single(unit.id);
        let actor = resolve_building_inventory_actor(&InventoryUiState::default(), &selection)
            .expect("actor");
        let eligible =
            building_inventory_transfer_eligible(&world, &catalog, &interaction, farm.id, actor);
        assert!(eligible);
        assert_eq!(
            building_inventory_grid_interaction(eligible),
            InventoryGridInteraction::Interactive {
                side: InventoryPaneSide::Right
            }
        );
    }

    #[test]
    fn range_transitions_follow_actor_position_without_reopen() {
        let categories = BuildingCategoryCatalog::default();
        let catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction = BuildingInteractionProfileCatalog::default();
        let mut world = flat_world();
        let ctx = inventory_ctx();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let farm = create_building_with_inventory(
            &catalog,
            &mut world,
            &BuildingDefinitionId::new("prispod_farm"),
            pos(20.0, 20.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            ctx,
        )
        .unwrap();
        let unit = create_unit_with_inventory(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(20.5, 20.5),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            ctx,
        )
        .unwrap();
        let mut selection = SelectedUnits::default();
        selection.set_single(unit.id);
        let actor = resolve_building_inventory_actor(&InventoryUiState::default(), &selection)
            .expect("actor");

        assert!(building_inventory_transfer_eligible(
            &world,
            &catalog,
            &interaction,
            farm.id,
            actor,
        ));

        world.mutate_unit(unit.id, |record| {
            record.placement.position = pos(1.0, 1.0);
        });
        assert!(!building_inventory_transfer_eligible(
            &world,
            &catalog,
            &interaction,
            farm.id,
            actor,
        ));

        world.mutate_unit(unit.id, |record| {
            record.placement.position = pos(20.5, 20.5);
        });
        assert!(building_inventory_transfer_eligible(
            &world,
            &catalog,
            &interaction,
            farm.id,
            actor,
        ));
    }
}
