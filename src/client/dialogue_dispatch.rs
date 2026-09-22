//! Player dialogue approach and session open dispatch.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::ui::gameplay::dialogue::DialogueSessionState;
use crate::units::input::{MoveOrdersReport, SelectedUnits, issue_move_orders_to_selection};
use crate::world::{AttackTargetingPolicy, is_unit_alive};
use crate::world::relationship::AuthoredRelationshipCatalog;
use crate::world::{
    DoodadCatalog, NavigationConfig, UnitCatalog, UnitId, WeaponCatalog, WorldData,
    unit_supports_dialogue, units_within_dialogue_range,
};

/// A unit approaching an NPC to open dialogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingDialogueInteraction {
    pub actor_unit_id: UnitId,
    pub target_unit_id: UnitId,
}

#[derive(Resource, Default, Debug)]
pub struct PendingDialogueInteractionState {
    pending: Option<PendingDialogueInteraction>,
}

impl PendingDialogueInteractionState {
    pub fn get(&self) -> Option<PendingDialogueInteraction> {
        self.pending
    }

    pub fn set(&mut self, actor_unit_id: UnitId, target_unit_id: UnitId) {
        self.pending = Some(PendingDialogueInteraction {
            actor_unit_id,
            target_unit_id,
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

pub fn supersede_pending_dialogue_for_selection(
    pending: &mut PendingDialogueInteractionState,
    selection: &SelectedUnits,
) {
    for unit_id in selection.iter() {
        pending.clear_for_unit(unit_id);
    }
}

fn pending_still_valid(
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
    pending: PendingDialogueInteraction,
) -> bool {
    let actor = match world.get_unit(pending.actor_unit_id) {
        Some(record) => record,
        None => return false,
    };
    let target = match world.get_unit(pending.target_unit_id) {
        Some(record) => record,
        None => return false,
    };
    if !is_unit_alive(actor) || !is_unit_alive(target) {
        return false;
    }
    if !unit_supports_dialogue(target) {
        return false;
    }
    if actor.current_space_id != target.current_space_id {
        return false;
    }
    let standing = world.relationship_standing_store();
    crate::world::evaluate_dialogue_option(
        world,
        authored_relationships,
        standing,
        pending.actor_unit_id,
        pending.target_unit_id,
        crate::world::DialogueActionKind::Talk,
    )
    .is_available()
        || crate::world::present_dialogue_options(
            world,
            authored_relationships,
            standing,
            pending.actor_unit_id,
            pending.target_unit_id,
        )
        .iter()
        .any(|(_, availability)| availability.is_available())
}

pub fn try_dispatch_dialogue_interaction(
    world: &mut WorldData,
    dialogue: &mut DialogueSessionState,
    pending: &mut PendingDialogueInteractionState,
    authored_relationships: &AuthoredRelationshipCatalog,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
) -> DialogueDispatchOutcome {
    let target = world.get_unit(target_unit_id);
    if !target.is_some_and(unit_supports_dialogue) {
        return DialogueDispatchOutcome::Ignored;
    }

    if units_within_dialogue_range(world, actor_unit_id, target_unit_id) {
        pending.clear_for_unit(actor_unit_id);
        dialogue.open_session(actor_unit_id, target_unit_id);
        return DialogueDispatchOutcome::Opened;
    }

    pending.set(actor_unit_id, target_unit_id);
    let issued_approach = issue_approach_to_dialogue(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        actor_unit_id,
        target_unit_id,
    )
    .issued
        > 0;

    DialogueDispatchOutcome::Deferred { issued_approach }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueDispatchOutcome {
    Ignored,
    Opened,
    Deferred { issued_approach: bool },
}

fn issue_approach_to_dialogue(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
) -> MoveOrdersReport {
    let Some(target) = world.get_unit(target_unit_id) else {
        return MoveOrdersReport::default();
    };
    let target_position = target.placement.position;
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
        target_position,
        AttackTargetingPolicy::default(),
        &[target_unit_id],
    )
}

pub fn try_complete_pending_dialogue_interaction(
    dialogue: &mut DialogueSessionState,
    pending: &mut PendingDialogueInteractionState,
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
) -> bool {
    let Some(pending_interaction) = pending.get() else {
        return false;
    };

    if !pending_still_valid(world, authored_relationships, pending_interaction) {
        pending.clear();
        return false;
    }

    if !units_within_dialogue_range(
        world,
        pending_interaction.actor_unit_id,
        pending_interaction.target_unit_id,
    ) {
        return false;
    }

    dialogue.open_session(
        pending_interaction.actor_unit_id,
        pending_interaction.target_unit_id,
    );
    pending.clear();
    true
}

#[derive(SystemParam)]
pub struct PendingDialogueInteractionTickParams<'w> {
    pub pending: ResMut<'w, PendingDialogueInteractionState>,
    pub dialogue: ResMut<'w, DialogueSessionState>,
    pub world: Res<'w, WorldData>,
    pub authored_relationships: Res<'w, AuthoredRelationshipCatalog>,
}

pub fn tick_pending_dialogue_interactions(mut params: PendingDialogueInteractionTickParams) {
    try_complete_pending_dialogue_interaction(
        &mut params.dialogue,
        &mut params.pending,
        &params.world,
        &params.authored_relationships,
    );
}
