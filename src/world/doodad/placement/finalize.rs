use super::finalized::FinalizedDoodadPlacement;
use crate::world::doodad::generation::DoodadSpawnCandidate;
use crate::world::{LocalPosition, WorldData, WorldPosition};

/// Outcome of [`finalize_placements`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlacementFinalizationResult {
    pub finalized: Vec<FinalizedDoodadPlacement>,
    pub placements_finalized: u32,
    pub terrain_snaps_applied: u32,
    pub skipped_terrain_unavailable: u32,
}

/// Resolve candidate transforms into materialization-ready placements (ADR-022).
///
/// Read-only on [`WorldData`]; deterministic and side-effect free. Does not
/// modify input candidates.
pub fn finalize_placements(
    candidates: &[DoodadSpawnCandidate],
    world: &WorldData,
    snap_to_terrain: bool,
) -> PlacementFinalizationResult {
    let mut result = PlacementFinalizationResult {
        finalized: Vec::with_capacity(candidates.len()),
        ..PlacementFinalizationResult::default()
    };

    for candidate in candidates {
        if snap_to_terrain {
            match snap_candidate_to_terrain(candidate, world) {
                Some(placement) => {
                    if placement.position.local.0.y != candidate.position.local.0.y {
                        result.terrain_snaps_applied += 1;
                    }
                    result.finalized.push(placement);
                    result.placements_finalized += 1;
                }
                None => result.skipped_terrain_unavailable += 1,
            }
        } else {
            result.finalized.push(FinalizedDoodadPlacement::from_candidate(candidate));
            result.placements_finalized += 1;
        }
    }

    result
}

fn snap_candidate_to_terrain(
    candidate: &DoodadSpawnCandidate,
    world: &WorldData,
) -> Option<FinalizedDoodadPlacement> {
    let terrain_height = world.sample_height_at_position(candidate.position)?;
    let mut local = candidate.position.local.0;
    local.y = terrain_height;
    Some(FinalizedDoodadPlacement {
        definition_id: candidate.definition_id.clone(),
        source: candidate.source,
        position: WorldPosition::new(candidate.position.chunk, LocalPosition::new(local)),
        rotation: candidate.rotation,
        scale: candidate.scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::doodad::catalog::DoodadDefinitionId;
    use crate::world::doodad::source::DoodadSource;
    use crate::world::terrain::Heightfield;
    use crate::world::{ChunkCoord, ChunkData, ChunkId, ChunkLayout, LocalPosition};
    use bevy::prelude::{Quat, Vec3};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn flat_world(height: f32) -> WorldData {
        let samples = vec![height; 9];
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        let mut world = WorldData::new(layout());
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn candidate_at(x: f32, y: f32, z: f32) -> DoodadSpawnCandidate {
        DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 1 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(x, y, z)),
            ),
            rotation: Quat::from_rotation_y(0.5),
            scale: Vec3::new(0.9, 1.0, 1.1),
        }
    }

    #[test]
    fn terrain_snap_updates_y() {
        let world = flat_world(42.0);
        let candidate = candidate_at(128.0, 0.0, 128.0);

        let result = finalize_placements(&[candidate], &world, true);

        assert_eq!(result.finalized.len(), 1);
        assert_eq!(result.finalized[0].position.local.0.y, 42.0);
        assert_eq!(result.terrain_snaps_applied, 1);
    }

    #[test]
    fn xz_unchanged_after_snap() {
        let world = flat_world(10.0);
        let candidate = candidate_at(64.0, 99.0, 96.0);

        let result = finalize_placements(&[candidate], &world, true);

        assert_eq!(result.finalized[0].position.local.0.x, 64.0);
        assert_eq!(result.finalized[0].position.local.0.z, 96.0);
    }

    #[test]
    fn rotation_preserved() {
        let world = flat_world(0.0);
        let candidate = candidate_at(128.0, 0.0, 128.0);
        let expected = candidate.rotation;

        let result = finalize_placements(&[candidate], &world, true);

        assert_eq!(result.finalized[0].rotation, expected);
    }

    #[test]
    fn scale_preserved() {
        let world = flat_world(0.0);
        let candidate = candidate_at(128.0, 0.0, 128.0);
        let expected = candidate.scale;

        let result = finalize_placements(&[candidate], &world, true);

        assert_eq!(result.finalized[0].scale, expected);
    }

    #[test]
    fn disabled_snap_keeps_original_y() {
        let world = flat_world(42.0);
        let candidate = candidate_at(128.0, 17.0, 128.0);

        let result = finalize_placements(&[candidate], &world, false);

        assert_eq!(result.finalized[0].position.local.0.y, 17.0);
        assert_eq!(result.terrain_snaps_applied, 0);
        assert_eq!(result.placements_finalized, 1);
    }

    #[test]
    fn finalization_is_deterministic() {
        let world = flat_world(25.0);
        let candidates = vec![
            candidate_at(64.0, 0.0, 64.0),
            candidate_at(128.0, 0.0, 128.0),
        ];

        let a = finalize_placements(&candidates, &world, true);
        let b = finalize_placements(&candidates, &world, true);

        assert_eq!(a, b);
    }

    #[test]
    fn snap_skips_when_terrain_not_resident() {
        let world = WorldData::new(layout());
        let candidate = candidate_at(128.0, 0.0, 128.0);

        let result = finalize_placements(&[candidate], &world, true);

        assert!(result.finalized.is_empty());
        assert_eq!(result.skipped_terrain_unavailable, 1);
    }
}
