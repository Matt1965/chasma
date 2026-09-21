use bevy::prelude::*;

use crate::world::{ChunkLayout, WorldPosition};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OriginSpawnAnchor {
    pub start_x: f32,
    pub start_z: f32,
    pub yaw_deg: f32,
}

const FORMATION_SPACING: f32 = 2.0;

pub fn origin_member_formation_offsets(count: usize) -> Vec<Vec3> {
    if count == 0 {
        return Vec::new();
    }
    let center = (count.saturating_sub(1) as f32) * 0.5;
    (0..count)
        .map(|index| Vec3::new((index as f32 - center) * FORMATION_SPACING, 0.0, 0.0))
        .collect()
}

pub fn member_spawn_global_position(
    anchor: &OriginSpawnAnchor,
    formation_offset: Vec3,
    preview_offset: Vec3,
) -> Vec3 {
    let yaw = anchor.yaw_deg.to_radians();
    let local = formation_offset + preview_offset;
    let rotated = Quat::from_rotation_y(yaw) * local;
    Vec3::new(anchor.start_x + rotated.x, 0.0, anchor.start_z + rotated.z)
}

pub fn world_position_from_global(global: Vec3, layout: ChunkLayout) -> WorldPosition {
    WorldPosition::from_global(global, layout)
}
