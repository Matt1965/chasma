//! Local player ownership context and selection policy (ADR-051 O1).

use bevy::prelude::*;

use crate::world::{SelectionControllabilityPolicy, DEFAULT_PLAYER_OWNER_ID, DEFAULT_PLAYER_TEAM_ID, OwnerId, TeamId};

/// Client-local human player identity for controllability checks.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalPlayerOwnership {
    pub owner_id: OwnerId,
    pub team_id: TeamId,
}

impl Default for LocalPlayerOwnership {
    fn default() -> Self {
        Self {
            owner_id: DEFAULT_PLAYER_OWNER_ID,
            team_id: DEFAULT_PLAYER_TEAM_ID,
        }
    }
}

/// Selection policy for the current frame (gameplay vs dev inspect override).
pub fn selection_policy_for_frame(dev_mode_enabled: bool) -> SelectionControllabilityPolicy {
    if dev_mode_enabled {
        SelectionControllabilityPolicy::dev_inspect()
    } else {
        SelectionControllabilityPolicy::gameplay_default()
    }
}

#[cfg(feature = "dev")]
pub fn selection_policy_from_dev(dev_state: &crate::dev::DevModeState) -> SelectionControllabilityPolicy {
    selection_policy_for_frame(dev_state.enabled)
}
