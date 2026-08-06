//! Dev import pipeline for navigation blueprint generation (NV1.2).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::cache::{
    NAVIGATION_BLUEPRINT_CACHE_MANIFEST_PATH, NavigationBlueprintCacheEntry,
    NavigationBlueprintCacheManifest,
};
use super::catalog::{
    BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH, BuildingNavigationBlueprintCatalog,
    BuildingNavigationBlueprintCatalogRon,
};
use super::definition::BuildingNavigationBlueprint;
use super::generate::{
    NavigationBlueprintGenerateInput, NavigationBlueprintGenerateOutput, failed_report,
    generate_navigation_blueprint, hash_asset_path, navigation_blueprint_generation_rejection,
    navigation_mesh_source_label, should_generate_navigation_blueprint,
};
use super::id::{BuildingNavigationBlueprintId, blueprint_id_for_building};
use super::mesh::load_building_mesh_for_navigation_with_fallback;
use super::report::{
    EntranceGenerationDiagnostics, GeometryGenerationDiagnostics,
    NavigationBlueprintGenerationReport, NavigationBlueprintGenerationStatus,
};
use crate::world::BuildingCatalog;
use crate::world::building::catalog::BuildingDefinition;

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

pub const NAVIGATION_BLUEPRINT_REPORT_PATH: &str = "logs/navigation_blueprint_report.md";

pub fn import_navigation_blueprints_for_catalog(
    buildings: &BuildingCatalog,
    existing: BuildingNavigationBlueprintCatalog,
) -> (
    BuildingNavigationBlueprintCatalog,
    Vec<NavigationBlueprintGenerationReport>,
) {
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
                mesh_source_label: None,
                warnings: vec!["skipped: building is not Navigable".into()],
                errors: Vec::new(),
                entrance_diagnostics: EntranceGenerationDiagnostics::default(),
                geometry_diagnostics: GeometryGenerationDiagnostics::default(),
            });
            continue;
        }

        let blueprint_id = blueprint_id_for_building(definition);
        let collision_path = collision_asset_path(definition);
        let render_path = render_asset_path(definition);
        let collision_hash = hash_asset_path(&collision_path).unwrap_or_default();
        let render_hash = render_path.as_ref().and_then(|path| hash_asset_path(path));
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
                    mesh_source_label: None,
                    warnings: Vec::new(),
                    errors: Vec::new(),
                    entrance_diagnostics: EntranceGenerationDiagnostics::default(),
                    geometry_diagnostics: GeometryGenerationDiagnostics::default(),
                });
                continue;
            }
        }

        let mesh = match load_building_mesh_for_navigation_with_fallback(
            &collision_path,
            render_path.as_deref(),
        ) {
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

        let mesh_source_label = Some(navigation_mesh_source_label(&mesh).to_string());

        match generate_navigation_blueprint(NavigationBlueprintGenerateInput {
            blueprint_id: blueprint_id.clone(),
            display_name: format!("{} Navigation", definition.display_name),
            collision_asset_path: collision_path.clone(),
            render_asset_path: render_path.clone(),
            baseline_scale: baseline_scale(definition),
            mesh,
        }) {
            Ok(output) if output.validation.valid() => {
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
                    mesh_source_label,
                    warnings: output.warnings,
                    errors: Vec::new(),
                    entrance_diagnostics: output.entrance_diagnostics,
                    geometry_diagnostics: output.geometry_diagnostics,
                });
            }
            Ok(output) => {
                reports.push(NavigationBlueprintGenerationReport {
                    building_id: definition.id.as_str().to_string(),
                    blueprint_id: blueprint_id.clone(),
                    status: NavigationBlueprintGenerationStatus::Failed,
                    mesh_source_label,
                    warnings: output.warnings,
                    errors: vec![format!(
                        "generated blueprint has {} validation errors",
                        output.validation.error_count
                    )],
                    entrance_diagnostics: output.entrance_diagnostics,
                    geometry_diagnostics: output.geometry_diagnostics,
                });
            }
            Err(err) => {
                reports.push(failed_report(definition.id.as_str(), blueprint_id, err));
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
                mesh_source_label: None,
                warnings: Vec::new(),
                errors: vec![format!("catalog merge failed: {err}")],
                entrance_diagnostics: EntranceGenerationDiagnostics::default(),
                geometry_diagnostics: GeometryGenerationDiagnostics::default(),
            });
            existing
        }
    };

    if let Err(err) = manifest.save_to_path(&manifest_path) {
        reports.push(NavigationBlueprintGenerationReport {
            building_id: "*".to_string(),
            blueprint_id: BuildingNavigationBlueprintId::new("cache_manifest"),
            status: NavigationBlueprintGenerationStatus::Failed,
            mesh_source_label: None,
            warnings: Vec::new(),
            errors: vec![format!("failed to save cache manifest: {err}")],
            entrance_diagnostics: EntranceGenerationDiagnostics::default(),
            geometry_diagnostics: GeometryGenerationDiagnostics::default(),
        });
    }

    let _ = export_navigation_blueprint_catalog(&catalog);
    let _ = super::report::export_generation_reports_markdown(
        &Path::new(MANIFEST_DIR).join(NAVIGATION_BLUEPRINT_REPORT_PATH),
        &reports,
    );

    (catalog, reports)
}

/// Mesh-slice a navigation blueprint from a building definition without persisting.
#[cfg(feature = "data-import")]
pub fn generate_navigation_blueprint_draft_for_definition(
    definition: &BuildingDefinition,
) -> Result<NavigationBlueprintGenerateOutput, String> {
    if let Some(reason) = navigation_blueprint_generation_rejection(definition) {
        return Err(reason.into());
    }

    let blueprint_id = blueprint_id_for_building(definition);
    let collision_path = collision_asset_path(definition);
    let render_path = render_asset_path(definition);

    let mesh =
        load_building_mesh_for_navigation_with_fallback(&collision_path, render_path.as_deref())
            .map_err(|err| format!("mesh load failed: {err:?}"))?;

    generate_navigation_blueprint(NavigationBlueprintGenerateInput {
        blueprint_id,
        display_name: format!("{} Navigation", definition.display_name),
        collision_asset_path: collision_path,
        render_asset_path: render_path,
        baseline_scale: baseline_scale(definition),
        mesh,
    })
    .map_err(|err| err.to_string())
}

/// Slice a navigation blueprint draft for one placed building (NV1.2.5 editor Regenerate).
///
/// Does not write catalog, cache manifest, or instance overrides — callers load the returned
/// blueprint into the editor working copy until the user saves explicitly.
#[cfg(feature = "data-import")]
pub fn regenerate_navigation_blueprint_for_building(
    building_id: crate::world::BuildingId,
    world: &crate::world::WorldData,
    building_catalog: &BuildingCatalog,
) -> Result<
    (
        NavigationBlueprintGenerationReport,
        BuildingNavigationBlueprint,
    ),
    String,
> {
    let record = world
        .get_building(building_id)
        .ok_or_else(|| format!("building #{} not found", building_id.raw()))?;
    let definition = building_catalog
        .get(&record.definition_id)
        .ok_or_else(|| format!("definition {} missing", record.definition_id.as_str()))?;

    let blueprint_id = blueprint_id_for_building(definition);
    let collision_path = collision_asset_path(definition);
    let render_path = render_asset_path(definition);
    let mesh =
        load_building_mesh_for_navigation_with_fallback(&collision_path, render_path.as_deref())
            .map_err(|err| format!("mesh load failed: {err:?}"))?;
    let mesh_source_label = navigation_mesh_source_label(&mesh).to_string();

    let output = generate_navigation_blueprint(NavigationBlueprintGenerateInput {
        blueprint_id: blueprint_id.clone(),
        display_name: format!("{} Navigation", definition.display_name),
        collision_asset_path: collision_path,
        render_asset_path: render_path,
        baseline_scale: baseline_scale(definition),
        mesh,
    })
    .map_err(|err| err.to_string())?;

    Ok((
        NavigationBlueprintGenerationReport {
            building_id: definition.id.as_str().to_string(),
            blueprint_id,
            status: if output.validation.valid() {
                NavigationBlueprintGenerationStatus::Generated
            } else {
                NavigationBlueprintGenerationStatus::Failed
            },
            mesh_source_label: Some(mesh_source_label),
            warnings: output.warnings,
            errors: if output.validation.valid() {
                Vec::new()
            } else {
                vec![format!(
                    "generated draft has {} validation errors",
                    output.validation.error_count
                )]
            },
            entrance_diagnostics: output.entrance_diagnostics,
            geometry_diagnostics: output.geometry_diagnostics,
        },
        output.blueprint,
    ))
}

/// Write the navigation blueprint catalog to the shipped asset RON.
///
/// Never writes under `cfg(test)`: the path is a fixed repository asset, so a unit test
/// reaching this would overwrite authored blueprints.
pub fn export_navigation_blueprint_catalog(
    catalog: &BuildingNavigationBlueprintCatalog,
) -> Result<(), String> {
    if cfg!(test) {
        return Ok(());
    }
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

#[cfg(all(test, feature = "data-import"))]
mod tests {
    use super::*;
    use crate::world::authoring_transform::BuildingTransformSafetyClass;
    use crate::world::building::catalog::BuildingDefinitionId;
    use crate::world::building::footprint::FootprintSpec;

    fn hut_definition() -> BuildingDefinition {
        BuildingDefinition::new(
            BuildingDefinitionId::new("hut"),
            "Hut",
            crate::world::BuildingCategoryId::new("residential"),
            crate::world::BuildingRenderKey::reserved("hut"),
            crate::world::BuildingRenderKey::reserved("hut_collision"),
            100,
            10.0,
            FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        )
    }

    #[test]
    fn draft_generation_rejects_non_navigable_building() {
        let mut definition = hut_definition();
        definition.transform_safety_class = BuildingTransformSafetyClass::DecorativeNonNavigable;
        let err = generate_navigation_blueprint_draft_for_definition(&definition).unwrap_err();
        assert!(err.contains("not Navigable"), "unexpected error: {err}");
    }

    #[test]
    fn navigable_building_without_ids_reaches_mesh_pipeline() {
        let definition = hut_definition();
        assert!(definition.interior_profile_id.is_none());
        assert!(definition.navigation_blueprint_id.is_none());
        assert!(should_generate_navigation_blueprint(&definition));

        let result = generate_navigation_blueprint_draft_for_definition(&definition);
        match result {
            Ok(output) => assert!(
                !output.blueprint.floors.is_empty() || !output.warnings.is_empty(),
                "expected floors or warnings from mesh slicing"
            ),
            Err(err) => assert!(
                !err.contains("not configured") && !err.contains("not Navigable"),
                "expected mesh/validation error, got configuration rejection: {err}"
            ),
        }
    }
}
