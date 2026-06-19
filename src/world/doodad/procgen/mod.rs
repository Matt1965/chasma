//! Procedural doodad generation + materialization for a resident chunk (ADR-018/019).
//!
//! World-data only: no ECS, rendering, or streaming. Callers (e.g. dev runtime
//! trigger) decide when to invoke this after terrain residency is established.

use crate::world::doodad::catalog::DoodadCatalog;
use crate::world::doodad::generation::{
    generate_chunk_doodads, DoodadGenerationContext,
};
use crate::world::doodad::materialization::{
    materialize_candidates_with_options, DoodadMaterializationReport, MaterializationOptions,
};
use crate::world::{ChunkId, WorldData};

/// Result of a single procedural materialization pass for one chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkProceduralMaterializeOutcome {
    pub candidates: u32,
    pub inserted: u32,
}

impl ChunkProceduralMaterializeOutcome {
    pub fn from_report(report: &DoodadMaterializationReport) -> Self {
        Self {
            candidates: report.candidates_received,
            inserted: report.inserted,
        }
    }
}

/// Whether procedural materialization should run for `chunk`.
///
/// Requires resident terrain and no existing doodad records in the chunk store.
pub fn chunk_needs_procedural_materialization(world: &WorldData, chunk: ChunkId) -> bool {
    world.is_chunk_loaded(chunk) && world.doodads_in_chunk(chunk).is_none()
}

/// Generate procedural candidates for `chunk` and materialize with the production preset.
///
/// Returns `None` when [`chunk_needs_procedural_materialization`] is false.
/// Uses `world.layout()` and `world_seed` from the caller — no hardcoded world sizes.
pub fn try_materialize_procedural_chunk_doodads(
    catalog: &DoodadCatalog,
    world: &mut WorldData,
    chunk: ChunkId,
    world_seed: u64,
) -> Option<ChunkProceduralMaterializeOutcome> {
    if !chunk_needs_procedural_materialization(world, chunk) {
        return None;
    }

    let layout = world.layout();
    let ctx = DoodadGenerationContext::new(world_seed, chunk, &layout);
    let candidates = generate_chunk_doodads(&ctx, catalog);
    bevy::log::debug!(
        target: "chasma::doodad_generation",
        "chunk=({}, {}) candidates: {}",
        chunk.coord().x,
        chunk.coord().z,
        crate::world::doodad::generation::format_candidate_summary(&candidates, catalog),
    );
    let report = materialize_candidates_with_options(
        catalog,
        world,
        &candidates,
        &MaterializationOptions::procedural_default(),
    );

    Some(ChunkProceduralMaterializeOutcome::from_report(&report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::doodad::catalog::{DoodadDefinition, DoodadRenderKey};
    use crate::world::doodad::materialization::{
        materialize_candidates_with_options, MaterializationOptions,
    };
    use crate::world::doodad::authoring::DoodadPlacementOverrides;
    use bevy::prelude::Vec3;
    use crate::world::doodad::source::DoodadSource;
    use crate::world::terrain::Heightfield;
    use crate::world::{
        biome::{BiomeColorMapping, BiomeId, BiomeMask, BiomeMaskBounds},
        ChunkCoord, ChunkData, ChunkLayout, DoodadCatalog, DoodadDefinitionId, DoodadKind,
        LocalPosition, WorldPosition,
    };

    const TEST_SEED: u64 = 0x0045_4A5_5EED;

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn insert_flat_chunk(world: &mut WorldData, x: i32, z: i32, height: f32) {
        let samples_per_edge = 17;
        let spacing = 16.0;
        let sample_count = (samples_per_edge * samples_per_edge) as usize;
        let samples = vec![height; sample_count];
        let heightfield = Heightfield::from_samples(samples_per_edge, spacing, samples).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(x, z)),
            ChunkData::new(heightfield, Vec::new()),
        );
    }

    fn uniform_forest_mask(extent_units: f32) -> BiomeMask {
        BiomeMask::from_rgba_rows(
            1,
            1,
            BiomeMaskBounds::new(0.0, 0.0, extent_units, extent_units),
            &[0, 255, 0],
            3,
            &BiomeColorMapping::starter(),
        )
        .unwrap()
    }

    fn forest_desert_mask(extent_x: f32, extent_z: f32) -> BiomeMask {
        BiomeMask::from_rgba_rows(
            2,
            1,
            BiomeMaskBounds::new(0.0, 0.0, extent_x, extent_z),
            &[0, 255, 0, 255, 255, 0, 0, 255],
            4,
            &BiomeColorMapping::starter(),
        )
        .unwrap()
    }

    fn forest_only_tree_catalog() -> DoodadCatalog {
        let defs = vec![DoodadDefinition::new(
            DoodadDefinitionId::new("tree_oak"),
            DoodadKind::Tree,
            "Oak Tree",
            4.0,
            0.85,
            1.15,
            None,
            None,
            Some(25.0),
            true,
            DoodadRenderKey::reserved("tree/oak"),
        )
        .with_allowed_biomes(vec![BiomeId::Forest])];
        DoodadCatalog::from_definitions(defs).unwrap()
    }

    #[test]
    fn resident_chunk_generates_once() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat_chunk(&mut world, 4, 7, 10.0);
        world.set_biome_mask(uniform_forest_mask(8192.0));

        let chunk = ChunkId::new(ChunkCoord::new(4, 7));
        assert!(chunk_needs_procedural_materialization(&world, chunk));

        let outcome = try_materialize_procedural_chunk_doodads(&catalog, &mut world, chunk, TEST_SEED)
            .expect("first pass should run");
        assert!(outcome.candidates > 0);
        assert!(outcome.inserted > 0);
        assert!(!chunk_needs_procedural_materialization(&world, chunk));
    }

    #[test]
    fn revisiting_chunk_does_not_duplicate() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat_chunk(&mut world, 2, 2, 0.0);
        world.set_biome_mask(uniform_forest_mask(4096.0));
        let chunk = ChunkId::new(ChunkCoord::new(2, 2));

        let first = try_materialize_procedural_chunk_doodads(&catalog, &mut world, chunk, TEST_SEED)
            .unwrap();
        let count_after_first = world.doodads_in_chunk(chunk).unwrap().len();

        assert!(try_materialize_procedural_chunk_doodads(&catalog, &mut world, chunk, TEST_SEED).is_none());
        assert_eq!(world.doodads_in_chunk(chunk).unwrap().len(), count_after_first);
        assert_eq!(first.inserted, count_after_first as u32);
    }

    #[test]
    fn deterministic_generation_with_fixed_seed() {
        let catalog = DoodadCatalog::default();
        let chunk = ChunkId::new(ChunkCoord::new(5, 5));
        let layout_ref = layout();

        let ctx = DoodadGenerationContext::new(TEST_SEED, chunk, &layout_ref);
        let a = generate_chunk_doodads(&ctx, &catalog);
        let b = generate_chunk_doodads(&ctx, &catalog);
        assert_eq!(a, b);

        let mut world_a = WorldData::new(layout());
        let mut world_b = WorldData::new(layout());
        insert_flat_chunk(&mut world_a, 5, 5, 0.0);
        insert_flat_chunk(&mut world_b, 5, 5, 0.0);
        world_a.set_biome_mask(uniform_forest_mask(8192.0));
        world_b.set_biome_mask(uniform_forest_mask(8192.0));

        let out_a = try_materialize_procedural_chunk_doodads(&catalog, &mut world_a, chunk, TEST_SEED).unwrap();
        let out_b = try_materialize_procedural_chunk_doodads(&catalog, &mut world_b, chunk, TEST_SEED).unwrap();
        assert_eq!(out_a, out_b);
        assert_eq!(
            world_a.doodads_in_chunk(chunk).unwrap().len(),
            world_b.doodads_in_chunk(chunk).unwrap().len(),
        );
    }

    #[test]
    fn biome_filter_respected() {
        let catalog = forest_only_tree_catalog();
        let mut world = WorldData::new(layout());
        insert_flat_chunk(&mut world, 0, 0, 0.0);
        world.set_biome_mask(forest_desert_mask(512.0, 256.0));

        let forest_chunk = ChunkId::new(ChunkCoord::new(0, 0));
        let desert_chunk = ChunkId::new(ChunkCoord::new(1, 0));

        insert_flat_chunk(&mut world, 1, 0, 0.0);

        let forest = try_materialize_procedural_chunk_doodads(&catalog, &mut world, forest_chunk, TEST_SEED)
            .unwrap();
        let desert = try_materialize_procedural_chunk_doodads(&catalog, &mut world, desert_chunk, TEST_SEED)
            .unwrap();

        assert!(forest.inserted > 0);
        assert_eq!(desert.inserted, 0);
        assert!(desert.candidates > 0);
    }

    #[test]
    fn generated_records_inserted_into_world_data() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat_chunk(&mut world, 3, 3, 5.0);
        world.set_biome_mask(uniform_forest_mask(8192.0));
        let chunk = ChunkId::new(ChunkCoord::new(3, 3));

        let outcome = try_materialize_procedural_chunk_doodads(&catalog, &mut world, chunk, TEST_SEED).unwrap();
        let store = world.doodads_in_chunk(chunk).expect("records should exist");
        assert_eq!(store.len(), outcome.inserted as usize);

        for record in store.records() {
            assert_eq!(record.placement.position.chunk, chunk.coord());
            assert!(matches!(record.source, DoodadSource::Procedural { .. }));
        }
        world.assert_doodad_index_consistent();
    }

    #[test]
    fn procedural_key_protection_prevents_duplicates() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat_chunk(&mut world, 8, 8, 0.0);
        world.set_biome_mask(uniform_forest_mask(8192.0));
        let chunk = ChunkId::new(ChunkCoord::new(8, 8));

        let layout_ref = layout();
        let ctx = DoodadGenerationContext::new(TEST_SEED, chunk, &layout_ref);
        let candidates = generate_chunk_doodads(&ctx, &catalog);

        let first = try_materialize_procedural_chunk_doodads(&catalog, &mut world, chunk, TEST_SEED).unwrap();
        assert!(first.inserted > 0);
        let count_before = world.doodads_in_chunk(chunk).unwrap().len();

        let report = materialize_candidates_with_options(
            &catalog,
            &mut world,
            &candidates,
            &MaterializationOptions::procedural_default(),
        );
        assert_eq!(report.inserted, 0);
        assert!(report.skipped_duplicate > 0);
        assert_eq!(world.doodads_in_chunk(chunk).unwrap().len(), count_before);
    }

    #[test]
    fn skips_chunk_without_resident_terrain() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        let chunk = ChunkId::new(ChunkCoord::new(0, 0));

        assert!(!chunk_needs_procedural_materialization(&world, chunk));
        assert!(try_materialize_procedural_chunk_doodads(&catalog, &mut world, chunk, TEST_SEED).is_none());
    }

    #[test]
    fn skips_chunk_already_populated() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat_chunk(&mut world, 1, 0, 0.0);
        world.set_biome_mask(uniform_forest_mask(8192.0));
        let chunk = ChunkId::new(ChunkCoord::new(1, 0));

        use crate::world::doodad::authoring::create_doodad;
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            WorldPosition::new(
                ChunkCoord::new(1, 0),
                LocalPosition::new(Vec3::new(10.0, 0.0, 10.0)),
            ),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        assert!(!chunk_needs_procedural_materialization(&world, chunk));
        assert!(try_materialize_procedural_chunk_doodads(&catalog, &mut world, chunk, TEST_SEED).is_none());
    }

    #[test]
    fn forest_materialization_includes_weighted_mix() {
        let catalog = DoodadCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat_chunk(&mut world, 5, 5, 0.0);
        world.set_biome_mask(uniform_forest_mask(8192.0));
        let chunk = ChunkId::new(ChunkCoord::new(5, 5));
        const FOREST_MIX_SEED: u64 = 9001;

        let outcome = try_materialize_procedural_chunk_doodads(
            &catalog,
            &mut world,
            chunk,
            FOREST_MIX_SEED,
        )
        .unwrap();
        assert!(outcome.inserted > 0);

        let store = world.doodads_in_chunk(chunk).unwrap();
        let inserted_ids: std::collections::BTreeSet<_> = store
            .records()
            .iter()
            .map(|record| record.definition_id.as_str())
            .collect();
        let forest_defs = ["tree_oak", "tree_dead", "bush_scrub", "rock_small"];
        let matched = forest_defs
            .iter()
            .filter(|id| inserted_ids.contains(**id))
            .count();
        assert!(
            matched >= 3,
            "expected at least 3 forest definition types, got {inserted_ids:?}"
        );
        assert!(!inserted_ids.contains("rock_large"));
    }
}
