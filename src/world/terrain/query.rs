//! Shared terrain height and slope queries against authoritative [`WorldData`] (ADR-029, REVIEW-B4).
//!
//! Reads resident [`super::Heightfield`] data only — never terrain runtime meshes or
//! render exaggeration. Simulation callers must use [`try_sample_height_at_position`] /
//! [`try_ground_world_position`] and handle [`super::TerrainQueryError`] explicitly.

use crate::world::{ChunkId, LocalPosition, WorldData, WorldPosition};

use super::{Heightfield, TerrainQueryError};

/// Sample resident heightfield height at an authoritative [`WorldPosition`].
pub fn try_sample_height_at_position(
    world: &WorldData,
    position: WorldPosition,
) -> Result<f32, TerrainQueryError> {
    let chunk_id = ChunkId::new(position.chunk);
    let data = world
        .get(chunk_id)
        .ok_or(TerrainQueryError::ChunkNotResident)?;
    data.heightfield
        .try_sample(position.local.0.x, position.local.0.z)
}

/// Sample terrain height and return a copy with authoritative Y set.
pub fn try_ground_world_position(
    world: &WorldData,
    position: WorldPosition,
) -> Result<WorldPosition, TerrainQueryError> {
    let height = try_sample_height_at_position(world, position)?;
    let mut local = position.local.0;
    local.y = height;
    Ok(WorldPosition::new(position.chunk, LocalPosition::new(local)))
}

/// Convenience wrapper — maps all query failures to `None`.
///
/// Prefer [`try_ground_world_position`] when the failure reason matters.
pub fn ground_world_position(world: &WorldData, position: WorldPosition) -> Option<WorldPosition> {
    try_ground_world_position(world, position).ok()
}

/// Estimate terrain slope in degrees at a chunk-local position (ADR-005).
pub fn slope_at(world: &WorldData, position: WorldPosition) -> Result<f32, TerrainQueryError> {
    let chunk_id = ChunkId::new(position.chunk);
    let data = world
        .get(chunk_id)
        .ok_or(TerrainQueryError::ChunkNotResident)?;
    estimate_slope_degrees(
        &data.heightfield,
        position.local.0.x,
        position.local.0.z,
    )
    .ok_or(TerrainQueryError::SlopeUnavailable)
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

    if !heightfield.is_within_domain(local_x, local_z) {
        return None;
    }

    let h = heightfield.sample(local_x, local_z);

    let dhdx = if local_x + spacing <= size + 1e-4 {
        (heightfield.sample(local_x + spacing, local_z) - h) / spacing
    } else if local_x >= spacing {
        (h - heightfield.sample(local_x - spacing, local_z)) / spacing
    } else {
        return None;
    };

    let dhdz = if local_z + spacing <= size + 1e-4 {
        (heightfield.sample(local_x, local_z + spacing) - h) / spacing
    } else if local_z >= spacing {
        (h - heightfield.sample(local_x, local_z - spacing)) / spacing
    } else {
        return None;
    };

    Some(dhdx.hypot(dhdz).atan().to_degrees())
}

/// Whether terrain slope at `position` is within the unit's limit.
///
/// Returns `false` when heightfield data is unavailable or slope cannot be estimated.
pub fn is_position_slope_walkable(
    world: &WorldData,
    position: WorldPosition,
    max_slope_degrees: f32,
) -> bool {
    matches!(
        classify_slope_walkability(world, position, max_slope_degrees),
        SlopeWalkability::Walkable
    )
}

/// Slope classification for movement blocking (ADR-066).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlopeWalkability {
    Walkable,
    Unavailable,
    TooSteep,
}

/// Classify slope at `position` for movement outcome reporting.
pub fn classify_slope_walkability(
    world: &WorldData,
    position: WorldPosition,
    max_slope_degrees: f32,
) -> SlopeWalkability {
    match slope_at(world, position) {
        Ok(slope) if slope > max_slope_degrees => SlopeWalkability::TooSteep,
        Ok(_) => SlopeWalkability::Walkable,
        Err(TerrainQueryError::SlopeUnavailable) => SlopeWalkability::Unavailable,
        Err(TerrainQueryError::ChunkNotResident | TerrainQueryError::InvalidTerrainCoordinate) => {
            SlopeWalkability::Unavailable
        }
    }
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
    fn resident_terrain_query_succeeds() {
        let mut world = WorldData::new(layout());
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            sample_chunk(vec![0.0, 1.0, 2.0, 3.0, 11.0, 5.0, 6.0, 7.0, 8.0]),
        );
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
        );

        let height = try_sample_height_at_position(&world, position).unwrap();
        assert_eq!(height, 11.0);
        let grounded = try_ground_world_position(&world, position).unwrap();
        assert_eq!(grounded.local.0.y, 11.0);
    }

    #[test]
    fn non_resident_terrain_returns_chunk_not_resident() {
        let world = WorldData::new(layout());
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(10.0, 5.0, 10.0)),
        );
        assert_eq!(
            try_sample_height_at_position(&world, position),
            Err(TerrainQueryError::ChunkNotResident)
        );
        assert!(ground_world_position(&world, position).is_none());
    }

    #[test]
    fn out_of_domain_coordinate_returns_invalid_terrain_coordinate() {
        let mut world = WorldData::new(layout());
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            sample_chunk(vec![0.0; 9]),
        );
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(-5.0, 0.0, 64.0)),
        );
        assert_eq!(
            try_sample_height_at_position(&world, position),
            Err(TerrainQueryError::InvalidTerrainCoordinate)
        );
    }

    #[test]
    fn slope_at_reports_unavailable_for_missing_chunk() {
        let world = WorldData::new(layout());
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(64.0, 0.0, 64.0)),
        );
        assert_eq!(
            slope_at(&world, position),
            Err(TerrainQueryError::ChunkNotResident)
        );
    }

    #[test]
    fn classify_slope_distinguishes_unavailable_from_too_steep() {
        let mut world = WorldData::new(layout());
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            sample_chunk(vec![0.0; 9]),
        );
        let missing = WorldPosition::new(
            ChunkCoord::new(1, 0),
            LocalPosition::new(Vec3::new(64.0, 0.0, 64.0)),
        );
        assert_eq!(
            classify_slope_walkability(&world, missing, 30.0),
            SlopeWalkability::Unavailable
        );

        let mut ramp = Vec::new();
        for _row in 0..3 {
            for col in 0..3 {
                ramp.push(col as f32 * 40.0);
            }
        }
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            sample_chunk(ramp),
        );
        let steep = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
        );
        assert_eq!(
            classify_slope_walkability(&world, steep, 5.0),
            SlopeWalkability::TooSteep
        );
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
