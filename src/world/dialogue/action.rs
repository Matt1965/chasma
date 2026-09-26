//! Typed dialogue action dispatch seams.

use bevy::prelude::*;

use crate::world::UnitId;

/// Downstream systems consume these after eligibility revalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DialogueAction {
    Talk {
        actor_unit_id: UnitId,
        target_unit_id: UnitId,
    },
    Trade {
        actor_unit_id: UnitId,
        target_unit_id: UnitId,
    },
    Recruit {
        actor_unit_id: UnitId,
        target_unit_id: UnitId,
    },
}
