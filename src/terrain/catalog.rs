//! Manifest catalog for authored terrain chunks (ADR-012).
//!
//! Metadata only: chunk coordinates, file paths, and authored extent. No
//! height samples are loaded until synchronous on-demand chunk loading.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use crate::world::{ChunkCoord, ChunkExtent, WorldConfig};

use super::asset::{ManifestChunk, TerrainAssetError};
use super::decode::decode_manifest;
use super::load::{config_snapshot, read_manifest_text};

/// Runtime index of authored terrain chunks and their on-disk locations.
#[derive(Debug, Clone, Resource)]
pub struct TerrainWorldCatalog {
    base_dir: PathBuf,
    authored_extent: ChunkExtent,
    chunks: HashMap<ChunkCoord, ManifestChunk>,
}

impl TerrainWorldCatalog {
    /// Load a manifest from disk and build the catalog. Does not load chunk payloads.
    pub fn from_manifest(
        manifest_path: &Path,
        config: &WorldConfig,
    ) -> Result<Self, TerrainAssetError> {
        let manifest = decode_manifest(&read_manifest_text(manifest_path)?)?;

        let runtime = config_snapshot(config);
        if manifest.config != runtime {
            return Err(TerrainAssetError::ConfigMismatch {
                manifest: manifest.config,
                runtime,
            });
        }

        let authored_extent =
            authored_extent_from_entries(&manifest.chunks).ok_or(TerrainAssetError::Io {
                path: manifest_path.display().to_string(),
                message: "manifest listed no chunks".to_string(),
            })?;

        let base_dir = manifest_path
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
        let mut chunks = HashMap::with_capacity(manifest.chunks.len());
        for entry in manifest.chunks {
            let coord = ChunkCoord::new(entry.x, entry.z);
            if chunks.insert(coord, entry).is_some() {
                return Err(TerrainAssetError::Io {
                    path: manifest_path.display().to_string(),
                    message: format!(
                        "duplicate manifest entry for chunk ({}, {})",
                        coord.x, coord.z
                    ),
                });
            }
        }

        Ok(Self {
            base_dir,
            authored_extent,
            chunks,
        })
    }

    pub fn authored_extent(&self) -> ChunkExtent {
        self.authored_extent
    }

    pub fn contains(&self, coord: ChunkCoord) -> bool {
        self.chunks.contains_key(&coord)
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn get(&self, coord: ChunkCoord) -> Option<&ManifestChunk> {
        self.chunks.get(&coord)
    }

    pub fn chunk_path(&self, coord: ChunkCoord) -> Option<PathBuf> {
        self.chunks
            .get(&coord)
            .map(|entry| self.base_dir.join(&entry.path))
    }

    pub fn albedo_path(&self, coord: ChunkCoord) -> Option<PathBuf> {
        self.chunks.get(&coord).and_then(|entry| {
            entry
                .albedo_path
                .as_ref()
                .map(|rel| self.base_dir.join(rel))
        })
    }
}

/// Derive inclusive authored bounds from manifest chunk entries.
pub fn authored_extent_from_entries(chunks: &[ManifestChunk]) -> Option<ChunkExtent> {
    let first = chunks.first()?;
    let mut min = ChunkCoord::new(first.x, first.z);
    let mut max = min;
    for entry in chunks.iter().skip(1) {
        let coord = ChunkCoord::new(entry.x, entry.z);
        min = ChunkCoord::new(min.x.min(coord.x), min.z.min(coord.z));
        max = ChunkCoord::new(max.x.max(coord.x), max.z.max(coord.z));
    }
    Some(ChunkExtent { min, max })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::asset::{
        CHUNK_FORMAT_VERSION, ChunkFile, MANIFEST_FORMAT_VERSION, Manifest, ManifestChunk,
        ManifestConfig,
    };
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut dir = std::env::temp_dir();
        dir.push(format!("chasma_catalog_{}_{}", std::process::id(), id));
        fs::create_dir_all(dir.join("chunks")).unwrap();
        dir
    }

    fn chunk_file(x: i32, z: i32) -> ChunkFile {
        ChunkFile {
            version: CHUNK_FORMAT_VERSION,
            x,
            z,
            samples_per_edge: 3,
            spacing_meters: 128.0,
            samples: vec![0.0; 9],
            height_min: 0.0,
            height_max: 0.0,
        }
    }

    fn write_fixture(dir: &Path, chunks: &[(i32, i32)], config: &WorldConfig) {
        let mut entries = Vec::new();
        for &(x, z) in chunks {
            let rel = format!("chunks/{x}_{z}.ron");
            fs::write(dir.join(&rel), ron::to_string(&chunk_file(x, z)).unwrap()).unwrap();
            entries.push(ManifestChunk::at(x, z, rel));
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
        fs::write(dir.join("manifest.ron"), ron::to_string(&manifest).unwrap()).unwrap();
    }

    #[test]
    fn builds_extent_from_manifest_entries() {
        let entries = vec![ManifestChunk::at(0, 0, "a"), ManifestChunk::at(2, 3, "b")];
        assert_eq!(
            authored_extent_from_entries(&entries),
            Some(ChunkExtent {
                min: ChunkCoord::new(0, 0),
                max: ChunkCoord::new(2, 3),
            })
        );
    }

    #[test]
    fn loads_catalog_without_touching_world_data() {
        let dir = temp_dir();
        let config = WorldConfig::default();
        write_fixture(&dir, &[(0, 0), (1, 1)], &config);

        let catalog =
            TerrainWorldCatalog::from_manifest(&dir.join("manifest.ron"), &config).unwrap();
        assert_eq!(catalog.chunk_count(), 2);
        assert!(catalog.contains(ChunkCoord::new(0, 0)));
        assert!(!catalog.contains(ChunkCoord::new(5, 5)));
        assert_eq!(
            catalog.authored_extent(),
            ChunkExtent {
                min: ChunkCoord::new(0, 0),
                max: ChunkCoord::new(1, 1),
            }
        );
        assert!(catalog.chunk_path(ChunkCoord::new(0, 0)).unwrap().exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_config_mismatch() {
        let dir = temp_dir();
        let config = WorldConfig::default();
        write_fixture(&dir, &[(0, 0)], &config);

        let mismatched = WorldConfig {
            chunk_size_meters: 128.0,
            ..config
        };
        let err =
            TerrainWorldCatalog::from_manifest(&dir.join("manifest.ron"), &mismatched).unwrap_err();
        assert!(matches!(err, TerrainAssetError::ConfigMismatch { .. }));

        fs::remove_dir_all(&dir).ok();
    }
}
