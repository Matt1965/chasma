//! Pure social-option eligibility resolver.

use bevy::prelude::*;

use crate::world::combat::is_unit_alive;
use crate::world::ownership::is_player_controllable;
use crate::world::relationship::{
    AuthoredRelationshipCatalog, RelationshipStandingStore, effective_relationship_for_records,
};
use crate::world::unit::CombatState;
use crate::world::{UnitId, UnitRecord, WorldData};

use super::config::{DialogueActionKind, UnitDialogueConfig};

/// Authoritative social interaction range for approach-to-talk.
pub const DIALOGUE_INTERACTION_RANGE_METERS: f32 =
    crate::world::INTERACTION_WORK_RANGE_METERS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueUnavailableReason {
    DialogueNotSupported,
    OptionDisabled,
    RelationshipTooLow,
    ActorMissing,
    ActorDead,
    TargetMissing,
    TargetDead,
    TargetIsPlayerUnit,
    TargetCannotTrade,
    TargetCannotBeRecruited,
    CombatPreventsInteraction,
    DifferentSpace,
    TargetAttackingActor,
    SameUnit,
}

impl DialogueUnavailableReason {
    pub fn player_message(self, kind: DialogueActionKind) -> &'static str {
        match self {
            Self::DialogueNotSupported => "This character cannot be spoken with",
            Self::OptionDisabled => match kind {
                DialogueActionKind::Talk => "Talk is unavailable",
                DialogueActionKind::Trade => "This character does not trade",
                DialogueActionKind::Recruit => "This character cannot be recruited",
            },
            Self::RelationshipTooLow => "Requires better relationship",
            Self::ActorMissing | Self::ActorDead => "Your unit is unavailable",
            Self::TargetMissing | Self::TargetDead => "They are no longer available",
            Self::TargetIsPlayerUnit => "Already under your control",
            Self::TargetCannotTrade => "This character does not trade",
            Self::TargetCannotBeRecruited => "This character cannot be recruited",
            Self::CombatPreventsInteraction => "Cannot interact during combat",
            Self::DifferentSpace => "Too far away",
            Self::TargetAttackingActor => "They are attacking you",
            Self::SameUnit => "Invalid target",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueOptionAvailability {
    Available,
    Unavailable(DialogueUnavailableReason),
}

impl DialogueOptionAvailability {
    pub fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}

pub fn evaluate_dialogue_option(
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
    standing: &RelationshipStandingStore,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
    kind: DialogueActionKind,
) -> DialogueOptionAvailability {
    if actor_unit_id == target_unit_id {
        return DialogueOptionAvailability::Unavailable(DialogueUnavailableReason::SameUnit);
    }

    let actor = match world.get_unit(actor_unit_id) {
        Some(record) => record,
        None => {
            return DialogueOptionAvailability::Unavailable(DialogueUnavailableReason::ActorMissing);
        }
    };
    if !is_unit_alive(actor) {
        return DialogueOptionAvailability::Unavailable(DialogueUnavailableReason::ActorDead);
    }

    let target = match world.get_unit(target_unit_id) {
        Some(record) => record,
        None => {
            return DialogueOptionAvailability::Unavailable(DialogueUnavailableReason::TargetMissing);
        }
    };
    if !is_unit_alive(target) {
        return DialogueOptionAvailability::Unavailable(DialogueUnavailableReason::TargetDead);
    }

    if actor.current_space_id != target.current_space_id {
        return DialogueOptionAvailability::Unavailable(DialogueUnavailableReason::DifferentSpace);
    }

    if combat_blocks_dialogue(actor, target) {
        return DialogueOptionAvailability::Unavailable(
            DialogueUnavailableReason::CombatPreventsInteraction,
        );
    }

    if target_attacking_actor(target, actor_unit_id) {
        return DialogueOptionAvailability::Unavailable(
            DialogueUnavailableReason::TargetAttackingActor,
        );
    }

    let config = match target.dialogue.as_ref() {
        Some(config) => config,
        None => {
            return DialogueOptionAvailability::Unavailable(
                DialogueUnavailableReason::DialogueNotSupported,
            );
        }
    };

    evaluate_with_config(
        authored_relationships,
        standing,
        actor,
        target,
        config,
        kind,
    )
}

fn evaluate_with_config(
    authored_relationships: &AuthoredRelationshipCatalog,
    standing: &RelationshipStandingStore,
    actor: &UnitRecord,
    target: &UnitRecord,
    config: &UnitDialogueConfig,
    kind: DialogueActionKind,
) -> DialogueOptionAvailability {
    let rule = config.rule(kind);
    if !rule.enabled {
        return DialogueOptionAvailability::Unavailable(match kind {
            DialogueActionKind::Trade => DialogueUnavailableReason::TargetCannotTrade,
            DialogueActionKind::Recruit => DialogueUnavailableReason::TargetCannotBeRecruited,
            DialogueActionKind::Talk => DialogueUnavailableReason::OptionDisabled,
        });
    }

    if matches!(kind, DialogueActionKind::Trade | DialogueActionKind::Recruit)
        && is_player_controllable(target)
    {
        return DialogueOptionAvailability::Unavailable(DialogueUnavailableReason::TargetIsPlayerUnit);
    }

    let relationship = effective_relationship_for_records(
        authored_relationships,
        standing,
        target,
        actor,
    );
    if relationship < rule.min_relationship {
        return DialogueOptionAvailability::Unavailable(DialogueUnavailableReason::RelationshipTooLow);
    }

    DialogueOptionAvailability::Available
}

fn target_attacking_actor(target: &UnitRecord, actor_unit_id: UnitId) -> bool {
    matches!(
        target.combat_state,
        CombatState::Attacking { target } if target == actor_unit_id
    ) || matches!(
        target.combat_state,
        CombatState::Chasing { target } if target == actor_unit_id
    )
}

fn combat_blocks_dialogue(actor: &UnitRecord, target: &UnitRecord) -> bool {
    crate::world::unit_in_active_combat(&actor.combat_state)
        || crate::world::unit_in_active_combat(&target.combat_state)
}

pub fn units_within_dialogue_range(
    world: &WorldData,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
) -> bool {
    let layout = world.layout();
    let Some(actor) = world.get_unit(actor_unit_id) else {
        return false;
    };
    let Some(target) = world.get_unit(target_unit_id) else {
        return false;
    };
    crate::world::xz_distance(
        actor.placement.position,
        target.placement.position,
        layout,
    ) <= DIALOGUE_INTERACTION_RANGE_METERS
}

pub fn unit_supports_dialogue(record: &UnitRecord) -> bool {
    record.dialogue.is_some()
}

pub fn present_dialogue_options(
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
    standing: &RelationshipStandingStore,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
) -> Vec<(DialogueActionKind, DialogueOptionAvailability)> {
    DialogueActionKind::ALL
        .iter()
        .map(|kind| {
            let availability = if world
                .get_unit(target_unit_id)
                .and_then(|target| target.dialogue.as_ref())
                .is_some()
            {
                evaluate_dialogue_option(
                    world,
                    authored_relationships,
                    standing,
                    actor_unit_id,
                    target_unit_id,
                    *kind,
                )
            } else {
                DialogueOptionAvailability::Unavailable(DialogueUnavailableReason::DialogueNotSupported)
            };
            (*kind, availability)
        })
        .collect()
}
