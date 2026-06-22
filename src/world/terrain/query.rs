//! Shared terrain height and slope queries against authoritative [`WorldData`] (ADR-029).
//!
//! Reads resident [`super::Heightfield`] data only — never terrain runtime meshes or
//! render exaggeration.

use crate::world::{LocalPosition, WorldData, WorldPosition};

use super::Heightfield;

/// Sample terrain height at `position` and return a copy with authoritative Y set.
///
/// Returns `None` when the owning chunk's heightfield is not resident.
pub fn ground_world_position(world: &WorldData, position: WorldPosition) -> Option<WorldPosition> {
    let height = world.sample_height_at_position(position)?;
    let mut local = position.local.0;
    local.y = height;
    Some(WorldPosition::new(position.chunk, LocalPosition::new(local)))
}

/// Estimate terrain slope in degrees at a chunk-local position.
///
/// Uses forward finite differences over one heightfield sample spacing.
/// Returns `None` when the neighborhood is not fully inside the heightfield domain.
pub fn estimate_slope_degrees(
    heightfield: &Heightfield,
    local_x: f32,
    local_z: f32,
) -> Option<f32> {
    let spacing = heightfield.spacing_meters();
    let size = heightfield.chunk_size_meters();

    if local_x < 0.0
        || local_z < 0.0
        || local_x + spacing > size + 1e-4
        || local_z + spacing > size + 1e-4
    {
        return None;
    }

    let h = heightfield.sample(local_x, local_z);
    let h_dx = heightfield.sample(local_x + spacing, local_z);
    let h_dz = heightfield.sample(local_x, local_z + spacing);
    let dhdx = (h_dx - h) / spacing;
    let dhdz = (h_dz - h) / spacing;
    Some(dhdx.hypot(dhdz).atan().to_degrees())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, WorldData};
    use bevy::prelude::Vec3;

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn sample_chunk(heights: Vec<f32>) -> ChunkData {
        let heightfield = Heightfield::from_samples(3, 128.0, heights).unwrap();
        ChunkData::new(heightfield, Vec::new())
    }

    #[test]
    fn ground_world_position_samples_resident_terrain() {
        let mut world = WorldData::new(layout());
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            sample_chunk(vec![0.0, 1.0, 2.0, 3.0, 11.0, 5.0, 6.0, 7.0, 8.0]),
        );
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
        );

        let grounded = ground_world_position(&world, position).unwrap();
        assert_eq!(grounded.local.0.x, 128.0);
        assert_eq!(grounded.local.0.z, 128.0);
        assert_eq!(grounded.local.0.y, 11.0);
    }

    #[test]
    fn ground_world_position_none_when_chunk_not_resident() {
        let world = WorldData::new(layout());
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(10.0, 5.0, 10.0)),
        );
        assert!(ground_world_position(&world, position).is_none());
    }

    #[test]
    fn flat_terrain_has_zero_slope() {
        let samples = vec![10.0; 9];
        let hf = Heightfield::from_samples(3, 128.0, samples).unwrap();
        let slope = estimate_slope_degrees(&hf, 128.0, 128.0).unwrap();
        assert!(slope.abs() < 1e-4);
    }

    #[test]
    fn ramp_has_nonzero_slope() {
        let mut samples = Vec::new();
        for _row in 0..3 {
            for col in 0..3 {
                samples.push(col as f32 * 40.0);
            }
        }
        let hf = Heightfield::from_samples(3, 128.0, samples).unwrap();
        let slope = estimate_slope_degrees(&hf, 128.0, 128.0).unwrap();
        assert!(slope > 15.0);
    }
}
