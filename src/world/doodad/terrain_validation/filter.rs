use super::slope::estimate_slope_degrees;
use crate::world::doodad::catalog::{DoodadCatalog, DoodadDefinition};
use crate::world::doodad::generation::DoodadSpawnCandidate;
use crate::world::{ChunkId, WorldData, WorldPosition};

/// Outcome of [`filter_candidates_by_terrain`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TerrainValidationResult {
    pub retained: Vec<DoodadSpawnCandidate>,
    pub skipped_invalid_definition: u32,
    pub skipped_disabled_definition: u32,
    pub skipped_terrain_unavailable: u32,
    pub skipped_height_constraint: u32,
    pub skipped_slope_constraint: u32,
    pub skipped_slope_unavailable: u32,
}

/// Validate procedural candidates against resident terrain and catalog constraints (ADR-021).
///
/// Read-only on [`WorldData`]; deterministic and side-effect free. Does not modify
/// candidate positions (no terrain snapping).
pub fn filter_candidates_by_terrain(
    candidates: &[DoodadSpawnCandidate],
    catalog: &DoodadCatalog,
    world: &WorldData,
) -> TerrainValidationResult {
    let mut result = TerrainValidationResult {
        retained: Vec::with_capacity(candidates.len()),
        ..TerrainValidationResult::default()
    };

    for candidate in candidates {
        let Some(definition) = catalog.get(&candidate.definition_id) else {
            result.skipped_invalid_definition += 1;
            continue;
        };

        if !definition.enabled {
            result.skipped_disabled_definition += 1;
            continue;
        }

        match validate_candidate_terrain(candidate.position, definition, world) {
            TerrainCheck::Accepted => result.retained.push(candidate.clone()),
            TerrainCheck::TerrainUnavailable => result.skipped_terrain_unavailable += 1,
            TerrainCheck::HeightConstraint => result.skipped_height_constraint += 1,
            TerrainCheck::SlopeConstraint => result.skipped_slope_constraint += 1,
            TerrainCheck::SlopeUnavailable => result.skipped_slope_unavailable += 1,
        }
    }

    result
}

enum TerrainCheck {
    Accepted,
    TerrainUnavailable,
    HeightConstraint,
    SlopeConstraint,
    SlopeUnavailable,
}

fn validate_candidate_terrain(
    position: WorldPosition,
    definition: &DoodadDefinition,
    world: &WorldData,
) -> TerrainCheck {
    let chunk_id = ChunkId::new(position.chunk);
    let Some(chunk_data) = world.get(chunk_id) else {
        return TerrainCheck::TerrainUnavailable;
    };

    let local_x = position.local.0.x;
    let local_z = position.local.0.z;
    let Some(terrain_height) = world.sample_height_at_position(position) else {
        return TerrainCheck::TerrainUnavailable;
    };

    if let Some(min) = definition.min_height {
        if terrain_height < min {
            return TerrainCheck::HeightConstraint;
        }
    }
    if let Some(max) = definition.max_height {
        if terrain_height > max {
            return TerrainCheck::HeightConstraint;
        }
    }

    if definition.max_slope_degrees.is_some() {
        let Some(slope) = estimate_slope_degrees(&chunk_data.heightfield, local_x, local_z) else {
            return TerrainCheck::SlopeUnavailable;
        };
        if let Some(max_slope) = definition.max_slope_degrees {
            if slope > max_slope {
                return TerrainCheck::SlopeConstraint;
            }
        }
    }

    TerrainCheck::Accepted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::doodad::catalog::{DoodadCatalog, DoodadDefinitionId, DoodadRenderKey};
    use crate::world::doodad::catalog::starter_definitions;
    use crate::world::terrain::Heightfield;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkLayout, DoodadKind, DoodadSource, LocalPosition, WorldPosition,
    };
    use bevy::prelude::{Quat, Vec3};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn flat_chunk(height: f32) -> ChunkData {
        let samples = vec![height; 9];
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        ChunkData::new(heightfield, Vec::new())
    }

    fn ramp_chunk() -> ChunkData {
        let mut samples = Vec::new();
        for _row in 0..3 {
            for col in 0..3 {
                samples.push(col as f32 * 40.0);
            }
        }
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        ChunkData::new(heightfield, Vec::new())
    }

    fn world_with_chunk(chunk: ChunkId, data: ChunkData) -> WorldData {
        let mut world = WorldData::new(layout());
        world.insert(chunk, data);
        world
    }

    fn candidate_at(local: Vec3) -> DoodadSpawnCandidate {
        DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 1 },
            position: WorldPosition::new(ChunkCoord::new(0, 0), LocalPosition::new(local)),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    fn catalog_with_constraints(
        min_height: Option<f32>,
        max_height: Option<f32>,
        max_slope_degrees: Option<f32>,
    ) -> DoodadCatalog {
        let mut defs = starter_definitions();
        defs[0] = DoodadDefinition::new(
            DoodadDefinitionId::new("tree_oak"),
            DoodadKind::Tree,
            "Oak Tree",
            4.0,
            0.85,
            1.15,
            min_height,
            max_height,
            max_slope_degrees,
            true,
            DoodadRenderKey::reserved("tree/oak"),
        );
        DoodadCatalog::from_definitions(defs).unwrap()
    }

    #[test]
    fn accepted_when_terrain_height_within_range() {
        let chunk = ChunkId::new(ChunkCoord::new(0, 0));
        let world = world_with_chunk(chunk, flat_chunk(50.0));
        let catalog = catalog_with_constraints(Some(10.0), Some(100.0), None);

        let result = filter_candidates_by_terrain(
            &[candidate_at(Vec3::new(128.0, 0.0, 128.0))],
            &catalog,
            &world,
        );

        assert_eq!(result.retained.len(), 1);
        assert_eq!(result.skipped_terrain_unavailable, 0);
    }

    #[test]
    fn rejected_when_below_min_height() {
        let world = world_with_chunk(ChunkId::new(ChunkCoord::new(0, 0)), flat_chunk(5.0));
        let catalog = catalog_with_constraints(Some(10.0), None, None);

        let result = filter_candidates_by_terrain(
            &[candidate_at(Vec3::new(128.0, 0.0, 128.0))],
            &catalog,
            &world,
        );

        assert!(result.retained.is_empty());
        assert_eq!(result.skipped_height_constraint, 1);
    }

    #[test]
    fn rejected_when_above_max_height() {
        let world = world_with_chunk(ChunkId::new(ChunkCoord::new(0, 0)), flat_chunk(150.0));
        let catalog = catalog_with_constraints(None, Some(100.0), None);

        let result = filter_candidates_by_terrain(
            &[candidate_at(Vec3::new(128.0, 0.0, 128.0))],
            &catalog,
            &world,
        );

        assert!(result.retained.is_empty());
        assert_eq!(result.skipped_height_constraint, 1);
    }

    #[test]
    fn rejected_when_terrain_chunk_not_resident() {
        let world = WorldData::new(layout());
        let catalog = catalog_with_constraints(None, None, None);

        let result = filter_candidates_by_terrain(
            &[candidate_at(Vec3::new(128.0, 0.0, 128.0))],
            &catalog,
            &world,
        );

        assert!(result.retained.is_empty());
        assert_eq!(result.skipped_terrain_unavailable, 1);
    }

    #[test]
    fn accepted_when_slope_within_max() {
        let world = world_with_chunk(ChunkId::new(ChunkCoord::new(0, 0)), flat_chunk(0.0));
        let catalog = catalog_with_constraints(None, None, Some(45.0));

        let result = filter_candidates_by_terrain(
            &[candidate_at(Vec3::new(128.0, 0.0, 128.0))],
            &catalog,
            &world,
        );

        assert_eq!(result.retained.len(), 1);
    }

    #[test]
    fn rejected_when_slope_exceeds_max() {
        let world = world_with_chunk(ChunkId::new(ChunkCoord::new(0, 0)), ramp_chunk());
        let catalog = catalog_with_constraints(None, None, Some(5.0));

        let result = filter_candidates_by_terrain(
            &[candidate_at(Vec3::new(128.0, 0.0, 128.0))],
            &catalog,
            &world,
        );

        assert!(result.retained.is_empty());
        assert_eq!(result.skipped_slope_constraint, 1);
    }

    #[test]
    fn validation_is_deterministic() {
        let world = world_with_chunk(ChunkId::new(ChunkCoord::new(0, 0)), flat_chunk(50.0));
        let catalog = catalog_with_constraints(Some(0.0), Some(100.0), Some(30.0));
        let candidates = vec![
            candidate_at(Vec3::new(128.0, 0.0, 128.0)),
            candidate_at(Vec3::new(64.0, 0.0, 64.0)),
        ];

        let a = filter_candidates_by_terrain(&candidates, &catalog, &world);
        let b = filter_candidates_by_terrain(&candidates, &catalog, &world);
        assert_eq!(a.retained.len(), b.retained.len());
        assert_eq!(a.skipped_height_constraint, b.skipped_height_constraint);
        assert_eq!(a.skipped_terrain_unavailable, b.skipped_terrain_unavailable);
    }
}
