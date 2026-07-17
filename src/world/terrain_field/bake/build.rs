//! Build terrain field tiles from source profiles (ADR-102).

use std::path::Path;

use super::super::contract::{
    TERRAIN_FIELD_SAMPLE_SPACING_METERS, TERRAIN_FIELD_SAMPLES_PER_EDGE,
    validate_world_config_for_fields,
};
use super::super::generate::{
    GenerationContext, HeightfieldDependency, generate_chunk_tile, validate_generation_dependencies,
};
use super::super::id::TerrainFieldId;
use super::super::import::partition::{TerrainFieldWorldRaster, raster_to_layer};
use super::super::import::{
    decode_field_png_from_path, decode_field_png_with_channel, resample_imported_image,
};
use super::super::layer::TerrainFieldLayer;
use super::super::source::provenance::TerrainFieldSourceProvenance;
use super::super::source::{
    TerrainFieldSourceKind, TerrainFieldSourceProfileDefinition, TerrainFieldWorldBounds,
    generator_kind_label, target_sample_dimensions,
};
use super::super::source_error::TerrainFieldSourceError;
use super::super::tile::TerrainFieldTile;
use super::package::{PackageReport, package_field_layers};
use super::statistics::TerrainFieldStatistics;
use crate::terrain::catalog::TerrainWorldCatalog;
use crate::world::{BiomeMask, ChunkCoord, ChunkExtent, WorldConfig};

const DEFAULT_TERRAIN_HEIGHT_MANIFEST: &str = "assets/worlds/main/manifest.ron";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldBuildReport {
    pub field_id: TerrainFieldId,
    pub source_version: String,
    pub statistics: TerrainFieldStatistics,
    pub tile_count: usize,
}

#[derive(Clone, Default)]
pub struct BuildDependencies<'a> {
    pub heightfield: Option<HeightfieldDependency>,
    pub biome: Option<BiomeDependencyRef<'a>>,
    pub terrain_manifest_path: Option<&'a Path>,
}

pub struct BiomeDependencyRef<'a> {
    pub mask: &'a BiomeMask,
}

impl<'a> Clone for BiomeDependencyRef<'a> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a> Copy for BiomeDependencyRef<'a> {}

pub fn build_field_layer_from_profile(
    profile: &TerrainFieldSourceProfileDefinition,
    extent: ChunkExtent,
    config: &WorldConfig,
    deps: &BuildDependencies<'_>,
) -> Result<(TerrainFieldLayer, FieldBuildReport), TerrainFieldSourceError> {
    validate_world_config_for_fields(config)
        .map_err(|e| TerrainFieldSourceError::TargetWorldConfigMismatch(e.to_string()))?;
    profile.validate()?;

    let bounds = TerrainFieldWorldBounds::from_chunk_extent(extent, config.chunk_layout());
    let (target_w, target_h) = target_sample_dimensions(extent);

    let provenance = TerrainFieldSourceProvenance {
        profile_id: profile.id.clone(),
        field_id: profile.output_field_id.clone(),
        profile_revision: profile.profile_revision.clone(),
        generator_kind: profile
            .generated
            .as_ref()
            .map(|g| generator_kind_label(&g.generator).to_string()),
        generator_version: profile
            .generated
            .as_ref()
            .map(|g| g.generator_version)
            .unwrap_or(0),
        world_seed: profile
            .generated
            .as_ref()
            .map(|g| g.world_seed)
            .unwrap_or(0),
        input_asset_hashes: Vec::new(),
        target_resolution: (target_w, target_h),
        world_extent: extent.into(),
        world_bounds: bounds,
    };
    let source_version = provenance.source_version_hash();

    let layer = match profile.source_kind {
        TerrainFieldSourceKind::ImportedMask => {
            build_imported_layer(profile, extent, bounds, target_w, target_h, &source_version)?
        }
        TerrainFieldSourceKind::Generated => {
            build_generated_layer(profile, extent, config, bounds, deps, &source_version)?
        }
        TerrainFieldSourceKind::Combined => {
            return Err(TerrainFieldSourceError::UnsupportedSourceKind(
                "Combined".to_string(),
            ));
        }
    };

    let statistics = TerrainFieldStatistics::from_layer(&layer);
    let report = FieldBuildReport {
        field_id: profile.output_field_id.clone(),
        source_version: source_version.clone(),
        statistics,
        tile_count: layer.tile_count(),
    };
    Ok((layer, report))
}

fn build_imported_layer(
    profile: &TerrainFieldSourceProfileDefinition,
    extent: ChunkExtent,
    bounds: TerrainFieldWorldBounds,
    target_w: u32,
    target_h: u32,
    source_version: &str,
) -> Result<TerrainFieldLayer, TerrainFieldSourceError> {
    let imported = profile.imported.as_ref().ok_or_else(|| {
        TerrainFieldSourceError::InvalidSourceConfiguration("missing imported".to_string())
    })?;
    imported.validate()?;
    let path = Path::new(&imported.asset_path);
    let image = if matches!(
        imported.channel,
        super::super::source::TerrainFieldImageChannel::Luminance
    ) {
        decode_field_png_from_path(path)?
    } else {
        let bytes = std::fs::read(path)
            .map_err(|err| TerrainFieldSourceError::SourceImageMissing(err.to_string()))?;
        decode_field_png_with_channel(&bytes, imported.channel)?
    };
    let image_aspect = image.width as f32 / image.height as f32;
    let world_aspect = bounds.extent_x / bounds.extent_z;
    let samples = resample_imported_image(
        &image,
        target_w,
        target_h,
        imported.orientation,
        imported.resampling,
        &imported.remap,
        false,
        image_aspect,
        world_aspect,
    )?;
    let raster = TerrainFieldWorldRaster::from_vec(target_w, target_h, samples)?;
    raster_to_layer(
        profile.output_field_id.clone(),
        &raster,
        extent,
        source_version,
    )
}

fn build_generated_layer(
    profile: &TerrainFieldSourceProfileDefinition,
    extent: ChunkExtent,
    config: &WorldConfig,
    bounds: TerrainFieldWorldBounds,
    deps: &BuildDependencies<'_>,
    source_version: &str,
) -> Result<TerrainFieldLayer, TerrainFieldSourceError> {
    let generated = profile.generated.as_ref().ok_or_else(|| {
        TerrainFieldSourceError::InvalidSourceConfiguration("missing generated".to_string())
    })?;

    let heightfield = if generated.dependencies.iter().any(|d| {
        matches!(
            d,
            super::super::source::TerrainFieldGeneratorDependency::Heightfield
        )
    }) {
        Some(
            deps.heightfield
                .clone()
                .or_else(|| {
                    let manifest = deps
                        .terrain_manifest_path
                        .unwrap_or(Path::new(DEFAULT_TERRAIN_HEIGHT_MANIFEST));
                    let catalog = TerrainWorldCatalog::from_manifest(manifest, config).ok()?;
                    HeightfieldDependency::load_from_terrain_catalog(&catalog, extent, config).ok()
                })
                .ok_or_else(|| {
                    TerrainFieldSourceError::GeneratorDependencyMissing("Heightfield".to_string())
                })?,
        )
    } else {
        None
    };

    let biome_dep = deps
        .biome
        .as_ref()
        .map(|b| super::super::generate::BiomeDependency {
            mask: b.mask.clone(),
        });

    validate_generation_dependencies(generated, heightfield.as_ref(), biome_dep.as_ref())?;

    let gen_ctx = GenerationContext {
        field_id: &profile.output_field_id,
        profile_id: &profile.id,
        generated,
        heightfield: heightfield.as_ref(),
        biome: biome_dep.as_ref(),
    };

    let mut layer =
        TerrainFieldLayer::new(profile.output_field_id.clone(), source_version.to_string());
    for z in extent.min.z..=extent.max.z {
        for x in extent.min.x..=extent.max.x {
            let chunk = ChunkCoord::new(x, z);
            let samples = generate_chunk_tile(
                &gen_ctx,
                chunk,
                extent.min,
                TERRAIN_FIELD_SAMPLE_SPACING_METERS,
                bounds.origin_x,
                bounds.origin_z,
            )?;
            let tile = TerrainFieldTile {
                chunk,
                samples_per_edge: TERRAIN_FIELD_SAMPLES_PER_EDGE,
                sample_spacing_meters: TERRAIN_FIELD_SAMPLE_SPACING_METERS,
                samples,
                tile_revision: 1,
                source_version: source_version.to_string(),
            };
            tile.validate(&profile.output_field_id)
                .map_err(|e| TerrainFieldSourceError::TilePartitionFailed(e.to_string()))?;
            layer
                .replace_tile(tile)
                .map_err(|e| TerrainFieldSourceError::TilePartitionFailed(e.to_string()))?;
        }
    }
    layer
        .validate_shared_edges()
        .map_err(|e| TerrainFieldSourceError::SharedEdgeMismatch(e.to_string()))?;
    Ok(layer)
}

pub fn build_and_package_field(
    profile: &TerrainFieldSourceProfileDefinition,
    extent: ChunkExtent,
    config: &WorldConfig,
    output_dir: &Path,
    world_id: &str,
    deps: &BuildDependencies<'_>,
) -> Result<(FieldBuildReport, PackageReport), TerrainFieldSourceError> {
    let (layer, report) = build_field_layer_from_profile(profile, extent, config, deps)?;
    let package = package_field_layers(
        output_dir,
        world_id,
        &report.source_version,
        extent,
        config,
        &[(profile.output_field_id.clone(), layer)],
    )?;
    Ok((report, package))
}

pub fn build_and_package_all_enabled(
    profiles: &[TerrainFieldSourceProfileDefinition],
    extent: ChunkExtent,
    config: &WorldConfig,
    output_dir: &Path,
    world_id: &str,
    deps: &BuildDependencies<'_>,
) -> Result<(Vec<FieldBuildReport>, PackageReport), TerrainFieldSourceError> {
    let mut layers = Vec::new();
    let mut reports = Vec::new();
    let mut last_source_version = String::new();
    for profile in profiles.iter().filter(|p| p.enabled) {
        let (layer, report) = build_field_layer_from_profile(profile, extent, config, deps)?;
        last_source_version = report.source_version.clone();
        layers.push((profile.output_field_id.clone(), layer));
        reports.push(report);
    }
    let package = package_field_layers(
        output_dir,
        world_id,
        &last_source_version,
        extent,
        config,
        &layers,
    )?;
    Ok((reports, package))
}
