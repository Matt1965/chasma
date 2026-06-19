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

/// Tracks chunks where dev procedural materialization has already been attempted.
///
/// Prevents re-running generation every frame when biome filtering yields zero
/// inserts but the chunk store remains empty.
#[derive(Resource, Default, Debug)]
pub struct DevProceduralMaterializationLedger {
    attempted: HashSet<ChunkId>,
}

impl DevProceduralMaterializationLedger {
    pub fn mark_attempted(&mut self, chunk: ChunkId) {
        self.attempted.insert(chunk);
    }

    pub fn was_attempted(&self, chunk: ChunkId) -> bool {
        self.attempted.contains(&chunk)
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
        .filter(|&chunk| !ledger.was_attempted(chunk))
        .collect();

    for chunk in candidates {
        let coord = chunk.coord();
        match try_materialize_procedural_chunk_doodads(
            &catalog,
            &mut world,
            chunk,
            world_seed,
        ) {
            Some(outcome) => {
                info!(
                    "Generated doodads: chunk=({}, {}) candidates={} inserted={}",
                    coord.x, coord.z, outcome.candidates, outcome.inserted
                );
            }
            None => {}
        }
        ledger.mark_attempted(chunk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        biome::{BiomeColorMapping, BiomeMask, BiomeMaskBounds},
        terrain::Heightfield,
        ChunkCoord, ChunkData, ChunkLayout, WorldData,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn resident_world_with_chunk(x: i32, z: i32) -> WorldData {
        let mut world = WorldData::new(layout());
        let samples = vec![0.0; 9];
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(x, z)),
            ChunkData::new(heightfield, Vec::new()),
        );
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

    #[test]
    fn ledger_prevents_repeat_attempt_when_chunk_stays_empty() {
        let mut ledger = DevProceduralMaterializationLedger::default();
        let chunk = ChunkId::new(ChunkCoord::new(3, 3));

        assert!(!ledger.was_attempted(chunk));
        ledger.mark_attempted(chunk);
        assert!(ledger.was_attempted(chunk));
    }

    #[test]
    fn runtime_trigger_skips_chunks_already_populated() {
        let catalog = DoodadCatalog::default();
        let mut world = resident_world_with_chunk(2, 2);
        let chunk = ChunkId::new(ChunkCoord::new(2, 2));
        let seed = DoodadsRuntimeSettings::default().world_seed;

        try_materialize_procedural_chunk_doodads(&catalog, &mut world, chunk, seed).unwrap();
        assert!(world.doodads_in_chunk(chunk).is_some());
        assert!(!chunk_needs_procedural_materialization(&world, chunk));
    }
}
