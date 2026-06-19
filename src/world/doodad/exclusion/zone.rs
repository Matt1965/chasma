use bevy::prelude::*;

use crate::world::WorldPosition;

/// Data-only spherical exclusion region (ADR-015, ADR-020).
///
/// Stored on [`crate::world::WorldData`]. Procedural candidate filtering consumes
/// these zones before materialization; the authoring API is unaffected.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct DoodadExclusionZone {
    pub center: WorldPosition,
    pub radius_meters: f32,
}

impl DoodadExclusionZone {
    pub fn new(center: WorldPosition, radius_meters: f32) -> Self {
        Self {
            center,
            radius_meters,
        }
    }
}
