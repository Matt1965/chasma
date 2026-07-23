//! Shared gizmo helpers for navigation debug overlays (NV0).

use bevy::prelude::*;

use crate::world::{ChunkLayout, WorldData, WorldPosition, ground_world_position};

/// Draw a horizontal quad at global XZ with Y sampled from terrain.
pub fn draw_xz_quad(
    gizmos: &mut Gizmos,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
    center_xz: Vec2,
    half_extent: f32,
    y_lift: f32,
    color: Color,
) {
    let y = sample_terrain_y(world, center_xz, layout, vertical_scale) + y_lift;
    let corners = [
        Vec3::new(center_xz.x - half_extent, y, center_xz.y - half_extent),
        Vec3::new(center_xz.x + half_extent, y, center_xz.y - half_extent),
        Vec3::new(center_xz.x + half_extent, y, center_xz.y + half_extent),
        Vec3::new(center_xz.x - half_extent, y, center_xz.y + half_extent),
    ];
    for i in 0..4 {
        gizmos.line(corners[i], corners[(i + 1) % 4], color);
    }
}

pub fn sample_terrain_y(
    world: &WorldData,
    center_xz: Vec2,
    layout: ChunkLayout,
    vertical_scale: f32,
) -> f32 {
    let sample = WorldPosition::from_global(Vec3::new(center_xz.x, 0.0, center_xz.y), layout);
    ground_world_position(world, sample)
        .map(|grounded| grounded.to_global(layout).y * vertical_scale)
        .unwrap_or(0.0)
}
