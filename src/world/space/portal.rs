use bevy::prelude::*;

use super::id::{PortalId, SpaceId};

/// Portal traversal kind (ADR-083 B6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum PortalType {
    Stair,
    Ramp,
    ExteriorEntrance,
    Doorway,
    CaveEntrance,
}

impl PortalType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stair => "Stair",
            Self::Ramp => "Ramp",
            Self::ExteriorEntrance => "ExteriorEntrance",
            Self::Doorway => "Doorway",
            Self::CaveEntrance => "CaveEntrance",
        }
    }
}

/// Authoritative portal instance connecting two spaces (ADR-083 B6).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct PortalRecord {
    pub id: PortalId,
    pub portal_type: PortalType,
    pub from_space: SpaceId,
    pub to_space: SpaceId,
    /// Transition region center in global XZ.
    pub from_center_global_xz: Vec2,
    pub from_radius_meters: f32,
    /// Destination spawn position after transition.
    pub to_position: crate::world::WorldPosition,
    pub traversal_cost: f32,
    pub bidirectional: bool,
    pub enabled: bool,
    pub owning_building_id: Option<crate::world::BuildingId>,
}

impl PortalRecord {
    pub fn contains_agent_global(&self, agent_global_xz: Vec2) -> bool {
        if !(self.from_radius_meters > 0.0) || !self.from_radius_meters.is_finite() {
            return false;
        }
        agent_global_xz.distance(self.from_center_global_xz) <= self.from_radius_meters
    }
}
