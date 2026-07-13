use bevy::prelude::*;

/// Optional per-doodad metadata container (ADR-015, ADR-084 B7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
pub struct DoodadMetadata {
    pub parent_building_id: Option<crate::world::BuildingId>,
    pub interior_space_id: Option<crate::world::SpaceId>,
}
