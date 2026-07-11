//! Offline pre-chunked terrain asset writer (ADR-011).
//!
//! This is offline preprocessing tooling, gated behind the `terrain-import`
//! feature alongside the monolithic EXR import path (ADR-009). It serializes a
//! [`WorldData`] (typically produced by `import_world`) into the same
//! self-contained manifest + per-chunk format that the runtime loader reads.
//!
//! It is the inverse of [`super::decode`]: encoding is offline, decoding is
//! runtime, and both share the [`super::asset`] format types.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::world::{ChunkId, WorldConfig, WorldData};

use super::albedo::ChunkAlbedoGrid;
use super::asset::{
    ALBEDO_FORMAT_VERSION, AlbedoFile, CHUNK_FORMAT_VERSION, ChunkFile, MANIFEST_FORMAT_VERSION,
    Manifest, ManifestChunk, ManifestConfig, TerrainAssetError,
};

fn io_err(path: &Path, err: std::io::Error) -> TerrainAssetError {
    TerrainAssetError::Io {
        path: path.display().to_string(),
        message: err.to_string(),
    }
}

/// Write `world` to `dir` as a manifest plus one self-contained file per chunk.
///
/// Produces `dir/manifest.ron` and `dir/chunks/<x>_<z>.ron`. The manifest's
/// chunk list is sorted by coordinate so output is deterministic regardless of
/// the world's internal storage order (ADR-011 determinism).
pub fn write_world(
    dir: &Path,
    config: &WorldConfig,
    world: &WorldData,
) -> Result<(), TerrainAssetError> {
    write_world_with_albedo(dir, config, world, None)
}

/// Like [`write_world`], optionally writing albedo sidecars and manifest paths.
pub fn write_world_with_albedo(
    dir: &Path,
    config: &WorldConfig,
    world: &WorldData,
    albedo: Option<&HashMap<ChunkId, ChunkAlbedoGrid>>,
) -> Result<(), TerrainAssetError> {
    let chunks_dir = dir.join("chunks");
    fs::create_dir_all(&chunks_dir).map_err(|err| io_err(&chunks_dir, err))?;

    let mut sorted: Vec<_> = world.iter().collect();
    sorted.sort_by_key(|(id, _)| (id.coord().x, id.coord().z));

    let mut entries = Vec::with_capacity(sorted.len());
    for (id, data) in sorted {
        let coord = id.coord();
        let hf = &data.heightfield;
        let file = ChunkFile {
            version: CHUNK_FORMAT_VERSION,
            x: coord.x,
            z: coord.z,
            samples_per_edge: hf.samples_per_edge(),
            spacing_meters: hf.spacing_meters(),
            samples: hf.samples().to_vec(),
            height_min: data.metadata.height_min,
            height_max: data.metadata.height_max,
        };

        let rel = format!("chunks/{}_{}.ron", coord.x, coord.z);
        let path = dir.join(&rel);
        let text = ron::to_string(&file).map_err(|err| TerrainAssetError::Ron(err.to_string()))?;
        fs::write(&path, text).map_err(|err| io_err(&path, err))?;

        let mut entry = ManifestChunk::at(coord.x, coord.z, rel);
        if let Some(albedo_map) = albedo {
            if let Some(grid) = albedo_map.get(&id) {
                if !grid.matches_height_samples(hf.samples_per_edge()) {
                    return Err(TerrainAssetError::AlbedoDimensionMismatch {
                        path: format!("chunks/{}_{}.albedo.ron", coord.x, coord.z),
                        width: grid.samples_per_edge,
                        height: grid.samples_per_edge,
                        expected_samples_per_edge: hf.samples_per_edge() as usize,
                    });
                }
                let albedo_rel = format!("chunks/{}_{}.albedo.ron", coord.x, coord.z);
                let albedo_path = dir.join(&albedo_rel);
                let albedo_file = AlbedoFile {
                    version: ALBEDO_FORMAT_VERSION,
                    samples_per_edge: grid.samples_per_edge as u32,
                    samples: grid.data.clone(),
                };
                let albedo_text = ron::to_string(&albedo_file)
                    .map_err(|err| TerrainAssetError::Ron(err.to_string()))?;
                fs::write(&albedo_path, albedo_text).map_err(|err| io_err(&albedo_path, err))?;
                entry = entry.with_albedo(albedo_rel);
            }
        }

        entries.push(entry);
    }

    let manifest = Manifest {
        version: MANIFEST_FORMAT_VERSION,
        config: ManifestConfig {
            chunk_size_meters: config.chunk_size_meters,
            units_per_meter: config.units_per_meter,
            meters_per_sample: config.meters_per_sample,
        },
        chunks: entries,
    };
    let manifest_path = dir.join("manifest.ron");
    let text = ron::to_string(&manifest).map_err(|err| TerrainAssetError::Ron(err.to_string()))?;
    fs::write(&manifest_path, text).map_err(|err| io_err(&manifest_path, err))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::decode::decode_manifest;
    use crate::terrain::load::{load_chunk_payload_from_paths, load_world_from_manifest};
    use crate::world::{ChunkCoord, ChunkData, ChunkId, Heightfield, WorldData};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        let mut dir = std::env::temp_dir();
        dir.push(format!("chasma_write_test_{}_{}", std::process::id(), id));
        dir
    }

    fn sample_chunk(seed: f32) -> ChunkData {
        let mut samples = Vec::new();
        for row in 0..3 {
            for col in 0..3 {
                samples.push(seed + (row * 10 + col) as f32);
            }
        }
        ChunkData::new(
            Heightfield::from_samples(3, 128.0, samples).unwrap(),
            Vec::new(),
        )
    }

    fn hill_chunk(seed: f32) -> ChunkData {
        let samples = vec![
            seed,
            seed + 2.0,
            seed,
            seed + 2.0,
            seed + 12.0,
            seed + 2.0,
            seed,
            seed + 2.0,
            seed,
        ];
        ChunkData::new(
            Heightfield::from_samples(3, 128.0, samples).unwrap(),
            Vec::new(),
        )
    }

    /// Regenerates `assets/worlds/main/` from synthetic data. Run manually when
    /// the on-disk sample format changes:
    /// `cargo test --features terrain-import regenerate_committed_sample_world -- --ignored`
    #[test]
    #[ignore = "manual: regenerates committed assets/worlds/main"]
    fn regenerate_committed_sample_world() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/worlds/main");
        let config = WorldConfig::default();
        let mut world = WorldData::new(config.chunk_layout());
        world.insert(ChunkId::new(ChunkCoord::new(0, 0)), hill_chunk(0.0));
        world.insert(ChunkId::new(ChunkCoord::new(1, 0)), hill_chunk(3.0));
        write_world(&dir, &config, &world).unwrap();
    }

    #[test]
    fn write_then_load_round_trips_world_data() {
        let dir = temp_dir();
        let config = WorldConfig::default();

        let mut source = WorldData::new(config.chunk_layout());
        source.insert(ChunkId::new(ChunkCoord::new(0, 0)), sample_chunk(0.0));
        source.insert(ChunkId::new(ChunkCoord::new(1, 2)), sample_chunk(100.0));

        write_world(&dir, &config, &source).unwrap();

        let mut loaded = WorldData::new(config.chunk_layout());
        let count =
            load_world_from_manifest(&dir.join("manifest.ron"), &config, &mut loaded).unwrap();

        assert_eq!(count, 2);
        for (id, data) in source.iter() {
            assert_eq!(loaded.get(id), Some(data));
        }

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_world_with_albedo_emits_manifest_paths() {
        let dir = temp_dir();
        let config = WorldConfig::default();
        let mut source = WorldData::new(config.chunk_layout());
        source.insert(ChunkId::new(ChunkCoord::new(0, 0)), sample_chunk(0.0));

        let mut albedo = HashMap::new();
        albedo.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkAlbedoGrid::from_samples(3, vec![[0.5, 0.5, 0.5]; 9]).unwrap(),
        );

        write_world_with_albedo(&dir, &config, &source, Some(&albedo)).unwrap();

        let manifest =
            decode_manifest(&fs::read_to_string(dir.join("manifest.ron")).unwrap()).unwrap();
        assert_eq!(
            manifest.chunks[0].albedo_path.as_deref(),
            Some("chunks/0_0.albedo.ron")
        );

        let (_, payload) = load_chunk_payload_from_paths(
            &dir.join(&manifest.chunks[0].path),
            &manifest.chunks[0],
            &dir,
            &config,
        )
        .unwrap();
        assert_eq!(payload.albedo.unwrap().data[0], [0.5, 0.5, 0.5]);

        fs::remove_dir_all(&dir).ok();
    }
}
