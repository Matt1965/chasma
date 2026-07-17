//! World-package terrain field bootstrap for all build modes (ADR-106 TF6).

use std::path::Path;

use bevy::prelude::*;

use super::catalog::TerrainFieldCatalog;
use super::diff::{TerrainFieldPackageDiff, diff_terrain_field_stores};
use super::fixtures::bootstrap_dev_synthetic_fields;
use super::load::{
    DEFAULT_TERRAIN_FIELD_MANIFEST_PATH, TerrainFieldLoadSummary,
    load_terrain_fields_from_manifest, try_load_terrain_fields_from_manifest,
};
use crate::world::building::terrain_assessment::{
    AssessmentRebuildReport, TerrainAssessmentCatalogs, invalidate_buildings_for_changed_fields,
    rebuild_all_building_terrain_assessments,
};
use crate::world::{ChunkExtent, WorldConfig, WorldData};

/// Outcome of loading base terrain fields into [`WorldData`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerrainFieldBootstrapOutcome {
    Loaded(TerrainFieldLoadSummary),
    MissingManifest,
    Failed(String),
    DevSyntheticFallback,
}

/// Load committed world-package terrain fields into authoritative storage.
pub fn bootstrap_world_terrain_fields(
    world: &mut WorldData,
    catalog: &TerrainFieldCatalog,
    config: &WorldConfig,
    manifest_path: &Path,
    dev_synthetic_fallback: bool,
) -> TerrainFieldBootstrapOutcome {
    match try_load_terrain_fields_from_manifest(
        world.terrain_fields_mut(),
        catalog,
        manifest_path,
        config,
    ) {
        Some(Ok(summary)) => TerrainFieldBootstrapOutcome::Loaded(summary),
        Some(Err(err)) => {
            if dev_synthetic_fallback {
                if let Some(extent) = world.extent() {
                    bootstrap_dev_synthetic_fields(
                        world.terrain_fields_mut(),
                        extent.min,
                        extent.max,
                    );
                    return TerrainFieldBootstrapOutcome::DevSyntheticFallback;
                }
            }
            TerrainFieldBootstrapOutcome::Failed(err.to_string())
        }
        None => {
            if dev_synthetic_fallback {
                if let Some(extent) = world.extent() {
                    bootstrap_dev_synthetic_fields(
                        world.terrain_fields_mut(),
                        extent.min,
                        extent.max,
                    );
                    return TerrainFieldBootstrapOutcome::DevSyntheticFallback;
                }
            }
            TerrainFieldBootstrapOutcome::MissingManifest
        }
    }
}

/// Startup system: load world-package terrain fields when extent is known.
pub fn bootstrap_terrain_fields_on_startup(
    mut world: ResMut<WorldData>,
    catalog: Res<TerrainFieldCatalog>,
    config: Res<WorldConfig>,
) {
    if world.terrain_fields().memory_bytes() > 0 {
        return;
    }
    let outcome = bootstrap_world_terrain_fields(
        &mut world,
        &catalog,
        &config,
        Path::new(DEFAULT_TERRAIN_FIELD_MANIFEST_PATH),
        cfg!(feature = "dev"),
    );
    match outcome {
        TerrainFieldBootstrapOutcome::Loaded(summary) => {
            bevy::log::info!(
                "loaded {} terrain field tiles from world package",
                summary.tiles_loaded
            );
        }
        TerrainFieldBootstrapOutcome::DevSyntheticFallback => {
            bevy::log::warn!("terrain field package unavailable; using dev synthetic fixtures");
        }
        TerrainFieldBootstrapOutcome::MissingManifest => {
            bevy::log::warn!(
                "terrain field manifest missing; field-dependent buildings unavailable"
            );
        }
        TerrainFieldBootstrapOutcome::Failed(err) => {
            bevy::log::warn!("terrain field package load failed: {err}");
        }
    }
}

/// Helper for tests/dev when extent must be set before bootstrap.
pub fn bootstrap_with_extent(
    world: &mut WorldData,
    catalog: &TerrainFieldCatalog,
    config: &WorldConfig,
    extent: ChunkExtent,
    dev_synthetic_fallback: bool,
) -> TerrainFieldBootstrapOutcome {
    world.set_authored_extent(extent);
    bootstrap_world_terrain_fields(
        world,
        catalog,
        config,
        Path::new(DEFAULT_TERRAIN_FIELD_MANIFEST_PATH),
        dev_synthetic_fallback,
    )
}

/// Reload package, diff stores, invalidate affected assessments, and rebuild (ADR-106 TF6).
pub fn reload_terrain_fields_with_invalidation(
    world: &mut WorldData,
    field_catalog: &TerrainFieldCatalog,
    config: &WorldConfig,
    assessment_catalogs: &TerrainAssessmentCatalogs<'_>,
    assessment_store: &mut crate::world::BuildingTerrainAssessmentStore,
    manifest_path: &Path,
) -> Result<
    (
        TerrainFieldLoadSummary,
        TerrainFieldPackageDiff,
        AssessmentRebuildReport,
    ),
    crate::world::terrain_field::TerrainFieldLoadError,
> {
    let before = world.terrain_fields().clone();
    world.clear_terrain_fields();
    let summary = load_terrain_fields_from_manifest(
        world.terrain_fields_mut(),
        field_catalog,
        manifest_path,
        config,
    )?;
    let after = world.terrain_fields().clone();
    let diff = diff_terrain_field_stores(&before, &after);
    let changed_fields: Vec<_> = diff.affected_field_ids().into_iter().collect();
    let _invalidated = invalidate_buildings_for_changed_fields(
        world,
        assessment_catalogs,
        assessment_store,
        &changed_fields,
    );
    let rebuild =
        rebuild_all_building_terrain_assessments(world, assessment_catalogs, assessment_store);
    Ok((summary, diff, rebuild))
}
