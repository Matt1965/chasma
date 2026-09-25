//! Player corpse loot interaction dispatch.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::client::commands::CommandTarget;
use crate::client::inventory_intent::{InventoryIntent, InventoryIntentQueue};
use crate::units::input::{MoveOrdersReport, SelectedUnits, issue_move_orders_to_selection};
use crate::world::{
    AttackTargetingPolicy, CorpseId, CorpseSettings, CorpseState, DoodadCatalog, FootprintCatalog,
    ItemPileSettings, NavigationConfig, UnitCatalog, UnitId, WeaponCatalog, WorldData,
    is_unit_alive,
};

use super::inventory_dispatch::try_open_corpse_inventory;

/// A unit approaching a corpse to complete player loot interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingCorpsePlayerInteraction {
    pub actor_unit_id: UnitId,
    pub corpse_id: CorpseId,
}

#[derive(Resource, Default, Debug)]
pub struct PendingCorpsePlayerInteractionState {
    pending: Option<PendingCorpsePlayerInteraction>,
}

impl PendingCorpsePlayerInteractionState {
    pub fn get(&self) -> Option<PendingCorpsePlayerInteraction> {
        self.pending
    }

    pub fn set(&mut self, actor_unit_id: UnitId, corpse_id: CorpseId) {
        self.pending = Some(PendingCorpsePlayerInteraction {
            actor_unit_id,
            corpse_id,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpsePlayerInteractionOutcome {
    pub opened_inventory: bool,
    pub issued_approach: bool,
    pub deferred_until_arrival: bool,
}

pub fn supersede_pending_corpse_interaction_for_selection(
    pending: &mut PendingCorpsePlayerInteractionState,
    selection: &SelectedUnits,
) {
    for unit_id in selection.iter() {
        pending.clear_for_unit(unit_id);
    }
}

pub fn unit_within_corpse_interaction_range(
    world: &WorldData,
    settings: &CorpseSettings,
    actor_unit_id: UnitId,
    corpse_id: CorpseId,
) -> bool {
    let Some(unit) = world.get_unit(actor_unit_id) else {
        return false;
    };
    let Some(corpse) = world.corpse_store().get(corpse_id) else {
        return false;
    };
    if corpse.state != CorpseState::Present {
        return false;
    }
    if unit.current_space_id != corpse.current_space_id {
        return false;
    }
    let dist = crate::world::quantized_distance_squared_cm(
        unit.placement.position,
        corpse.placement.position,
    );
    dist <= settings.interaction_radius_squared_cm()
}

fn pending_interaction_still_valid(
    world: &WorldData,
    pending: PendingCorpsePlayerInteraction,
) -> bool {
    let Some(unit) = world.get_unit(pending.actor_unit_id) else {
        return false;
    };
    if !is_unit_alive(unit) || unit.inventory_id.is_none() {
        return false;
    }
    world
        .corpse_store()
        .get(pending.corpse_id)
        .is_some_and(|record| record.state == CorpseState::Present)
}

pub fn complete_corpse_player_interaction(
    inventory_queue: &mut InventoryIntentQueue,
    world: &WorldData,
    actor_unit_id: UnitId,
    corpse_id: CorpseId,
) -> bool {
    match try_open_corpse_inventory(world, actor_unit_id, corpse_id) {
        Ok(mode) => {
            inventory_queue.push(InventoryIntent::Open(mode));
            true
        }
        Err(_) => false,
    }
}

pub fn try_dispatch_corpse_player_interaction(
    world: &mut WorldData,
    inventory_queue: &mut InventoryIntentQueue,
    pending: &mut PendingCorpsePlayerInteractionState,
    corpse_settings: &CorpseSettings,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    actor_unit_id: UnitId,
    corpse_id: CorpseId,
) -> Option<CorpsePlayerInteractionOutcome> {
    if world.corpse_store().get(corpse_id).is_none() {
        return None;
    }

    if unit_within_corpse_interaction_range(world, corpse_settings, actor_unit_id, corpse_id) {
        pending.clear_for_unit(actor_unit_id);
        let opened_inventory =
            complete_corpse_player_interaction(inventory_queue, world, actor_unit_id, corpse_id);
        return Some(CorpsePlayerInteractionOutcome {
            opened_inventory,
            issued_approach: false,
            deferred_until_arrival: false,
        });
    }

    pending.set(actor_unit_id, corpse_id);
    let issued_approach = issue_approach_to_corpse(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        actor_unit_id,
        corpse_id,
    )
    .issued
        > 0;

    Some(CorpsePlayerInteractionOutcome {
        opened_inventory: false,
        issued_approach,
        deferred_until_arrival: true,
    })
}

fn issue_approach_to_corpse(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    actor_unit_id: UnitId,
    corpse_id: CorpseId,
) -> MoveOrdersReport {
    let Some(corpse) = world.corpse_store().get(corpse_id) else {
        return MoveOrdersReport::default();
    };
    let target = corpse.placement.position;
    let mut actor_only = SelectedUnits::default();
    actor_only.set_single(actor_unit_id);
    issue_move_orders_to_selection(
        world,
        &actor_only,
        unit_catalog,
        weapon_catalog,
        &crate::world::ItemCatalog::default(),
        doodad_catalog,
        nav_config,
        target,
        AttackTargetingPolicy::default(),
        &[],
    )
}

pub fn try_complete_pending_corpse_player_interaction(
    pending: &mut PendingCorpsePlayerInteractionState,
    inventory_queue: &mut InventoryIntentQueue,
    world: &WorldData,
    corpse_settings: &CorpseSettings,
) -> bool {
    let Some(pending_interaction) = pending.get() else {
        return false;
    };
    if !pending_interaction_still_valid(world, pending_interaction) {
        pending.clear();
        return false;
    }
    if !unit_within_corpse_interaction_range(
        world,
        corpse_settings,
        pending_interaction.actor_unit_id,
        pending_interaction.corpse_id,
    ) {
        return false;
    }
    let opened = complete_corpse_player_interaction(
        inventory_queue,
        world,
        pending_interaction.actor_unit_id,
        pending_interaction.corpse_id,
    );
    pending.clear();
    opened
}

#[derive(SystemParam)]
pub struct PendingCorpseInteractionTickParams<'w> {
    pub pending: ResMut<'w, PendingCorpsePlayerInteractionState>,
    pub inventory_queue: ResMut<'w, InventoryIntentQueue>,
    pub world: Res<'w, WorldData>,
    pub corpse_settings: Res<'w, CorpseSettings>,
}

pub fn tick_pending_corpse_player_interactions(mut params: PendingCorpseInteractionTickParams) {
    try_complete_pending_corpse_player_interaction(
        &mut params.pending,
        &mut params.inventory_queue,
        &params.world,
        &params.corpse_settings,
    );
}

pub fn resolve_corpse_interact_target(
    world: &WorldData,
    building_catalog: &crate::world::BuildingCatalog,
    doodad_catalog: &DoodadCatalog,
    footprint_catalog: &FootprintCatalog,
    interaction_catalog: &crate::world::BuildingInteractionProfileCatalog,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    pile_settings: &ItemPileSettings,
    corpse_settings: &CorpseSettings,
    target: CommandTarget,
) -> Option<CorpseId> {
    use crate::world::{
        InteractionQueryContext, InteractionTargetRef, InteractionType, query_world_interaction,
    };

    let position = match target {
        CommandTarget::Terrain { position } => position,
        CommandTarget::Unit { unit_id } => world
            .get_unit(unit_id)
            .map(|unit| unit.placement.position)
            .unwrap_or_else(|| {
                crate::world::WorldPosition::new(
                    crate::world::ChunkCoord::new(0, 0),
                    crate::world::LocalPosition::new(Vec3::ZERO),
                )
            }),
        CommandTarget::Building { building_id } => world
            .get_building(building_id)
            .map(|building| building.placement.position)
            .unwrap_or_else(|| {
                crate::world::WorldPosition::new(
                    crate::world::ChunkCoord::new(0, 0),
                    crate::world::LocalPosition::new(Vec3::ZERO),
                )
            }),
    };
    let ctx = InteractionQueryContext::new(
        world,
        doodad_catalog,
        building_catalog,
        footprint_catalog,
        interaction_catalog,
        unit_catalog,
        weapon_catalog,
        pile_settings,
        corpse_settings,
    );
    let Some(interaction) = query_world_interaction(&ctx, position) else {
        return None;
    };
    if interaction.interaction_type != InteractionType::Corpse {
        return None;
    }
    match interaction.target {
        InteractionTargetRef::Corpse(corpse_id) if interaction.valid => Some(corpse_id),
        _ => None,
    }
}
