//! Phase 2A/2B synchronous terrain loading (ADR-012).
//!
//! Reads manifest and chunk files from disk synchronously. All parsing is
//! delegated to delivery-agnostic [`super::decode`] functions.

use std::fs;
use std::path::Path;

use crate::world::{ChunkId, WorldConfig, validate_heightfield_against_config};

#[cfg(any(test, feature = "terrain-import"))]
use crate::world::WorldData;

#[cfg(any(test, feature = "terrain-import"))]
use super::albedo::TerrainChunkPayload;
#[cfg(any(test, feature = "terrain-import"))]
use super::albedo_decode::try_load_optional_albedo;
use super::asset::{ManifestChunk, ManifestConfig, TerrainAssetError};
#[cfg(any(test, feature = "terrain-import"))]
use super::catalog::authored_extent_from_entries;
#[cfg(any(test, feature = "terrain-import"))]
use super::decode::{decode_chunk, decode_chunk_payload, decode_manifest};

pub(crate) fn read_manifest_text(path: &Path) -> Result<String, TerrainAssetError> {
    fs::read_to_string(path).map_err(|err| TerrainAssetError::Io {
        path: path.display().to_string(),
        message: err.to_string(),
    })
}

pub(crate) fn config_snapshot(config: &WorldConfig) -> ManifestConfig {
    ManifestConfig {
        chunk_size_meters: config.chunk_size_meters,
        units_per_meter: config.units_per_meter,
        meters_per_sample: config.meters_per_sample,
    }
}

/// Relative tolerance for comparing a chunk tile span to `WorldConfig`.
const CHUNK_SIZE_TOLERANCE: f32 = 1e-5;

pub(crate) fn validate_loaded_chunk(
    entry: &ManifestChunk,
    id: ChunkId,
    data: &crate::world::ChunkData,
    config: &WorldConfig,
) -> Result<(), TerrainAssetError> {
    let coord = id.coord();
    if coord.x != entry.x || coord.z != entry.z {
        return Err(TerrainAssetError::ChunkCoordMismatch {
            manifest_x: entry.x,
            manifest_z: entry.z,
            file_x: coord.x,
            file_z: coord.z,
        });
    }

    let expected = config.chunk_size_meters;
    let found = data.heightfield.chunk_size_meters();
    if found < expected * (1.0 - CHUNK_SIZE_TOLERANCE)
        || found > expected * (1.0 + CHUNK_SIZE_TOLERANCE)
    {
        return Err(TerrainAssetError::ChunkSizeMismatch {
            x: coord.x,
            z: coord.z,
            expected_meters: expected,
            found_meters: found,
        });
    }

    validate_heightfield_against_config(&data.heightfield, config)
        .map_err(TerrainAssetError::Heightfield)?;

    Ok(())
}

/// Synchronously load one chunk file into a pipeline payload (height + optional albedo).
///
/// Inserts only geography into `world` when used via [`load_chunk_from_path`].
#[cfg(any(test, feature = "terrain-import"))]
pub fn load_chunk_payload_from_paths(
    chunk_path: &Path,
    entry: &ManifestChunk,
    base_dir: &Path,
    config: &WorldConfig,
) -> Result<(ChunkId, TerrainChunkPayload), TerrainAssetError> {
    let (id, mut payload) = decode_chunk_payload(&read_manifest_text(chunk_path)?)?;
    validate_loaded_chunk(entry, id, &payload.chunk_data, config)?;
    payload.albedo = try_load_optional_albedo(
        base_dir,
        entry.albedo_path.as_deref(),
        payload.chunk_data.heightfield.samples_per_edge(),
    )?;
    Ok((id, payload))
}

/// Synchronously load one chunk file into `world` (tests / terrain-import tooling only).
#[cfg(any(test, feature = "terrain-import"))]
pub fn load_chunk_from_path(
    chunk_path: &Path,
    entry: &ManifestChunk,
    config: &WorldConfig,
    world: &mut WorldData,
) -> Result<ChunkId, TerrainAssetError> {
    let (id, data) = decode_chunk(&read_manifest_text(chunk_path)?)?;
    validate_loaded_chunk(entry, id, &data, config)?;
    world.insert(id, data);
    Ok(id)
}

/// Load all chunks listed in a manifest into `world` (tests / terrain-import tooling only).
///
/// Chunk paths in the manifest are resolved relative to the manifest's own
/// directory. The manifest's embedded config snapshot is validated against
/// `config`; authored extent is set from the manifest chunk list. Returns the
/// number of chunks inserted.
#[cfg(any(test, feature = "terrain-import"))]
pub fn load_world_from_manifest(
    manifest_path: &Path,
    config: &WorldConfig,
    world: &mut WorldData,
) -> Result<usize, TerrainAssetError> {
    let manifest = decode_manifest(&read_manifest_text(manifest_path)?)?;

    let runtime = config_snapshot(config);
    if manifest.config != runtime {
        return Err(TerrainAssetError::ConfigMismatch {
            manifest: manifest.config,
            runtime,
        });
    }

    if let Some(extent) = authored_extent_from_entries(&manifest.chunks) {
        world.set_authored_extent(extent);
    }

    let base_dir = manifest_path.parent().unwrap_or(Path::new(""));
    let mut loaded = 0;
    for entry in &manifest.chunks {
        let chunk_path = base_dir.join(&entry.path);
        load_chunk_from_path(&chunk_path, entry, config, world)?;
        loaded += 1;
    }

    Ok(loaded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::asset::{
        ALBEDO_FORMAT_VERSION, AlbedoFile, CHUNK_FORMAT_VERSION, ChunkFile,
        MANIFEST_FORMAT_VERSION, Manifest, ManifestChunk, ManifestConfig,
    };
    use crate::world::{ChunkCoord, ChunkId, ChunkLayout, WorldData};
    use bevy::prelude::Vec3;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "chasma_load_test_{}_{}",
            std::process::id(),
            id
        ));
        fs::create_dir_all(dir.join("chunks")).unwrap();
        dir
    }

    fn chunk_file(x: i32, z: i32) -> ChunkFile {
        let mut samples = Vec::new();
        for row in 0..3 {
            for col in 0..3 {
                samples.push((row * 10 + col) as f32);
            }
        }
        ChunkFile {
            version: CHUNK_FORMAT_VERSION,
            x,
            z,
            samples_per_edge: 3,
            spacing_meters: 128.0,
            samples,
            height_min: 0.0,
            height_max: 22.0,
        }
    }

    fn config() -> WorldConfig {
        WorldConfig {
            meters_per_sample: 128.0,
            ..WorldConfig::default()
        }
    }

    fn write_world_fixture(dir: &Path, chunks: &[(i32, i32)]) {
        let mut entries = Vec::new();
        for &(x, z) in chunks {
            let rel = format!("chunks/{x}_{z}.ron");
            fs::write(dir.join(&rel), ron::to_string(&chunk_file(x, z)).unwrap()).unwrap();
            entries.push(ManifestChunk::at(x, z, rel));
        }
        let cfg = config();
        let manifest = Manifest {
            version: MANIFEST_FORMAT_VERSION,
            config: ManifestConfig {
                chunk_size_meters: cfg.chunk_size_meters,
                units_per_meter: cfg.units_per_meter,
                meters_per_sample: cfg.meters_per_sample,
            },
            chunks: entries,
        };
        fs::write(
            dir.join("manifest.ron"),
            ron::to_string(&manifest).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn load_chunk_from_path_inserts_one_resident() {
        let dir = temp_dir();
        write_world_fixture(&dir, &[(1, 2)]);

        let manifest = decode_manifest(&read_manifest_text(&dir.join("manifest.ron")).unwrap()).unwrap();
        let entry = &manifest.chunks[0];
        let chunk_path = dir.join(&entry.path);

        let mut world = WorldData::new(config().chunk_layout());
        world.set_authored_extent(crate::world::ChunkExtent {
            min: ChunkCoord::new(1, 2),
            max: ChunkCoord::new(1, 2),
        });

        let id = load_chunk_from_path(&chunk_path, entry, &config(), &mut world).unwrap();
        assert_eq!(id, ChunkId::new(ChunkCoord::new(1, 2)));
        assert_eq!(world.len(), 1);
        assert_eq!(world.height_at(Vec3::new(256.0 + 128.0, 0.0, 512.0 + 128.0)), Some(11.0));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn loads_listed_chunks_into_world_data() {
        let dir = temp_dir();
        write_world_fixture(&dir, &[(0, 0), (2, 3)]);

        let mut world = WorldData::new(config().chunk_layout());
        let count = load_world_from_manifest(&dir.join("manifest.ron"), &config(), &mut world)
            .unwrap();

        assert_eq!(count, 2);
        assert!(world.is_chunk_loaded(ChunkId::new(ChunkCoord::new(0, 0))));
        assert!(world.is_chunk_loaded(ChunkId::new(ChunkCoord::new(2, 3))));
        assert_eq!(world.height_at(Vec3::new(128.0, 0.0, 128.0)), Some(11.0));
        assert_eq!(
            world.extent(),
            Some(crate::world::ChunkExtent {
                min: ChunkCoord::new(0, 0),
                max: ChunkCoord::new(2, 3),
            })
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_config_mismatch() {
        let dir = temp_dir();
        write_world_fixture(&dir, &[(0, 0)]);

        let mismatched = WorldConfig {
            chunk_size_meters: 128.0,
            ..config()
        };
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 128.0,
            units_per_meter: 1.0,
        });
        let err =
            load_world_from_manifest(&dir.join("manifest.ron"), &mismatched, &mut world)
                .unwrap_err();
        assert!(matches!(err, TerrainAssetError::ConfigMismatch { .. }));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reports_io_error_for_missing_manifest() {
        let dir = temp_dir();
        let mut world = WorldData::new(config().chunk_layout());
        let err = load_world_from_manifest(&dir.join("nope.ron"), &config(), &mut world)
            .unwrap_err();
        assert!(matches!(err, TerrainAssetError::Io { .. }));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_manifest_chunk_coord_mismatch() {
        let dir = temp_dir();
        let mut file = chunk_file(0, 0);
        file.x = 9;
        file.z = 9;
        fs::write(
            dir.join("chunks/0_0.ron"),
            ron::to_string(&file).unwrap(),
        )
        .unwrap();
        let cfg = config();
        let manifest = Manifest {
            version: MANIFEST_FORMAT_VERSION,
            config: ManifestConfig {
                chunk_size_meters: cfg.chunk_size_meters,
                units_per_meter: cfg.units_per_meter,
                meters_per_sample: cfg.meters_per_sample,
            },
            chunks: vec![ManifestChunk::at(0, 0, "chunks/0_0.ron")],
        };
        fs::write(
            dir.join("manifest.ron"),
            ron::to_string(&manifest).unwrap(),
        )
        .unwrap();

        let mut world = WorldData::new(config().chunk_layout());
        let err =
            load_world_from_manifest(&dir.join("manifest.ron"), &config(), &mut world)
                .unwrap_err();
        assert!(matches!(
            err,
            TerrainAssetError::ChunkCoordMismatch {
                manifest_x: 0,
                manifest_z: 0,
                file_x: 9,
                file_z: 9,
            }
        ));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_chunk_size_mismatch() {
        let dir = temp_dir();
        let file = ChunkFile {
            version: CHUNK_FORMAT_VERSION,
            x: 0,
            z: 0,
            samples_per_edge: 2,
            spacing_meters: 1.0,
            samples: vec![0.0, 0.0, 0.0, 0.0],
            height_min: 0.0,
            height_max: 0.0,
        };
        fs::write(
            dir.join("chunks/0_0.ron"),
            ron::to_string(&file).unwrap(),
        )
        .unwrap();
        let cfg = config();
        let manifest = Manifest {
            version: MANIFEST_FORMAT_VERSION,
            config: ManifestConfig {
                chunk_size_meters: cfg.chunk_size_meters,
                units_per_meter: cfg.units_per_meter,
                meters_per_sample: cfg.meters_per_sample,
            },
            chunks: vec![ManifestChunk::at(0, 0, "chunks/0_0.ron")],
        };
        fs::write(
            dir.join("manifest.ron"),
            ron::to_string(&manifest).unwrap(),
        )
        .unwrap();

        let mut world = WorldData::new(config().chunk_layout());
        let err =
            load_world_from_manifest(&dir.join("manifest.ron"), &config(), &mut world)
                .unwrap_err();
        assert!(matches!(
            err,
            TerrainAssetError::ChunkSizeMismatch {
                x: 0,
                z: 0,
                expected_meters,
                found_meters,
            } if expected_meters == 256.0 && found_meters == 1.0
        ));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn loads_committed_sample_world() {
        let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/worlds/main/manifest.ron");
        let manifest = decode_manifest(&read_manifest_text(&manifest_path).unwrap()).unwrap();
        let config = WorldConfig {
            chunk_size_meters: manifest.config.chunk_size_meters,
            units_per_meter: manifest.config.units_per_meter,
            meters_per_sample: manifest.config.meters_per_sample,
        };
        let expected = manifest.chunks.len();
        let mut world = WorldData::new(config.chunk_layout());
        let count = load_world_from_manifest(&manifest_path, &config, &mut world).unwrap();
        assert_eq!(count, expected);
        assert!(world.is_chunk_loaded(ChunkId::new(ChunkCoord::new(0, 0))));
        assert_eq!(world.len(), expected);
    }

    #[test]
    fn load_chunk_payload_loads_optional_albedo_sidecar() {
        let dir = temp_dir();
        write_world_fixture(&dir, &[(0, 0)]);

        let albedo = AlbedoFile {
            version: ALBEDO_FORMAT_VERSION,
            samples_per_edge: 3,
            samples: vec![[0.2, 0.4, 0.6]; 9],
        };
        fs::write(
            dir.join("chunks/0_0.albedo.ron"),
            ron::to_string(&albedo).unwrap(),
        )
        .unwrap();

        let mut manifest = decode_manifest(&read_manifest_text(&dir.join("manifest.ron")).unwrap()).unwrap();
        manifest.chunks[0] = manifest.chunks[0].clone().with_albedo("chunks/0_0.albedo.ron");
        fs::write(
            dir.join("manifest.ron"),
            ron::to_string(&manifest).unwrap(),
        )
        .unwrap();

        let entry = &manifest.chunks[0];
        let (id, payload) = load_chunk_payload_from_paths(
            &dir.join(&entry.path),
            entry,
            &dir,
            &config(),
        )
        .unwrap();

        assert_eq!(id, ChunkId::new(ChunkCoord::new(0, 0)));
        let grid = payload.albedo.expect("albedo sidecar should load");
        assert_eq!(grid.samples_per_edge, 3);
        assert_eq!(grid.data[0], [0.2, 0.4, 0.6]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_albedo_sidecar_does_not_fail_height_load() {
        let dir = temp_dir();
        write_world_fixture(&dir, &[(0, 0)]);

        let mut manifest = decode_manifest(&read_manifest_text(&dir.join("manifest.ron")).unwrap()).unwrap();
        manifest.chunks[0] = manifest.chunks[0].clone().with_albedo("chunks/missing.albedo.ron");
        fs::write(
            dir.join("manifest.ron"),
            ron::to_string(&manifest).unwrap(),
        )
        .unwrap();

        let entry = &manifest.chunks[0];
        let (_, payload) = load_chunk_payload_from_paths(
            &dir.join(&entry.path),
            entry,
            &dir,
            &config(),
        )
        .unwrap();
        assert!(payload.albedo.is_none());

        let mut world = WorldData::new(config().chunk_layout());
        load_chunk_from_path(&dir.join(&entry.path), entry, &config(), &mut world).unwrap();
        assert_eq!(world.len(), 1);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mismatched_albedo_dimensions_fail_load() {
        let dir = temp_dir();
        write_world_fixture(&dir, &[(0, 0)]);

        let albedo = AlbedoFile {
            version: ALBEDO_FORMAT_VERSION,
            samples_per_edge: 2,
            samples: vec![[1.0, 0.0, 0.0]; 4],
        };
        fs::write(
            dir.join("chunks/0_0.albedo.ron"),
            ron::to_string(&albedo).unwrap(),
        )
        .unwrap();

        let mut manifest = decode_manifest(&read_manifest_text(&dir.join("manifest.ron")).unwrap()).unwrap();
        manifest.chunks[0] = manifest.chunks[0].clone().with_albedo("chunks/0_0.albedo.ron");
        fs::write(
            dir.join("manifest.ron"),
            ron::to_string(&manifest).unwrap(),
        )
        .unwrap();

        let entry = &manifest.chunks[0];
        let err = load_chunk_payload_from_paths(
            &dir.join(&entry.path),
            entry,
            &dir,
            &config(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TerrainAssetError::AlbedoDimensionMismatch {
                expected_samples_per_edge: 3,
                ..
            }
        ));

        fs::remove_dir_all(&dir).ok();
    }
}
