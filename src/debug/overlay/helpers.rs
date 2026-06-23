//! Shared helpers for debug overlay rendering.

use bevy::prelude::*;

use crate::terrain::world_position_to_render_global;
use crate::world::{ChunkLayout, WorldPosition};

pub fn render_position(
    position: WorldPosition,
    layout: ChunkLayout,
    vertical_scale: f32,
) -> Vec3 {
    world_position_to_render_global(position, layout, vertical_scale)
}

pub fn xz_to_render_y(base: Vec3, y_offset: f32) -> Vec3 {
    Vec3::new(base.x, base.y + y_offset, base.z)
}
