//! World-package terrain field loading (ADR-101 TF1).

use std::path::{Path, PathBuf};

use super::asset::{
    TERRAIN_FIELD_MANIFEST_VERSION, TerrainFieldManifest, decode_manifest, decode_tile,
    tile_path_for_chunk,
};
use super::catalog::TerrainFieldCatalog;
use super::contract::{
    TERRAIN_FIELD_SAMPLE_SPACING_METERS, TERRAIN_FIELD_SAMPLES_PER_EDGE,
    validate_world_config_for_fields,
};
use super::error::TerrainFieldLoadError;
use super::id::TerrainFieldId;
use super::store::TerrainFieldStore;
use crate::world::{ChunkCoord, WorldConfig};

pub const DEFAULT_TERRAIN_FIELD_MANIFEST_PATH: &str =
    "assets/worlds/main/terrain_fields/manifest.ron";

pub const TERRAIN_FIELD_CATALOG_RON_PATH: &str = "assets/terrain_fields/catalog.ron";

/// Load committed terrain field definitions for production builds.
pub fn load_terrain_field_catalog() -> TerrainFieldCatalog {
    TerrainFieldCatalog::load_from_ron_path(Path::new(TERRAIN_FIELD_CATALOG_RON_PATH))
        .unwrap_or_else(|err| {
            panic!(
                "failed to load terrain field catalog from {TERRAIN_FIELD_CATALOG_RON_PATH}: {err}"
            )
        })
}

/// Load terrain field tiles from a world-package manifest into [`TerrainFieldStore`].
pub fn load_terrain_fields_from_manifest(
    store: &mut TerrainFieldStore,
    catalog: &TerrainFieldCatalog,
    manifest_path: &Path,
    config: &WorldConfig,
) -> Result<TerrainFieldLoadSummary, TerrainFieldLoadError> {
    validate_world_config_for_fields(config)
        .map_err(|err| TerrainFieldLoadError::WorldConfigMismatch(err.to_string()))?;
    if !manifest_path.exists() {
        return Err(TerrainFieldLoadError::ManifestMissing(
            manifest_path.display().to_string(),
        ));
    }
    let text = std::fs::read_to_string(manifest_path)
        .map_err(|err| TerrainFieldLoadError::ManifestParse(err.to_string()))?;
    let manifest = decode_manifest(&text)?;
    validate_manifest_config(&manifest, config)?;
    let base_dir = manifest_path.parent().unwrap_or(Path::new(""));

    let mut temp_store = TerrainFieldStore::new();
    let mut summary = TerrainFieldLoadSummary::default();
    for entry in &manifest.fields {
        let field_id = TerrainFieldId::new(entry.field_id.trim());
        if catalog.get(&field_id).is_none() {
            summary.skipped_unknown_fields += 1;
            continue;
        }
        let tile_dir = base_dir.join(&entry.tile_dir);
        if !tile_dir.exists() {
            summary.missing_field_dirs += 1;
            continue;
        }
        for file in
            std::fs::read_dir(&tile_dir).map_err(|err| TerrainFieldLoadError::TileParse {
                path: tile_dir.display().to_string(),
                message: err.to_string(),
            })?
        {
            let file = file.map_err(|err| TerrainFieldLoadError::TileParse {
                path: tile_dir.display().to_string(),
                message: err.to_string(),
            })?;
            let path = file.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("ron") {
                continue;
            }
            let tile_text =
                std::fs::read_to_string(&path).map_err(|err| TerrainFieldLoadError::TileParse {
                    path: path.display().to_string(),
                    message: err.to_string(),
                })?;
            let tile = decode_tile(&tile_text).map_err(|mut err| {
                if let TerrainFieldLoadError::TileParse { path: _, message } = &mut err {
                    *message = format!("{} ({})", path.display(), message);
                }
                err
            })?;
            if tile.source_version != manifest.source_version {
                return Err(TerrainFieldLoadError::SourceVersionMismatch {
                    field_id: field_id.clone(),
                    manifest: manifest.source_version.clone(),
                    tile: tile.source_version.clone(),
                });
            }
            tile.validate(&field_id)
                .map_err(TerrainFieldLoadError::Storage)?;
            temp_store.replace_tile(field_id.clone(), tile, manifest.source_version.clone())?;
            summary.tiles_loaded += 1;
        }
    }
    summary.manifest_version = TERRAIN_FIELD_MANIFEST_VERSION;
    *store = temp_store;
    Ok(summary)
}

/// Try loading world-package tiles; returns `None` when manifest is absent.
pub fn try_load_terrain_fields_from_manifest(
    store: &mut TerrainFieldStore,
    catalog: &TerrainFieldCatalog,
    manifest_path: &Path,
    config: &WorldConfig,
) -> Option<Result<TerrainFieldLoadSummary, TerrainFieldLoadError>> {
    if !manifest_path.exists() {
        return None;
    }
    Some(load_terrain_fields_from_manifest(
        store,
        catalog,
        manifest_path,
        config,
    ))
}

pub fn terrain_field_tile_path(
    base_dir: &Path,
    field_id: &TerrainFieldId,
    chunk: ChunkCoord,
) -> PathBuf {
    base_dir
        .join(field_id.as_str())
        .join(tile_path_for_chunk("", chunk))
        .with_file_name(format!("{}_{}.ron", chunk.x, chunk.z))
}

fn validate_manifest_config(
    manifest: &TerrainFieldManifest,
    config: &WorldConfig,
) -> Result<(), TerrainFieldLoadError> {
    if (manifest.config.chunk_size_meters - config.chunk_size_meters).abs() > 1e-3 {
        return Err(TerrainFieldLoadError::WorldConfigMismatch(format!(
            "manifest chunk size {} != runtime {}",
            manifest.config.chunk_size_meters, config.chunk_size_meters
        )));
    }
    if (manifest.config.sample_spacing_meters - TERRAIN_FIELD_SAMPLE_SPACING_METERS).abs() > 1e-4 {
        return Err(TerrainFieldLoadError::WorldConfigMismatch(format!(
            "manifest sample spacing {} != expected {}",
            manifest.config.sample_spacing_meters, TERRAIN_FIELD_SAMPLE_SPACING_METERS
        )));
    }
    if manifest.config.samples_per_edge != TERRAIN_FIELD_SAMPLES_PER_EDGE {
        return Err(TerrainFieldLoadError::WorldConfigMismatch(format!(
            "manifest samples per edge {} != expected {}",
            manifest.config.samples_per_edge, TERRAIN_FIELD_SAMPLES_PER_EDGE
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerrainFieldLoadSummary {
    pub manifest_version: u32,
    pub tiles_loaded: usize,
    pub skipped_unknown_fields: usize,
    pub missing_field_dirs: usize,
}
