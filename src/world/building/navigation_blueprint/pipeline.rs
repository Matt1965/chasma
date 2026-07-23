//! Dev import pipeline for navigation blueprint generation (NV1.2).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::cache::{
    NavigationBlueprintCacheEntry, NavigationBlueprintCacheManifest,
    NAVIGATION_BLUEPRINT_CACHE_MANIFEST_PATH,
};
use super::catalog::{
    BuildingNavigationBlueprintCatalog, BuildingNavigationBlueprintCatalogRon,
    BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH,
};
use super::generate::{
    blueprint_id_for_building, failed_report, generate_navigation_blueprint, hash_asset_path,
    should_generate_navigation_blueprint, NavigationBlueprintGenerateInput,
};
use super::id::BuildingNavigationBlueprintId;
use super::mesh::load_building_mesh_for_navigation;
use super::report::{
    NavigationBlueprintGenerationReport, NavigationBlueprintGenerationStatus,
};
use crate::world::BuildingCatalog;
use crate::world::building::catalog::BuildingDefinition;

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

pub const NAVIGATION_BLUEPRINT_REPORT_PATH: &str = "logs/navigation_blueprint_report.md";

pub fn import_navigation_blueprints_for_catalog(
    buildings: &BuildingCatalog,
    existing: BuildingNavigationBlueprintCatalog,
) -> (BuildingNavigationBlueprintCatalog, Vec<NavigationBlueprintGenerationReport>) {
    let manifest_path = Path::new(MANIFEST_DIR).join(NAVIGATION_BLUEPRINT_CACHE_MANIFEST_PATH);
    let mut manifest = NavigationBlueprintCacheManifest::load_from_path(&manifest_path);
    let mut reports = Vec::new();
    let mut definitions: BTreeMap<String, super::definition::BuildingNavigationBlueprint> =
        existing
            .definitions()
            .iter()
            .map(|def| (def.id.as_str().to_string(), def.clone()))
            .collect();

    for definition in buildings.definitions() {
        if !should_generate_navigation_blueprint(definition) {
            reports.push(NavigationBlueprintGenerationReport {
                building_id: definition.id.as_str().to_string(),
                blueprint_id: blueprint_id_for_building(definition),
                status: NavigationBlueprintGenerationStatus::Skipped,
                warnings: vec!["building not configured for interior navigation".into()],
                errors: Vec::new(),
            });
            continue;
        }

        let blueprint_id = blueprint_id_for_building(definition);
        let collision_path = collision_asset_path(definition);
        let render_path = render_asset_path(definition);
        let collision_hash = hash_asset_path(&collision_path).unwrap_or_default();
        let render_hash = render_path
            .as_ref()
            .and_then(|path| hash_asset_path(path));
        let baseline_scale_milli = baseline_scale_milli(definition);

        if manifest.is_fresh(
            &blueprint_id,
            &collision_hash,
            render_hash.as_deref(),
            baseline_scale_milli,
        ) {
            if definitions.contains_key(blueprint_id.as_str()) {
                reports.push(NavigationBlueprintGenerationReport {
                    building_id: definition.id.as_str().to_string(),
                    blueprint_id: blueprint_id.clone(),
                    status: NavigationBlueprintGenerationStatus::Cached,
                    warnings: Vec::new(),
                    errors: Vec::new(),
                });
                continue;
            }
        }

        let mesh = match load_building_mesh_for_navigation(&collision_path) {
            Ok(mesh) => mesh,
            Err(err) => {
                reports.push(failed_report(
                    definition.id.as_str(),
                    blueprint_id.clone(),
                    format!("mesh load failed for {}: {err:?}", collision_path.display()),
                ));
                continue;
            }
        };

        match generate_navigation_blueprint(NavigationBlueprintGenerateInput {
            blueprint_id: blueprint_id.clone(),
            display_name: format!("{} Navigation", definition.display_name),
            collision_asset_path: collision_path.clone(),
            render_asset_path: render_path.clone(),
            baseline_scale: baseline_scale(definition),
            mesh,
        }) {
            Ok(output) => {
                manifest.upsert(NavigationBlueprintCacheEntry {
                    blueprint_id: blueprint_id.as_str().to_string(),
                    building_definition_id: definition.id.as_str().to_string(),
                    collision_render_key: collision_render_key(definition),
                    collision_source_hash: collision_hash,
                    render_source_hash: render_hash,
                    baseline_scale_milli,
                });
                definitions.insert(blueprint_id.as_str().to_string(), output.blueprint);
                reports.push(NavigationBlueprintGenerationReport {
                    building_id: definition.id.as_str().to_string(),
                    blueprint_id,
                    status: NavigationBlueprintGenerationStatus::Generated,
                    warnings: output.warnings,
                    errors: Vec::new(),
                });
            }
            Err(err) => {
                reports.push(failed_report(
                    definition.id.as_str(),
                    blueprint_id,
                    err,
                ));
            }
        }
    }

    let catalog = match BuildingNavigationBlueprintCatalog::from_definitions(
        definitions.into_values().collect(),
    ) {
        Ok(catalog) => catalog,
        Err(err) => {
            reports.push(NavigationBlueprintGenerationReport {
                building_id: "*".to_string(),
                blueprint_id: BuildingNavigationBlueprintId::new("catalog_merge"),
                status: NavigationBlueprintGenerationStatus::Failed,
                warnings: Vec::new(),
                errors: vec![format!("catalog merge failed: {err}")],
            });
            existing
        }
    };

    if let Err(err) = manifest.save_to_path(&manifest_path) {
        reports.push(NavigationBlueprintGenerationReport {
            building_id: "*".to_string(),
            blueprint_id: BuildingNavigationBlueprintId::new("cache_manifest"),
            status: NavigationBlueprintGenerationStatus::Failed,
            warnings: Vec::new(),
            errors: vec![format!("failed to save cache manifest: {err}")],
        });
    }

    let _ = export_navigation_blueprint_catalog(&catalog);
    let _ = super::report::export_generation_reports_markdown(
        &Path::new(MANIFEST_DIR).join(NAVIGATION_BLUEPRINT_REPORT_PATH),
        &reports,
    );

    (catalog, reports)
}

/// Force-regenerate the navigation blueprint for one placed building (NV1.2.5).
#[cfg(feature = "data-import")]
pub fn regenerate_navigation_blueprint_for_building(
    building_id: crate::world::BuildingId,
    world: &crate::world::WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &mut BuildingNavigationBlueprintCatalog,
    revision: &mut super::catalog::BuildingNavigationBlueprintCatalogRevision,
) -> Result<NavigationBlueprintGenerationReport, String> {
    let record = world
        .get_building(building_id)
        .ok_or_else(|| format!("building #{} not found", building_id.raw()))?;
    let definition = building_catalog
        .get(&record.definition_id)
        .ok_or_else(|| format!("definition {} missing", record.definition_id.as_str()))?;

    if !should_generate_navigation_blueprint(definition) {
        return Err("building not configured for interior navigation".into());
    }

    let blueprint_id = blueprint_id_for_building(definition);
    let collision_path = collision_asset_path(definition);
    let render_path = render_asset_path(definition);
    let collision_hash = hash_asset_path(&collision_path).unwrap_or_default();
    let render_hash = render_path
        .as_ref()
        .and_then(|path| hash_asset_path(path));
    let baseline_scale_milli = baseline_scale_milli(definition);

    let mesh = load_building_mesh_for_navigation(&collision_path)
        .map_err(|err| format!("mesh load failed for {}: {err:?}", collision_path.display()))?;

    let output = generate_navigation_blueprint(NavigationBlueprintGenerateInput {
        blueprint_id: blueprint_id.clone(),
        display_name: format!("{} Navigation", definition.display_name),
        collision_asset_path: collision_path.clone(),
        render_asset_path: render_path.clone(),
        baseline_scale: baseline_scale(definition),
        mesh,
    })
    .map_err(|err| err.to_string())?;

    let manifest_path = Path::new(MANIFEST_DIR).join(NAVIGATION_BLUEPRINT_CACHE_MANIFEST_PATH);
    let mut manifest = NavigationBlueprintCacheManifest::load_from_path(&manifest_path);
    manifest.upsert(NavigationBlueprintCacheEntry {
        blueprint_id: blueprint_id.as_str().to_string(),
        building_definition_id: definition.id.as_str().to_string(),
        collision_render_key: collision_render_key(definition),
        collision_source_hash: collision_hash,
        render_source_hash: render_hash,
        baseline_scale_milli,
    });
    manifest
        .save_to_path(&manifest_path)
        .map_err(|err| format!("failed to save cache manifest: {err}"))?;

    nav_catalog
        .upsert(output.blueprint)
        .map_err(|err| err.to_string())?;
    export_navigation_blueprint_catalog(nav_catalog)?;
    revision.0 = revision.0.saturating_add(1);

    Ok(NavigationBlueprintGenerationReport {
        building_id: definition.id.as_str().to_string(),
        blueprint_id,
        status: NavigationBlueprintGenerationStatus::Generated,
        warnings: output.warnings,
        errors: Vec::new(),
    })
}

pub fn export_navigation_blueprint_catalog(
    catalog: &BuildingNavigationBlueprintCatalog,
) -> Result<(), String> {
    let path = Path::new(MANIFEST_DIR).join(BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let file = BuildingNavigationBlueprintCatalogRon {
        definitions: catalog.definitions().to_vec(),
    };
    let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default())
        .map_err(|err| err.to_string())?;
    let temp_path = path.with_extension("ron.tmp");
    std::fs::write(&temp_path, text).map_err(|err| {
        format!(
            "failed to write temporary catalog {}: {err}",
            temp_path.display()
        )
    })?;
    std::fs::rename(&temp_path, &path).map_err(|err| {
        format!(
            "failed to commit navigation blueprint catalog to {}: {err}",
            path.display()
        )
    })
}

fn collision_render_key(definition: &BuildingDefinition) -> String {
    definition
        .collision_render_key
        .0
        .clone()
        .or(definition.render_key.0.clone())
        .unwrap_or_default()
}

fn collision_asset_path(definition: &BuildingDefinition) -> PathBuf {
    asset_path_for_key(collision_render_key(definition).as_str())
}

fn render_asset_path(definition: &BuildingDefinition) -> Option<PathBuf> {
    definition
        .render_key
        .0
        .as_deref()
        .map(|key| asset_path_for_key(key))
}

fn asset_path_for_key(key: &str) -> PathBuf {
    Path::new(MANIFEST_DIR)
        .join("assets/buildings")
        .join(format!("{key}.glb"))
}

fn baseline_scale(definition: &BuildingDefinition) -> f32 {
    definition
        .asset_sizing
        .resolved_baseline_scale()
        .to_vec3()
        .x
        .max(f32::EPSILON)
}

fn baseline_scale_milli(definition: &BuildingDefinition) -> Option<i32> {
    let vec = definition.asset_sizing.resolved_baseline_scale().to_vec3();
    Some((vec.x * 1000.0).round() as i32)
}
