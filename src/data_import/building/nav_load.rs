//! Dev navigation blueprint generation during building import (NV1.2).

use std::path::Path;

use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_line};
use crate::world::{
    BuildingCatalog, BuildingNavigationBlueprintCatalog, NavigationBlueprintGenerationStatus,
    BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH, import_navigation_blueprints_for_catalog,
    load_building_navigation_blueprint_catalog,
};

const SESSION_HEADER: &str = "# chasma dev startup log";
const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

/// Generate or refresh navigation blueprints for imported buildings.
pub fn resolve_dev_navigation_blueprint_catalog(
    buildings: &BuildingCatalog,
) -> BuildingNavigationBlueprintCatalog {
    let existing = BuildingNavigationBlueprintCatalog::load_from_ron_path(&Path::new(MANIFEST_DIR).join(
        BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH,
    ))
    .unwrap_or_else(|_| load_building_navigation_blueprint_catalog());

    let (catalog, reports) = import_navigation_blueprints_for_catalog(buildings, existing);

    append_log_line(
        DEV_STARTUP_LOG_PATH,
        SESSION_HEADER,
        &format!(
            "Navigation blueprint import: {} reports (generated/cached/skipped/failed)",
            reports.len()
        ),
    );
    for report in &reports {
        if report.status == NavigationBlueprintGenerationStatus::Failed {
            for error in &report.errors {
                append_log_line(
                    DEV_STARTUP_LOG_PATH,
                    SESSION_HEADER,
                    &format!(
                        "Navigation blueprint {} for {}: {error}",
                        report.blueprint_id, report.building_id
                    ),
                );
            }
        }
        for warning in &report.warnings {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Navigation blueprint {} warning: {warning}",
                    report.blueprint_id
                ),
            );
        }
    }

    catalog
}
