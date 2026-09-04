//! Player-facing autonomous work permission domains (settlement workforce).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Binary allow/deny categories for autonomous settlement work eligibility.
///
/// Distinct from physical [`UnitWorkCapabilities`] and from future priority scores.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect, Serialize, Deserialize,
)]
pub enum WorkPermissionDomain {
    Farming,
    Mining,
    Construction,
    Hauling,
}

impl WorkPermissionDomain {
    pub const ALL: [Self; 4] = [
        Self::Farming,
        Self::Mining,
        Self::Construction,
        Self::Hauling,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Farming => "Farming",
            Self::Mining => "Mining",
            Self::Construction => "Construction",
            Self::Hauling => "Hauling",
        }
    }
}
