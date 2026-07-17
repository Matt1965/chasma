use super::options::MaterializationOptions;
use super::report::DoodadMaterializationReport;
use crate::world::WorldData;
use crate::world::doodad::authoring::{
    DoodadAuthoringError, DoodadPlacementOverrides, create_doodad,
};
use crate::world::doodad::biome_filter::{BiomeFilterResult, filter_candidates_by_biome};
use crate::world::doodad::catalog::DoodadCatalog;
use crate::world::doodad::exclusion::filter_candidates_by_exclusion_zones;
use crate::world::doodad::generation::DoodadSpawnCandidate;
use crate::world::doodad::placement::{
    FinalizedDoodadPlacement, PlacementFinalizationResult, finalize_placements,
};
use crate::world::doodad::procedural_key::ProceduralDoodadKey;
use crate::world::doodad::terrain_validation::{
    TerrainValidationResult, filter_candidates_by_terrain,
};

/// Materialize spawn candidates into [`WorldData`] via the authoring API (ADR-019).
///
/// Uses [`MaterializationOptions::procedural_default`]: biome filtering, exclusion
/// filtering, terrain validation, and terrain snap. For minimal snap-only
/// behavior (tests, custom pipelines), use [`materialize_candidates_with_options`]
/// with [`MaterializationOptions::raw`].
pub fn materialize_candidates(
    catalog: &DoodadCatalog,
    world: &mut WorldData,
    candidates: &[DoodadSpawnCandidate],
) -> DoodadMaterializationReport {
    materialize_candidates_with_options(
        catalog,
        world,
        candidates,
        &MaterializationOptions::default(),
    )
}

/// Materialize procedural candidates with the production preset (ADR-020).
///
/// Equivalent to [`materialize_candidates`] — both use
/// [`MaterializationOptions::procedural_default`].
pub fn materialize_candidates_with_exclusion(
    catalog: &DoodadCatalog,
    world: &mut WorldData,
    candidates: &[DoodadSpawnCandidate],
) -> DoodadMaterializationReport {
    materialize_candidates(catalog, world, candidates)
}

/// Materialize with explicit pipeline options.
pub fn materialize_candidates_with_options(
    catalog: &DoodadCatalog,
    world: &mut WorldData,
    candidates: &[DoodadSpawnCandidate],
    options: &MaterializationOptions,
) -> DoodadMaterializationReport {
    let original_count = candidates.len() as u32;

    let biome_result = if options.apply_biome_filter {
        filter_candidates_by_biome(candidates, catalog, world)
    } else {
        BiomeFilterResult {
            retained: candidates.to_vec(),
            ..BiomeFilterResult::default()
        }
    };
    let biome_skipped_disallowed = biome_result.skipped_biome_disallowed;
    let biome_skipped_unavailable = biome_result.skipped_biome_unavailable;
    let biome_skipped_invalid = biome_result.skipped_invalid_definition;
    let after_biome = biome_result.retained;

    let (to_materialize, excluded_by_zone) = if options.apply_exclusion_zones {
        let layout = world.layout();
        let filtered = filter_candidates_by_exclusion_zones(
            &after_biome,
            world.doodad_exclusion_zones(),
            layout,
        );
        (filtered.retained, filtered.excluded_count)
    } else {
        (after_biome, 0)
    };

    let terrain_result = if options.validate_terrain {
        filter_candidates_by_terrain(&to_materialize, catalog, world)
    } else {
        TerrainValidationResult {
            retained: to_materialize,
            ..TerrainValidationResult::default()
        }
    };

    let finalization = finalize_placements(
        &terrain_result.retained,
        world,
        options.snap_to_terrain,
        options.apply_catalog_believability.then_some(catalog),
    );

    let PlacementFinalizationResult {
        finalized,
        placements_finalized,
        terrain_snaps_applied,
        skipped_terrain_unavailable,
    } = finalization;

    let mut report = materialize_finalized_slice(catalog, world, &finalized);
    report.candidates_received = original_count;
    report.excluded_by_zone = excluded_by_zone;
    report.skipped_biome_disallowed = biome_skipped_disallowed;
    report.skipped_biome_unavailable = biome_skipped_unavailable;
    report.skipped_invalid_definition += biome_skipped_invalid;
    merge_terrain_validation(&mut report, &terrain_result);
    report.placements_finalized = placements_finalized;
    report.terrain_snaps_applied = terrain_snaps_applied;
    report.skipped_terrain_unavailable += skipped_terrain_unavailable;
    report
}

fn merge_terrain_validation(
    report: &mut DoodadMaterializationReport,
    terrain: &TerrainValidationResult,
) {
    report.skipped_invalid_definition += terrain.skipped_invalid_definition;
    report.skipped_disabled_definition += terrain.skipped_disabled_definition;
    report.skipped_terrain_unavailable = terrain.skipped_terrain_unavailable;
    report.skipped_height_constraint = terrain.skipped_height_constraint;
    report.skipped_slope_constraint = terrain.skipped_slope_constraint;
    report.skipped_slope_unavailable = terrain.skipped_slope_unavailable;
}

fn materialize_finalized_slice(
    catalog: &DoodadCatalog,
    world: &mut WorldData,
    placements: &[FinalizedDoodadPlacement],
) -> DoodadMaterializationReport {
    let mut report = DoodadMaterializationReport {
        candidates_received: placements.len() as u32,
        ..DoodadMaterializationReport::default()
    };

    for placement in placements {
        if let Some(key) = ProceduralDoodadKey::from_finalized(placement) {
            if world.procedural_doodad_id(&key).is_some() {
                report.skipped_duplicate += 1;
                continue;
            }
        }

        let Some(definition) = catalog.get(&placement.definition_id) else {
            report.skipped_invalid_definition += 1;
            continue;
        };

        if !definition.enabled {
            report.skipped_disabled_definition += 1;
            continue;
        }

        match create_doodad(
            catalog,
            world,
            &placement.definition_id,
            placement.position,
            placement.source,
            DoodadPlacementOverrides {
                rotation: Some(placement.rotation),
                scale: Some(placement.scale),
            },
            None,
        ) {
            Ok(record) => {
                if let Some(key) = ProceduralDoodadKey::from_record(&record) {
                    world.register_procedural_doodad(key, record.id);
                }
                report.inserted += 1;
            }
            Err(DoodadAuthoringError::ScaleOutOfRange { .. }) => {
                report.skipped_validation_failed += 1;
            }
            Err(DoodadAuthoringError::DefinitionNotFound(_)) => {
                report.skipped_invalid_definition += 1;
            }
            Err(DoodadAuthoringError::DefinitionDisabled(_)) => {
                report.skipped_disabled_definition += 1;
            }
            Err(DoodadAuthoringError::ChunkPlacementMismatch) => {
                report.skipped_validation_failed += 1;
            }
            Err(DoodadAuthoringError::DoodadNotFound(_)) => {
                report.skipped_validation_failed += 1;
            }
            Err(DoodadAuthoringError::Occupancy(_)) => {
                report.skipped_validation_failed += 1;
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::doodad::authoring::create_doodad;
    use crate::world::doodad::catalog::starter_definitions;
    use crate::world::doodad::generation::{DoodadGenerationContext, generate_chunk_doodads};
    use crate::world::{
        ChunkCoord, ChunkId, ChunkLayout, DoodadCatalog, DoodadDefinitionId, DoodadExclusionZone,
        DoodadPlacementOverrides, DoodadSource, LocalPosition, WorldPosition,
        biome::{BiomeColorMapping, BiomeMask, BiomeMaskBounds},
    };
    use bevy::prelude::{Quat, Vec3};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn world() -> WorldData {
        WorldData::new(layout())
    }

    fn insert_flat_chunk(world: &mut WorldData, x: i32, z: i32, height: f32) {
        use crate::world::ChunkData;
        use crate::world::terrain::Heightfield;

        let samples = vec![height; 9];
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(x, z)),
            ChunkData::new(heightfield, Vec::new()),
        );
    }

    fn world_with_chunk(x: i32, z: i32, height: f32) -> WorldData {
        let mut world = world();
        insert_flat_chunk(&mut world, x, z, height);
        world
    }

    fn install_uniform_forest_mask(world: &mut WorldData) {
        let mask = BiomeMask::from_rgba_rows(
            1,
            1,
            BiomeMaskBounds::new(0.0, 0.0, 4096.0, 4096.0),
            &[0, 255, 0],
            3,
            &BiomeColorMapping::starter(),
        )
        .unwrap();
        world.set_biome_mask(mask);
    }

    fn forest_desert_mask() -> BiomeMask {
        BiomeMask::from_rgba_rows(
            2,
            1,
            BiomeMaskBounds::new(0.0, 0.0, 512.0, 256.0),
            &[0, 255, 0, 255, 255, 0, 0, 255],
            4,
            &BiomeColorMapping::starter(),
        )
        .unwrap()
    }

    fn materialize_raw(
        catalog: &DoodadCatalog,
        world: &mut WorldData,
        candidates: &[DoodadSpawnCandidate],
    ) -> DoodadMaterializationReport {
        materialize_candidates_with_options(
            catalog,
            world,
            candidates,
            &MaterializationOptions::raw(),
        )
    }

    fn candidates_for_chunk(seed: u64, x: i32, z: i32) -> Vec<DoodadSpawnCandidate> {
        let layout = layout();
        let ctx = DoodadGenerationContext::new(seed, ChunkId::new(ChunkCoord::new(x, z)), &layout);
        generate_chunk_doodads(&ctx, &DoodadCatalog::default())
    }

    #[test]
    fn materialize_candidates_inserts_records() {
        let catalog = DoodadCatalog::default();
        let mut world = world_with_chunk(0, 0, 0.0);
        let candidates = candidates_for_chunk(42, 0, 0);

        let report = materialize_raw(&catalog, &mut world, &candidates);

        assert_eq!(report.inserted, candidates.len() as u32);
        assert_eq!(report.candidates_received, candidates.len() as u32);
        assert_eq!(report.excluded_by_zone, 0);
        assert!(report.skipped_at_insert() == 0);
        assert!(
            world
                .get_doodad(
                    world
                        .procedural_doodad_id(
                            &ProceduralDoodadKey::from_candidate(&candidates[0]).unwrap()
                        )
                        .unwrap()
                )
                .is_some()
        );
        world.assert_doodad_index_consistent();
    }

    #[test]
    fn materialized_records_use_candidate_definition_id() {
        let catalog = DoodadCatalog::default();
        let mut world = world_with_chunk(2, 3, 0.0);
        let candidates = candidates_for_chunk(1, 2, 3);
        materialize_raw(&catalog, &mut world, &candidates);

        for candidate in &candidates {
            let key = ProceduralDoodadKey::from_candidate(candidate).unwrap();
            let id = world.procedural_doodad_id(&key).unwrap();
            assert_eq!(
                world.get_doodad(id).unwrap().definition_id,
                candidate.definition_id
            );
        }
    }

    #[test]
    fn materialized_records_preserve_procedural_source_seed() {
        let catalog = DoodadCatalog::default();
        let mut world = world_with_chunk(0, 0, 0.0);
        let candidates = candidates_for_chunk(99, 0, 0);
        materialize_raw(&catalog, &mut world, &candidates);

        for candidate in &candidates {
            let key = ProceduralDoodadKey::from_candidate(candidate).unwrap();
            let id = world.procedural_doodad_id(&key).unwrap();
            assert_eq!(world.get_doodad(id).unwrap().source, candidate.source);
        }
    }

    #[test]
    fn materialized_records_allocate_unique_doodad_ids() {
        let catalog = DoodadCatalog::default();
        let mut world = world_with_chunk(1, 1, 0.0);
        let candidates = candidates_for_chunk(7, 1, 1);
        materialize_raw(&catalog, &mut world, &candidates);

        let mut ids = std::collections::HashSet::new();
        for candidate in &candidates {
            let key = ProceduralDoodadKey::from_candidate(candidate).unwrap();
            assert!(ids.insert(world.procedural_doodad_id(&key).unwrap()));
        }
    }

    #[test]
    fn materializing_same_candidates_twice_skips_duplicates() {
        let catalog = DoodadCatalog::default();
        let mut world = world_with_chunk(4, 4, 0.0);
        let candidates = candidates_for_chunk(555, 4, 4);

        let first = materialize_raw(&catalog, &mut world, &candidates);
        let second = materialize_raw(&catalog, &mut world, &candidates);

        assert_eq!(first.inserted, candidates.len() as u32);
        assert_eq!(second.inserted, 0);
        assert_eq!(second.skipped_duplicate, candidates.len() as u32);
    }

    #[test]
    fn duplicate_key_survives_lookup_after_insertion() {
        let catalog = DoodadCatalog::default();
        let mut world = world_with_chunk(0, 0, 0.0);
        let candidates = candidates_for_chunk(12, 0, 0);
        materialize_raw(&catalog, &mut world, &candidates);

        let key = ProceduralDoodadKey::from_candidate(&candidates[0]).unwrap();
        let id = world.procedural_doodad_id(&key);
        assert!(id.is_some());
        assert!(world.get_doodad(id.unwrap()).is_some());
    }

    #[test]
    fn invalid_definition_is_skipped() {
        let catalog = DoodadCatalog::default();
        let mut world = world_with_chunk(0, 0, 0.0);
        let candidate = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("does_not_exist"),
            source: DoodadSource::Procedural { seed: 1 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(10.0, 0.0, 10.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };

        let report = materialize_raw(&catalog, &mut world, &[candidate]);

        assert_eq!(report.inserted, 0);
        assert_eq!(report.skipped_invalid_definition, 1);
    }

    #[test]
    fn disabled_definition_is_skipped() {
        let mut defs = starter_definitions();
        defs[0].enabled = false;
        let disabled_id = defs[0].id.clone();
        let catalog = DoodadCatalog::from_definitions(defs).unwrap();
        let mut world = world_with_chunk(0, 0, 0.0);

        let candidate = DoodadSpawnCandidate {
            definition_id: disabled_id,
            source: DoodadSource::Procedural { seed: 99 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(10.0, 0.0, 10.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(1.0),
        };

        let report = materialize_raw(&catalog, &mut world, &[candidate]);

        assert_eq!(report.inserted, 0);
        assert_eq!(report.skipped_disabled_definition, 1);
    }

    #[test]
    fn materialization_uses_world_data_index_correctly() {
        let catalog = DoodadCatalog::default();
        let mut world = world_with_chunk(5, 6, 0.0);
        let candidates = candidates_for_chunk(3, 5, 6);
        materialize_raw(&catalog, &mut world, &candidates);

        world.assert_doodad_index_consistent();
        world.assert_procedural_doodad_index_consistent();
    }

    #[test]
    fn materialization_report_counts_are_correct() {
        let catalog = DoodadCatalog::default();
        let mut world = world_with_chunk(0, 0, 0.0);
        let valid = candidates_for_chunk(1, 0, 0);
        let invalid = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("missing"),
            source: DoodadSource::Procedural { seed: 123 },
            position: WorldPosition::new(ChunkCoord::new(0, 0), LocalPosition::new(Vec3::ZERO)),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };

        let mut batch = valid.clone();
        batch.push(invalid);
        let report = materialize_raw(&catalog, &mut world, &batch);

        assert_eq!(report.candidates_received, batch.len() as u32);
        assert_eq!(report.inserted, valid.len() as u32);
        assert_eq!(report.skipped_invalid_definition, 1);

        let rematerialize = materialize_raw(&catalog, &mut world, &valid);
        assert_eq!(rematerialize.skipped_duplicate, valid.len() as u32);
    }

    #[test]
    fn validation_failure_skips_out_of_range_scale() {
        let catalog = DoodadCatalog::default();
        let mut world = world_with_chunk(0, 0, 0.0);
        let candidate = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 5 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(10.0, 0.0, 10.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(5.0),
        };

        let report = materialize_raw(&catalog, &mut world, &[candidate]);

        assert_eq!(report.skipped_validation_failed, 1);
        assert_eq!(report.inserted, 0);
    }

    #[test]
    fn materialization_excludes_filtered_candidates() {
        let catalog = DoodadCatalog::default();
        let mut world = world_with_chunk(0, 0, 0.0);
        let candidates = candidates_for_chunk(100, 0, 0);

        world.add_doodad_exclusion_zone(DoodadExclusionZone::new(
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
            ),
            200.0,
        ));

        let report = materialize_candidates_with_options(
            &catalog,
            &mut world,
            &candidates,
            &MaterializationOptions {
                apply_biome_filter: false,
                apply_exclusion_zones: true,
                ..MaterializationOptions::procedural_default()
            },
        );

        assert_eq!(report.candidates_received, candidates.len() as u32);
        assert_eq!(report.excluded_by_zone, candidates.len() as u32);
        assert_eq!(report.inserted, 0);
    }

    #[test]
    fn excluded_count_reported() {
        let catalog = DoodadCatalog::default();
        let mut world = world_with_chunk(0, 0, 0.0);
        let inside = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 1 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(50.0, 0.0, 50.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
        let outside = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("rock_small"),
            source: DoodadSource::Procedural { seed: 2 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(200.0, 0.0, 200.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };

        world.add_doodad_exclusion_zone(DoodadExclusionZone::new(
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(50.0, 0.0, 50.0)),
            ),
            10.0,
        ));

        let report = materialize_candidates_with_options(
            &catalog,
            &mut world,
            &[inside, outside],
            &MaterializationOptions {
                apply_exclusion_zones: true,
                ..MaterializationOptions::raw()
            },
        );

        assert_eq!(report.candidates_received, 2);
        assert_eq!(report.excluded_by_zone, 1);
        assert_eq!(report.inserted, 1);
    }

    #[test]
    fn authoring_api_unaffected_by_exclusion_zones() {
        let catalog = DoodadCatalog::default();
        let mut world = world_with_chunk(0, 0, 0.0);
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(64.0, 0.0, 64.0)),
        );

        world.add_doodad_exclusion_zone(DoodadExclusionZone::new(position, 100.0));

        let record = create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            position,
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
            None,
        )
        .unwrap();

        assert!(world.get_doodad(record.id).is_some());
    }

    fn flat_world(height: f32) -> WorldData {
        world_with_chunk(0, 0, height)
    }

    #[test]
    fn materialization_applies_exclusion_then_terrain_validation() {
        use crate::world::DoodadKind;
        use crate::world::doodad::catalog::{DoodadDefinition, DoodadRenderKey};

        let mut defs = starter_definitions();
        defs[0] = DoodadDefinition::new(
            DoodadDefinitionId::new("tree_oak"),
            DoodadKind::Tree,
            "Oak Tree",
            4.0,
            0.85,
            1.15,
            Some(100.0),
            None,
            None,
            true,
            DoodadRenderKey::reserved("tree/oak"),
        );
        let catalog = DoodadCatalog::from_definitions(defs).unwrap();
        let mut world = flat_world(50.0);

        world.add_doodad_exclusion_zone(DoodadExclusionZone::new(
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(50.0, 0.0, 50.0)),
            ),
            10.0,
        ));

        let excluded = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 1 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(50.0, 0.0, 50.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
        let height_rejected = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 2 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
        let accepted = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("rock_small"),
            source: DoodadSource::Procedural { seed: 4 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(64.0, 0.0, 64.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };

        let report = materialize_candidates_with_options(
            &catalog,
            &mut world,
            &[excluded, height_rejected, accepted],
            &MaterializationOptions {
                apply_biome_filter: false,
                apply_exclusion_zones: true,
                validate_terrain: true,
                ..MaterializationOptions::default()
            },
        );

        assert_eq!(report.candidates_received, 3);
        assert_eq!(report.excluded_by_zone, 1);
        assert_eq!(report.skipped_height_constraint, 1);
        assert_eq!(report.inserted, 1);
    }

    #[test]
    fn materialization_report_counts_terrain_rejects() {
        use crate::world::DoodadKind;
        use crate::world::doodad::catalog::{DoodadDefinition, DoodadRenderKey};

        let mut defs = starter_definitions();
        defs[0] = DoodadDefinition::new(
            DoodadDefinitionId::new("tree_oak"),
            DoodadKind::Tree,
            "Oak Tree",
            4.0,
            0.85,
            1.15,
            Some(100.0),
            None,
            None,
            true,
            DoodadRenderKey::reserved("tree/oak"),
        );
        let catalog = DoodadCatalog::from_definitions(defs).unwrap();
        let mut world = flat_world(50.0);

        let candidate = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 1 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };

        let report = materialize_candidates_with_options(
            &catalog,
            &mut world,
            &[candidate],
            &MaterializationOptions {
                apply_biome_filter: false,
                validate_terrain: true,
                ..MaterializationOptions::default()
            },
        );

        assert_eq!(report.inserted, 0);
        assert_eq!(report.skipped_height_constraint, 1);
    }

    #[test]
    fn authoring_api_unaffected_by_terrain_validation() {
        use crate::world::DoodadKind;
        use crate::world::doodad::catalog::{DoodadDefinition, DoodadRenderKey};

        let mut defs = starter_definitions();
        defs[0] = DoodadDefinition::new(
            DoodadDefinitionId::new("tree_oak"),
            DoodadKind::Tree,
            "Oak Tree",
            4.0,
            0.85,
            1.15,
            Some(100.0),
            None,
            None,
            true,
            DoodadRenderKey::reserved("tree/oak"),
        );
        let catalog = DoodadCatalog::from_definitions(defs).unwrap();
        let mut world = flat_world(50.0);
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
        );

        let record = create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            position,
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
            None,
        )
        .unwrap();

        assert!(world.get_doodad(record.id).is_some());
    }

    #[test]
    fn materialization_snaps_y_to_terrain_height() {
        let catalog = DoodadCatalog::default();
        let mut world = flat_world(37.5);
        install_uniform_forest_mask(&mut world);
        let candidate = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 8 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };

        let key = ProceduralDoodadKey::from_candidate(&candidate).unwrap();
        let report = materialize_candidates(&catalog, &mut world, &[candidate]);

        assert_eq!(report.inserted, 1);
        assert_eq!(report.terrain_snaps_applied, 1);
        let record = world
            .get_doodad(world.procedural_doodad_id(&key).unwrap())
            .unwrap();
        assert_eq!(record.placement.position.local.0.y, 37.5);
    }

    #[test]
    fn full_pipeline_exclusion_validation_placement_materialization() {
        use crate::world::DoodadKind;
        use crate::world::biome::BiomeId;
        use crate::world::doodad::catalog::{DoodadDefinition, DoodadRenderKey};

        let mut defs = starter_definitions();
        defs[0] = DoodadDefinition::new(
            DoodadDefinitionId::new("tree_oak"),
            DoodadKind::Tree,
            "Oak Tree",
            4.0,
            0.85,
            1.15,
            Some(0.0),
            Some(100.0),
            Some(45.0),
            true,
            DoodadRenderKey::reserved("tree/oak"),
        )
        .with_allowed_biomes(vec![BiomeId::Forest]);
        let catalog = DoodadCatalog::from_definitions(defs).unwrap();
        let mut world = flat_world(55.0);
        install_uniform_forest_mask(&mut world);

        world.add_doodad_exclusion_zone(DoodadExclusionZone::new(
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(50.0, 0.0, 50.0)),
            ),
            10.0,
        ));

        let excluded = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 1 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(50.0, 0.0, 50.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
        let accepted = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 2 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
            ),
            rotation: Quat::from_rotation_y(1.2),
            scale: Vec3::splat(1.05),
        };

        let report = materialize_candidates_with_options(
            &catalog,
            &mut world,
            &[excluded, accepted.clone()],
            &MaterializationOptions {
                apply_exclusion_zones: true,
                validate_terrain: true,
                snap_to_terrain: true,
                ..MaterializationOptions::default()
            },
        );

        assert_eq!(report.candidates_received, 2);
        assert_eq!(report.excluded_by_zone, 1);
        assert_eq!(report.placements_finalized, 1);
        assert_eq!(report.terrain_snaps_applied, 1);
        assert_eq!(report.inserted, 1);

        let key = ProceduralDoodadKey::from_candidate(&accepted).unwrap();
        let record = world
            .get_doodad(world.procedural_doodad_id(&key).unwrap())
            .unwrap();
        assert_eq!(record.placement.position.local.0.y, 55.0);
        // R7 catalog believability overwrites candidate rotation/scale from definition bounds.
        assert_ne!(record.placement.rotation_quat(), Quat::from_rotation_y(1.2));
        let scale_x = record.placement.scale_vec3().x;
        assert!(scale_x >= 0.85 && scale_x <= 1.15);
    }

    #[test]
    fn authoring_api_unaffected_by_placement_finalization() {
        let catalog = DoodadCatalog::default();
        let mut world = flat_world(50.0);
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(128.0, 999.0, 128.0)),
        );

        let record = create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            position,
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
            None,
        )
        .unwrap();

        assert_eq!(record.placement.position.local.0.y, 999.0);
    }

    #[test]
    fn procedural_default_materializes_valid_center_candidate() {
        let catalog = DoodadCatalog::default();
        let mut world = flat_world(0.0);
        install_uniform_forest_mask(&mut world);
        let candidate = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 11 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };

        let report = materialize_candidates(&catalog, &mut world, &[candidate]);

        assert_eq!(report.inserted, 1);
        assert_eq!(report.skipped_at_insert(), 0);
        assert_eq!(report.skipped_total(), 0);
    }

    #[test]
    fn materialization_report_counts_biome_rejects() {
        let catalog = DoodadCatalog::default();
        let mut world = flat_world(0.0);
        world.set_biome_mask(forest_desert_mask());

        let desert_tree = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 1 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(400.0, 0.0, 128.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };

        let report = materialize_candidates_with_options(
            &catalog,
            &mut world,
            &[desert_tree],
            &MaterializationOptions {
                apply_exclusion_zones: false,
                validate_terrain: false,
                ..MaterializationOptions::procedural_default()
            },
        );

        assert_eq!(report.skipped_biome_disallowed, 1);
        assert_eq!(report.inserted, 0);
    }

    #[test]
    fn forest_tree_materializes_under_procedural_default() {
        let catalog = DoodadCatalog::default();
        let mut world = flat_world(0.0);
        world.set_biome_mask(forest_desert_mask());

        let forest_tree = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 2 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(64.0, 0.0, 128.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };

        let report = materialize_candidates_with_options(
            &catalog,
            &mut world,
            &[forest_tree],
            &MaterializationOptions {
                apply_exclusion_zones: false,
                validate_terrain: false,
                ..MaterializationOptions::procedural_default()
            },
        );

        assert_eq!(report.inserted, 1);
        assert_eq!(report.skipped_biome_disallowed, 0);
    }

    #[test]
    fn missing_biome_mask_skips_under_procedural_default() {
        let catalog = DoodadCatalog::default();
        let mut world = flat_world(0.0);

        let candidate = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("rock_small"),
            source: DoodadSource::Procedural { seed: 3 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(64.0, 0.0, 128.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };

        let report = materialize_candidates(&catalog, &mut world, &[candidate]);

        assert_eq!(report.skipped_biome_unavailable, 1);
        assert_eq!(report.inserted, 0);
    }

    #[test]
    fn raw_materialization_skips_biome_filter_without_mask() {
        let catalog = DoodadCatalog::default();
        let mut world = flat_world(0.0);

        let candidate = DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("rock_small"),
            source: DoodadSource::Procedural { seed: 4 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(64.0, 0.0, 128.0)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };

        let report = materialize_raw(&catalog, &mut world, &[candidate]);

        assert_eq!(report.skipped_biome_unavailable, 0);
        assert_eq!(report.inserted, 1);
    }
}
