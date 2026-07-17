//! Atomic world-package writer for terrain fields (ADR-102).

use std::fs;
use std::path::{Path, PathBuf};

use super::super::asset::{
    TERRAIN_FIELD_MANIFEST_VERSION, TerrainFieldManifest, TerrainFieldManifestConfig,
    TerrainFieldManifestEntry, TerrainFieldTileFile,
};
use super::super::contract::{TERRAIN_FIELD_SAMPLE_SPACING_METERS, TERRAIN_FIELD_SAMPLES_PER_EDGE};
use super::super::id::TerrainFieldId;
use super::super::layer::TerrainFieldLayer;
use super::super::source_error::TerrainFieldSourceError;
use crate::world::{ChunkExtent, WorldConfig};

const TEMP_DIR_NAME: &str = ".build_tmp";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReport {
    pub tiles_written: usize,
    pub manifest_path: PathBuf,
    pub source_version: String,
}

pub fn package_field_layers(
    output_dir: &Path,
    world_id: &str,
    source_version: &str,
    extent: ChunkExtent,
    config: &WorldConfig,
    layers: &[(TerrainFieldId, TerrainFieldLayer)],
) -> Result<PackageReport, TerrainFieldSourceError> {
    let tmp = output_dir.join(TEMP_DIR_NAME);
    if tmp.exists() {
        fs::remove_dir_all(&tmp)
            .map_err(|err| TerrainFieldSourceError::TemporaryPackageWriteFailed(err.to_string()))?;
    }
    fs::create_dir_all(&tmp)
        .map_err(|err| TerrainFieldSourceError::TemporaryPackageWriteFailed(err.to_string()))?;

    let mut tiles_written = 0usize;
    let mut manifest_fields = Vec::new();
    for (field_id, layer) in layers {
        layer
            .validate_shared_edges()
            .map_err(|e| TerrainFieldSourceError::SharedEdgeMismatch(e.to_string()))?;
        let field_dir = tmp.join(field_id.as_str());
        fs::create_dir_all(&field_dir)
            .map_err(|err| TerrainFieldSourceError::TemporaryPackageWriteFailed(err.to_string()))?;
        for tile in layer.tiles.values() {
            let file = TerrainFieldTileFile::from_tile(field_id, tile);
            let path = field_dir.join(format!("{}_{}.ron", tile.chunk.x, tile.chunk.z));
            let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default())
                .map_err(|err| {
                    TerrainFieldSourceError::TemporaryPackageWriteFailed(err.to_string())
                })?;
            fs::write(&path, text).map_err(|err| {
                TerrainFieldSourceError::TemporaryPackageWriteFailed(err.to_string())
            })?;
            tiles_written += 1;
        }
        manifest_fields.push(TerrainFieldManifestEntry {
            field_id: field_id.as_str().to_string(),
            tile_dir: field_id.as_str().to_string(),
        });
    }
    manifest_fields.sort_by(|a, b| a.field_id.cmp(&b.field_id));

    let manifest = TerrainFieldManifest {
        version: TERRAIN_FIELD_MANIFEST_VERSION,
        world_id: world_id.to_string(),
        source_version: source_version.to_string(),
        config: TerrainFieldManifestConfig {
            chunk_size_meters: config.chunk_size_meters,
            sample_spacing_meters: TERRAIN_FIELD_SAMPLE_SPACING_METERS,
            samples_per_edge: TERRAIN_FIELD_SAMPLES_PER_EDGE,
        },
        fields: manifest_fields,
    };
    let manifest_text = ron::ser::to_string_pretty(&manifest, ron::ser::PrettyConfig::default())
        .map_err(|err| TerrainFieldSourceError::TemporaryPackageWriteFailed(err.to_string()))?;
    let manifest_path = tmp.join("manifest.ron");
    fs::write(&manifest_path, manifest_text)
        .map_err(|err| TerrainFieldSourceError::TemporaryPackageWriteFailed(err.to_string()))?;

    commit_package(&tmp, output_dir)?;
    let _ = extent;
    Ok(PackageReport {
        tiles_written,
        manifest_path: output_dir.join("manifest.ron"),
        source_version: source_version.to_string(),
    })
}

fn commit_package(tmp: &Path, output_dir: &Path) -> Result<(), TerrainFieldSourceError> {
    fs::create_dir_all(output_dir)
        .map_err(|err| TerrainFieldSourceError::OutputDirectoryUnavailable(err.to_string()))?;
    for entry in fs::read_dir(tmp)
        .map_err(|err| TerrainFieldSourceError::PackageCommitFailed(err.to_string()))?
    {
        let entry =
            entry.map_err(|err| TerrainFieldSourceError::PackageCommitFailed(err.to_string()))?;
        let name = entry.file_name();
        let dest = output_dir.join(name);
        if dest.exists() {
            if dest.is_dir() {
                fs::remove_dir_all(&dest).map_err(|err| {
                    TerrainFieldSourceError::StaleTileCleanupFailed(err.to_string())
                })?;
            } else {
                fs::remove_file(&dest).map_err(|err| {
                    TerrainFieldSourceError::StaleTileCleanupFailed(err.to_string())
                })?;
            }
        }
        fs::rename(entry.path(), &dest)
            .map_err(|err| TerrainFieldSourceError::PackageCommitFailed(err.to_string()))?;
    }
    fs::remove_dir_all(tmp).ok();
    Ok(())
}
