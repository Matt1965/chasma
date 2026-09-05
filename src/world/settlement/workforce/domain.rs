//! Player-facing autonomous work permission domains (settlement workforce).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Binary allow/deny categories for autonomous settlement work eligibility.
///
/// Aligned with [`crate::world::WorkSkillId`] taxonomy. Distinct from physical
/// [`UnitWorkCapabilities`] and from future priority scores.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect, Serialize, Deserialize,
)]
pub enum WorkPermissionDomain {
    Farming,
    /// Manual labor including extraction and hauling.
    #[serde(alias = "Mining", alias = "Hauling")]
    GeneralLabor,
    Construction,
    Cooking,
    Science,
    Smithing,
}

impl WorkPermissionDomain {
    pub const ALL: [Self; 6] = [
        Self::Farming,
        Self::GeneralLabor,
        Self::Construction,
        Self::Cooking,
        Self::Science,
        Self::Smithing,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Farming => "Farming",
            Self::GeneralLabor => "General Labor",
            Self::Construction => "Construction",
            Self::Cooking => "Cooking",
            Self::Science => "Science",
            Self::Smithing => "Smithing",
        }
    }
}
