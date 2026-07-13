use bevy::prelude::*;

/// Authoritative lifecycle marker for a building instance (ADR-082 B5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum BuildingLifecycleState {
    #[default]
    Complete,
    /// Player placement awaiting construction (B4).
    Planned,
    /// Construction started; footprint blocks movement (B5).
    Foundation,
    /// Active timed/worker construction (B5).
    InProgress,
    /// HP reached zero; brief transitional state before ruins (B5).
    Destroyed,
    /// Non-operational wreck; placement reservation per policy (B5).
    Ruins,
}

impl BuildingLifecycleState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Complete => "Complete",
            Self::Planned => "Planned",
            Self::Foundation => "Foundation",
            Self::InProgress => "InProgress",
            Self::Destroyed => "Destroyed",
            Self::Ruins => "Ruins",
        }
    }

    /// Whether this lifecycle blocks unit movement via static occupancy.
    pub fn blocks_movement(self) -> bool {
        matches!(self, Self::Foundation | Self::InProgress | Self::Complete)
    }

    /// Whether construction simulation may advance this record.
    pub fn receives_construction_progress(self) -> bool {
        matches!(self, Self::Planned | Self::Foundation | Self::InProgress)
    }

    pub fn is_terminal_damage_state(self) -> bool {
        matches!(self, Self::Destroyed | Self::Ruins)
    }
}

/// Construction progress within the active build (ADR-082 B5).
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct ConstructionState {
    /// Overall normalized progress in `[0.0, 1.0]`.
    pub progress_0_1: f32,
}

impl Default for ConstructionState {
    fn default() -> Self {
        Self { progress_0_1: 0.0 }
    }
}

/// Reserved navigable interior space ids (B6+).
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct BuildingSpaces {
    pub space_ids: Vec<String>,
}

/// Interior runtime linkage for a building instance (ADR-084 B7).
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct BuildingInteriorState {
    pub profile_id: Option<String>,
    pub door_ids: Vec<u32>,
    pub child_doodad_ids: Vec<u64>,
    pub child_building_ids: Vec<u64>,
    pub activated: bool,
    pub interior_space_id: Option<crate::world::SpaceId>,
}
