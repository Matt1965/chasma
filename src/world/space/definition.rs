use bevy::prelude::*;

use super::id::SpaceId;
use crate::world::BuildingId;

/// Runtime navigable space instance (ADR-083 B6).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct SpaceRecord {
    pub id: SpaceId,
    pub owning_building_id: Option<BuildingId>,
    pub display_floor_label: String,
    pub visibility_group_id: u32,
    /// Reference elevation for presentation ordering (not navigation truth).
    pub reference_elevation: f32,
    /// Interior floor Y in global space when grounded.
    pub floor_y_global: f32,
    /// Optional room/zone metadata (ADR-084 B7).
    pub room_tag: Option<String>,
    pub enabled: bool,
    pub walkable: bool,
}

impl SpaceRecord {
    pub fn surface() -> Self {
        Self {
            id: SpaceId::SURFACE,
            owning_building_id: None,
            display_floor_label: "Surface".into(),
            visibility_group_id: 0,
            reference_elevation: 0.0,
            floor_y_global: 0.0,
            room_tag: None,
            enabled: true,
            walkable: true,
        }
    }
}
