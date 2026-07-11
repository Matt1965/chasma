use super::finalized::FinalizedDoodadPlacement;
use super::variation::apply_catalog_believability_batch;
use crate::world::doodad::catalog::DoodadCatalog;
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

/// Resolve candidate transforms into materialization-ready placements (ADR-022, R7).
///
/// Read-only on [`WorldData`] and input `candidates`. When `catalog` is provided,
/// catalog-driven scale and yaw are applied to finalized placements only.
pub fn finalize_placements(
    candidates: &[DoodadSpawnCandidate],
    world: &WorldData,
    snap_to_terrain: bool,
    catalog: Option<&DoodadCatalog>,
) -> PlacementFinalizationResult {
    let mut result = PlacementFinalizationResult {
        finalized: Vec::with_capacity(candidates.len()),
        ..PlacementFinalizationResult::default()
    };

    for candidate in candidates {
        let placement = if snap_to_terrain {
            match snap_candidate_to_terrain(candidate, world) {
                Some(placement) => {
                    if placement.position.local.0.y != candidate.position.local.0.y {
                        result.terrain_snaps_applied += 1;
                    }
                    placement
                }
                None => {
                    result.skipped_terrain_unavailable += 1;
                    continue;
                }
            }
        } else {
            FinalizedDoodadPlacement::from_candidate(candidate)
        };

        result.finalized.push(placement);
        result.placements_finalized += 1;
    }

    if let Some(catalog) = catalog {
        apply_catalog_believability_batch(&mut result.finalized, catalog);
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
    use crate::world::doodad::catalog::DoodadCatalog;
    use crate::world::doodad::catalog::{DoodadDefinition, DoodadRenderKey};
    use crate::world::doodad::source::DoodadSource;
    use crate::world::terrain::Heightfield;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadDefinitionId, DoodadKind, LocalPosition,
    };
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

    fn catalog_with_tree(min: f32, max: f32, random_y: bool) -> DoodadCatalog {
        DoodadCatalog::from_definitions(vec![
            DoodadDefinition::new(
                DoodadDefinitionId::new("tree_oak"),
                DoodadKind::Tree,
                "Oak",
                4.0,
                min,
                max,
                None,
                None,
                Some(25.0),
                true,
                DoodadRenderKey::reserved("tree/oak"),
            )
            .with_random_rotation_y(random_y),
        ])
        .unwrap()
    }

    #[test]
    fn terrain_snap_updates_y() {
        let world = flat_world(42.0);
        let candidate = candidate_at(128.0, 0.0, 128.0);

        let result = finalize_placements(&[candidate], &world, true, None);

        assert_eq!(result.finalized.len(), 1);
        assert_eq!(result.finalized[0].position.local.0.y, 42.0);
        assert_eq!(result.terrain_snaps_applied, 1);
    }

    #[test]
    fn candidates_are_not_mutated_by_finalization() {
        let world = flat_world(10.0);
        let candidates = vec![candidate_at(64.0, 0.0, 64.0)];
        let before = candidates.clone();
        let catalog = catalog_with_tree(0.8, 1.2, true);

        let _ = finalize_placements(&candidates, &world, true, Some(&catalog));

        assert_eq!(candidates, before);
    }

    #[test]
    fn catalog_believability_overrides_candidate_transform() {
        let world = flat_world(0.0);
        let candidate = candidate_at(128.0, 0.0, 128.0);
        let catalog = catalog_with_tree(0.8, 1.2, true);

        let result = finalize_placements(&[candidate], &world, true, Some(&catalog));

        assert_ne!(result.finalized[0].rotation, Quat::from_rotation_y(0.5));
        assert_ne!(result.finalized[0].scale, Vec3::new(0.9, 1.0, 1.1));
    }

    #[test]
    fn finalization_without_catalog_preserves_candidate_transform() {
        let world = flat_world(0.0);
        let candidate = candidate_at(128.0, 0.0, 128.0);
        let expected_rotation = candidate.rotation;
        let expected_scale = candidate.scale;

        let result = finalize_placements(&[candidate], &world, true, None);

        assert_eq!(result.finalized[0].rotation, expected_rotation);
        assert_eq!(result.finalized[0].scale, expected_scale);
    }

    #[test]
    fn finalization_is_deterministic_with_catalog() {
        let world = flat_world(25.0);
        let candidates = vec![
            candidate_at(64.0, 0.0, 64.0),
            candidate_at(128.0, 0.0, 128.0),
        ];
        let catalog = catalog_with_tree(0.85, 1.15, true);

        let a = finalize_placements(&candidates, &world, true, Some(&catalog));
        let b = finalize_placements(&candidates, &world, true, Some(&catalog));

        assert_eq!(a, b);
    }

    #[test]
    fn snap_skips_when_terrain_not_resident() {
        let world = WorldData::new(layout());
        let candidate = candidate_at(128.0, 0.0, 128.0);

        let result = finalize_placements(&[candidate], &world, true, None);

        assert!(result.finalized.is_empty());
        assert_eq!(result.skipped_terrain_unavailable, 1);
    }
}
