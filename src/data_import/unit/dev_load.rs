//! Dev-only unit catalog resolution from Excel import (ADR-027 U1).

use std::path::Path;

use crate::data_import::paths::dev_design_workbook_path;
use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_line};
use crate::world::{FactionCatalog, SpeciesCatalog, UnitCatalog};

use super::import_units_from_excel;
use crate::data_import::DataImportError;

const SESSION_HEADER: &str = "# chasma dev startup log";

/// Load [`UnitCatalog`] for dev startup from the design workbook `Units` sheet.
pub fn resolve_dev_unit_catalog(
    factions: &FactionCatalog,
    species: &SpeciesCatalog,
    weapons: &crate::world::WeaponCatalog,
    animation_profiles: &crate::world::AnimationProfileCatalog,
    inventory_profiles: &crate::world::InventoryProfileCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
    sizing_reports: Option<&mut Vec<crate::world::AssetSizingReport>>,
) -> UnitCatalog {
    let path = dev_design_workbook_path();
    match try_import_dev_unit_catalog(
        &path,
        factions,
        species,
        weapons,
        animation_profiles,
        inventory_profiles,
        appearance_profiles,
    ) {
        Ok((catalog, summary)) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Unit Excel import ({}): processed={} valid={} failed={} warnings={}",
                    path.display(),
                    summary.rows_processed,
                    summary.rows_valid,
                    summary.rows_failed,
                    summary.warnings.len(),
                ),
            );
            for warning in &summary.warnings {
                append_log_line(
                    DEV_STARTUP_LOG_PATH,
                    SESSION_HEADER,
                    &format!("Unit import warning: {warning}"),
                );
            }
            if let Some(reports) = sizing_reports {
                reports.extend(summary.sizing_reports);
            }
            let ids: Vec<_> = catalog
                .definitions()
                .iter()
                .map(|def| def.id.as_str())
                .collect();
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!("Unit catalog ids: {}", ids.join(", ")),
            );
            let renderable: Vec<_> = catalog
                .definitions()
                .iter()
                .filter(|def| def.render_key.0.is_some())
                .map(|def| def.id.as_str())
                .collect();
            if renderable.is_empty() {
                append_log_line(
                    DEV_STARTUP_LOG_PATH,
                    SESSION_HEADER,
                    "Unit catalog has no renderable definitions (add a `File Path` column to the \
                     Units sheet, e.g. `\\units\\robot.glb` on the robot row)",
                );
            } else {
                append_log_line(
                    DEV_STARTUP_LOG_PATH,
                    SESSION_HEADER,
                    &format!("Unit catalog render keys: {}", renderable.join(", ")),
                );
            }
            catalog
        }
        Err(err) => {
            append_log_line(
                DEV_STARTUP_LOG_PATH,
                SESSION_HEADER,
                &format!(
                    "Unit Excel import failed for {} ({err}); dev unit catalog is empty",
                    path.display()
                ),
            );
            UnitCatalog::from_definitions(Vec::new()).expect("empty unit catalog is valid")
        }
    }
}

fn try_import_dev_unit_catalog(
    path: &Path,
    factions: &FactionCatalog,
    species: &SpeciesCatalog,
    weapons: &crate::world::WeaponCatalog,
    animation_profiles: &crate::world::AnimationProfileCatalog,
    inventory_profiles: &crate::world::InventoryProfileCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
) -> Result<(UnitCatalog, crate::data_import::ImportSummary), DataImportError> {
    let (definitions, summary) = import_units_from_excel(
        path,
        factions,
        species,
        weapons,
        animation_profiles,
        inventory_profiles,
        appearance_profiles,
    )?;
    let catalog = UnitCatalog::from_definitions(definitions).map_err(|err| {
        DataImportError::WorkbookOpen(format!("unit catalog build failed: {err:?}"))
    })?;
    Ok((catalog, summary))
}

#[cfg(all(test, feature = "data-import"))]
mod runtime_catalog_tests {
    use super::*;
    use crate::data_import::{
        resolve_dev_animation_profile_catalog, resolve_dev_appearance_profile_catalog,
        resolve_dev_faction_catalog, resolve_dev_inventory_profile_catalog,
        resolve_dev_species_catalog, resolve_dev_weapon_catalog,
    };
    use crate::world::{AnimationProfileId, UnitDefinitionId};

    fn resolve_dev_runtime_unit_catalog() -> UnitCatalog {
        let factions = resolve_dev_faction_catalog();
        let species = resolve_dev_species_catalog();
        let weapons = resolve_dev_weapon_catalog();
        let animation_profiles = resolve_dev_animation_profile_catalog();
        let inventory_profiles = resolve_dev_inventory_profile_catalog();
        let appearance_profiles = resolve_dev_appearance_profile_catalog();
        resolve_dev_unit_catalog(
            &factions,
            &species,
            &weapons,
            &animation_profiles,
            &inventory_profiles,
            &appearance_profiles,
            None,
        )
    }

    #[test]
    fn dev_runtime_unit_catalog_contains_human_player_units() {
        if !dev_design_workbook_path().is_file() {
            return;
        }

        let catalog = resolve_dev_runtime_unit_catalog();
        for id in ["U-0004", "U-0005"] {
            let def = catalog
                .get(&UnitDefinitionId::new(id))
                .expect("human unit missing from dev runtime catalog");
            assert!(def.enabled);
            assert_eq!(
                def.animation_profile_id
                    .as_ref()
                    .map(|profile| profile.as_str()),
                Some("human_base"),
            );
            assert!(def.render_key.0.is_some());
        }

        let animation_profiles = resolve_dev_animation_profile_catalog();
        let human_base = animation_profiles
            .get(&AnimationProfileId::new("human_base"))
            .expect("human_base profile in dev runtime");
        assert_eq!(human_base.work_clip.as_deref(), Some("Mine"));
    }

    #[test]
    fn design_workbook_imports_without_statistics_footer_base_hp_errors() {
        if !dev_design_workbook_path().is_file() {
            return;
        }

        let factions = resolve_dev_faction_catalog();
        let species = resolve_dev_species_catalog();
        let weapons = resolve_dev_weapon_catalog();
        let animation_profiles = resolve_dev_animation_profile_catalog();
        let inventory_profiles = resolve_dev_inventory_profile_catalog();
        let appearance_profiles = resolve_dev_appearance_profile_catalog();
        let (_, summary) = try_import_dev_unit_catalog(
            &dev_design_workbook_path(),
            &factions,
            &species,
            &weapons,
            &animation_profiles,
            &inventory_profiles,
            &appearance_profiles,
        )
        .expect("dev unit import");
        assert_eq!(
            summary.rows_failed,
            0,
            "warnings={:?}",
            summary.warnings,
        );
        assert!(
            !summary
                .warnings
                .iter()
                .any(|warning| warning.contains("Base HP")),
            "statistics footer rows must not be parsed as units: {:?}",
            summary.warnings,
        );
    }
}
