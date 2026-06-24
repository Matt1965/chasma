//! Cached selection policy for the current frame (ADR-051 O1).

use bevy::prelude::*;

use crate::client::ClientInputModifiers;
use crate::world::SelectionControllabilityPolicy;

use super::selection_policy_for_frame;

/// Sync selection policy into [`ClientInputModifiers`] before input collection.
pub fn sync_selection_policy_state(
    mut modifiers: ResMut<ClientInputModifiers>,
    #[cfg(feature = "dev")] dev_state: Res<crate::dev::DevModeState>,
) {
    modifiers.selection_policy = selection_policy_for_frame(
        #[cfg(feature = "dev")]
        dev_state.enabled,
        #[cfg(not(feature = "dev"))]
        false,
    );
}

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectionPolicyState {
    pub policy: SelectionControllabilityPolicy,
}

impl SelectionPolicyState {
    pub fn current(&self) -> SelectionControllabilityPolicy {
        self.policy
    }
}
