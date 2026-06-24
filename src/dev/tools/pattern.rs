//! Pattern point generation for dev placement brushes (ADR-044).

use bevy::prelude::*;

use crate::world::DeterministicRng;
use crate::world::{ChunkLayout, WorldPosition};

/// Reusable XZ offset buffer (world meters relative to anchor global position).
#[derive(Debug, Default)]
pub struct PatternPointBuffer {
    offsets: Vec<Vec2>,
}

impl PatternPointBuffer {
    pub fn clear(&mut self) {
        self.offsets.clear();
    }

    pub fn offsets(&self) -> &[Vec2] {
        &self.offsets
    }

    pub fn push_offset(&mut self, offset: Vec2) {
        self.offsets.push(offset);
    }
}

/// Evenly spaced points along a direction from the origin offset.
pub fn line_offsets(count: u32, spacing: f32, direction: Vec2) -> PatternPointBuffer {
    let mut buffer = PatternPointBuffer::default();
    if count == 0 {
        return buffer;
    }
    let dir = if direction.length_squared() > 1e-6 {
        direction.normalize()
    } else {
        Vec2::X
    };
    let half = (count.saturating_sub(1)) as f32 * 0.5;
    for index in 0..count {
        let t = index as f32 - half;
        buffer.push_offset(dir * (t * spacing));
    }
    buffer
}

/// Radial ring distribution (includes center when count == 1).
pub fn circle_offsets(count: u32, radius: f32) -> PatternPointBuffer {
    let mut buffer = PatternPointBuffer::default();
    if count == 0 {
        return buffer;
    }
    if count == 1 {
        buffer.push_offset(Vec2::ZERO);
        return buffer;
    }
    buffer.push_offset(Vec2::ZERO);
    let ring_count = count - 1;
    for index in 0..ring_count {
        let angle = (index as f32 / ring_count as f32) * std::f32::consts::TAU;
        buffer.push_offset(Vec2::new(angle.cos(), angle.sin()) * radius);
    }
    buffer
}

/// NxM grid centered on anchor.
pub fn grid_offsets(columns: u32, rows: u32, spacing: f32) -> PatternPointBuffer {
    let mut buffer = PatternPointBuffer::default();
    if columns == 0 || rows == 0 {
        return buffer;
    }
    let cx = (columns.saturating_sub(1)) as f32 * 0.5;
    let cz = (rows.saturating_sub(1)) as f32 * 0.5;
    for row in 0..rows {
        for col in 0..columns {
            let offset = Vec2::new((col as f32 - cx) * spacing, (row as f32 - cz) * spacing);
            buffer.push_offset(offset);
        }
    }
    buffer
}

/// Deterministic scatter within a disc (seed derived externally).
pub fn scatter_offsets(count: u32, radius: f32, seed: u64) -> PatternPointBuffer {
    let mut buffer = PatternPointBuffer::default();
    if count == 0 {
        return buffer;
    }
    let mut rng = DeterministicRng::new(seed);
    for _ in 0..count {
        let angle = rng.next_f32() * std::f32::consts::TAU;
        let dist = rng.next_f32().sqrt() * radius;
        buffer.push_offset(Vec2::new(angle.cos() * dist, angle.sin() * dist));
    }
    buffer
}

/// Map XZ offsets to authoritative [`WorldPosition`] candidates at anchor height.
pub fn offsets_to_world_positions(
    anchor: WorldPosition,
    layout: ChunkLayout,
    offsets: &[Vec2],
    out: &mut Vec<WorldPosition>,
) {
    out.clear();
    let base = anchor.to_global(layout);
    out.reserve(offsets.len());
    for offset in offsets {
        let global = Vec3::new(base.x + offset.x, base.y, base.z + offset.y);
        out.push(WorldPosition::from_global(global, layout));
    }
}

/// Hash seed from world seed, anchor, and definition id string.
pub fn dev_placement_seed(world_seed: u64, anchor: WorldPosition, definition_key: &str) -> u64 {
    let mut h = world_seed;
    h = h
        .wrapping_mul(0x9E3779B97F4A7C15)
        .wrapping_add(anchor.chunk.x as u64);
    h = h
        .wrapping_mul(0x9E3779B97F4A7C15)
        .wrapping_add(anchor.chunk.z as u64);
    h = h
        .wrapping_add(anchor.local.0.x.to_bits() as u64)
        .wrapping_mul(0xBF58476D1CE4E5B9);
    h = h
        .wrapping_add(anchor.local.0.z.to_bits() as u64)
        .wrapping_mul(0x94D049BB133111EB);
    for byte in definition_key.as_bytes() {
        h = h.wrapping_add(*byte as u64).wrapping_mul(0x100000001B3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition};

    fn anchor() -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(40.0, 0.0, 40.0)),
        )
    }

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    #[test]
    fn line_brush_aligns_to_direction_vector() {
        let points = line_offsets(5, 2.0, Vec2::new(1.0, 0.0));
        assert_eq!(points.offsets().len(), 5);
        assert!(points.offsets().iter().all(|o| o.y.abs() < 1e-4));
        assert_eq!(points.offsets()[0].x, -4.0);
        assert_eq!(points.offsets()[4].x, 4.0);
    }

    #[test]
    fn grid_brush_respects_spacing() {
        let points = grid_offsets(3, 2, 4.0);
        assert_eq!(points.offsets().len(), 6);
        assert!(points.offsets().contains(&Vec2::new(-4.0, -2.0)));
        assert!(points.offsets().contains(&Vec2::new(4.0, 2.0)));
    }

    #[test]
    fn circle_brush_is_deterministic() {
        let a = circle_offsets(8, 6.0);
        let b = circle_offsets(8, 6.0);
        assert_eq!(a.offsets(), b.offsets());
    }

    #[test]
    fn scatter_is_deterministic_per_seed() {
        let a = scatter_offsets(12, 10.0, 12345);
        let b = scatter_offsets(12, 10.0, 12345);
        assert_eq!(a.offsets(), b.offsets());
        let c = scatter_offsets(12, 10.0, 99999);
        assert_ne!(a.offsets(), c.offsets());
    }

    #[test]
    fn dev_placement_seed_varies_by_definition() {
        let anchor = anchor();
        let a = dev_placement_seed(1, anchor, "wolf");
        let b = dev_placement_seed(1, anchor, "tree_oak");
        assert_ne!(a, b);
    }
}
