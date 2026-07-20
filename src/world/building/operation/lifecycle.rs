//! Generic production lifecycle (EP1).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::building::operational_efficiency::OperationalLimitingFactor;

/// Authoritative production lifecycle for one building operation (EP1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect, Serialize, Deserialize)]
pub enum OperationLifecycle {
    #[default]
    Idle,
    Running,
    Blocked,
    Paused,
    Disabled,
    Completed,
}

impl OperationLifecycle {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Running => "Running",
            Self::Blocked => "Blocked",
            Self::Paused => "Paused",
            Self::Disabled => "Disabled",
            Self::Completed => "Completed",
        }
    }

    pub fn accepts_labor(self) -> bool {
        matches!(self, Self::Running)
    }
}

/// Update lifecycle after a blocked efficiency or policy gate.
pub fn set_blocked(
    lifecycle: &mut OperationLifecycle,
    blocked_reason: &mut Option<OperationalLimitingFactor>,
    reason: OperationalLimitingFactor,
) {
    *lifecycle = OperationLifecycle::Blocked;
    *blocked_reason = Some(reason);
}
