//! Dev procedural doodad materialization trigger (ADR-018/019/023 Phase 3K).
//!
//! When terrain becomes resident, generates and materializes procedural doodads
//! into [`WorldData`] so the existing runtime sync can spawn glTF entities.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::terrain::residency::ChunkResidencyTracker;
use crate::world::{
    chunk_needs_procedural_materialization, try_materialize_procedural_chunk_doodads, ChunkId,
    DoodadCatalog, WorldData,
};

use super::settings::DoodadsRuntimeSettings;

/// Tracks chunks where dev procedural materialization has completed successfully.
///
/// Only chunks with at least one inserted record are marked complete. Zero-insert
/// attempts remain retryable so a later biome mask load, asset fix, or filter
/// change can recover without restarting the session.
#[derive(Resource, Default, Debug)]
pub struct DevProceduralMaterializationLedger {
    completed: HashSet<ChunkId>,
}

impl DevProceduralMaterializationLedger {
    /// Mark a chunk complete after a successful materialization pass.
    pub fn mark_completed(&mut self, chunk: ChunkId) {
        self.completed.insert(chunk);
    }

    /// Whether a successful materialization pass already ran for this chunk.
    pub fn is_completed(&self, chunk: ChunkId) -> bool {
        self.completed.contains(&chunk)
    }
}

/// Generate and materialize procedural doodads for newly resident terrain chunks.
#[cfg(feature = "dev")]
pub fn materialize_dev_procedural_doodads(
    settings: Res<DoodadsRuntimeSettings>,
    catalog: Res<DoodadCatalog>,
    residency: Res<ChunkResidencyTracker>,
    mut world: ResMut<WorldData>,
    mut ledger: ResMut<DevProceduralMaterializationLedger>,
) {
    let world_seed = settings.world_seed;
    let candidates: Vec<ChunkId> = world
        .iter()
        .map(|(chunk, _)| chunk)
        .filter(|&chunk| residency.is_resident(chunk))
        .filter(|&chunk| chunk_needs_procedural_materialization(&world, chunk))
        .filter(|&chunk| !ledger.is_completed(chunk))
        .collect();

    for chunk in candidates {
        let coord = chunk.coord();
        let Some(outcome) = try_materialize_procedural_chunk_doodads(
            &catalog,
            &mut world,
            chunk,
            world_seed,
        ) else {
            continue;
        };

        if outcome.inserted > 0 {
            info!(
                "Generated doodads: chunk=({}, {}) candidates={} inserted={}",
                coord.x, coord.z, outcome.candidates, outcome.inserted
            );
            ledger.mark_completed(chunk);
        } else {
            debug!(
                target: "chasma::doodad_procgen",
                "chunk=({}, {}) materialization yielded zero inserts; will retry next frame",
                coord.x, coord.z,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BiomeColorMapping, BiomeMask, BiomeMaskBounds, create_doodad, Heightfield, ChunkCoord,
        ChunkData, ChunkId, ChunkLayout, DoodadDefinitionId, DoodadPlacementOverrides, DoodadSource,
        LocalPosition, WorldData, WorldPosition,
    };
    use bevy::prelude::Vec3;

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

    fn resident_world_with_chunk(x: i32, z: i32) -> WorldData {
        let mut world = WorldData::new(layout());
        insert_flat_chunk(&mut world, x, z, 0.0);
        let mask = BiomeMask::from_rgba_rows(
            1,
            1,
            BiomeMaskBounds::new(0.0, 0.0, 8192.0, 8192.0),
            &[0, 255, 0],
            3,
            &BiomeColorMapping::starter(),
        )
        .unwrap();
        world.set_biome_mask(mask);
        world
    }

    fn world_without_biome_mask(x: i32, z: i32) -> WorldData {
        let mut world = WorldData::new(layout());
        insert_flat_chunk(&mut world, x, z, 0.0);
        world
    }

    #[test]
    fn completed_chunk_is_tracked() {
        let mut ledger = DevProceduralMaterializationLedger::default();
        let chunk = ChunkId::new(ChunkCoord::new(3, 3));

        assert!(!ledger.is_completed(chunk));
        ledger.mark_completed(chunk);
        assert!(ledger.is_completed(chunk));
    }

    #[test]
    fn zero_insert_does_not_mark_completed() {
        let catalog = DoodadCatalog::default();
        let mut world = world_without_biome_mask(4, 4);
        let chunk = ChunkId::new(ChunkCoord::new(4, 4));
        let seed = DoodadsRuntimeSettings::default().world_seed;
        let ledger = DevProceduralMaterializationLedger::default();

        let outcome =
            try_materialize_procedural_chunk_doodads(&catalog, &mut world, chunk, seed).unwrap();
        assert_eq!(outcome.inserted, 0);
        assert!(world.doodads_in_chunk(chunk).is_none());
        assert!(!ledger.is_completed(chunk));
    }

    #[test]
    fn zero_insert_remains_retryable_until_mask_available() {
        let catalog = DoodadCatalog::default();
        let chunk = ChunkId::new(ChunkCoord::new(5, 5));
        let seed = DoodadsRuntimeSettings::default().world_seed;
        let mut ledger = DevProceduralMaterializationLedger::default();

        let mut world = world_without_biome_mask(5, 5);
        let first =
            try_materialize_procedural_chunk_doodads(&catalog, &mut world, chunk, seed).unwrap();
        assert_eq!(first.inserted, 0);
        assert!(!ledger.is_completed(chunk));

        let mask = BiomeMask::from_rgba_rows(
            1,
            1,
            BiomeMaskBounds::new(0.0, 0.0, 8192.0, 8192.0),
            &[0, 255, 0],
            3,
            &BiomeColorMapping::starter(),
        )
        .unwrap();
        world.set_biome_mask(mask);

        let second =
            try_materialize_procedural_chunk_doodads(&catalog, &mut world, chunk, seed).unwrap();
        assert!(second.inserted > 0);
        ledger.mark_completed(chunk);
        assert!(ledger.is_completed(chunk));
    }

    #[test]
    fn successful_insert_marks_complete_via_policy() {
        let catalog = DoodadCatalog::default();
        let mut world = resident_world_with_chunk(2, 2);
        let chunk = ChunkId::new(ChunkCoord::new(2, 2));
        let seed = DoodadsRuntimeSettings::default().world_seed;
        let mut ledger = DevProceduralMaterializationLedger::default();

        let outcome =
            try_materialize_procedural_chunk_doodads(&catalog, &mut world, chunk, seed).unwrap();
        assert!(outcome.inserted > 0);
        ledger.mark_completed(chunk);
        assert!(ledger.is_completed(chunk));
    }

    #[test]
    fn populated_chunk_is_skipped_by_needs_check() {
        let catalog = DoodadCatalog::default();
        let mut world = resident_world_with_chunk(2, 2);
        let chunk = ChunkId::new(ChunkCoord::new(2, 2));

        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            WorldPosition::new(
                ChunkCoord::new(2, 2),
                LocalPosition::new(Vec3::new(10.0, 0.0, 10.0)),
            ),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();

        assert!(!chunk_needs_procedural_materialization(&world, chunk));
    }
}
