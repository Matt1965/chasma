//! Excel / RON definition import pipeline (R6 doodads, ADR-027 units).
//!
//! Offline data only: reads authored spreadsheets, validates rows, and produces
//! catalog definitions. No ECS systems, runtime Excel dependency, or rendering coupling.

mod animation;
mod building;
mod error;
mod schema;
mod weapon;

pub mod unit;
mod validate;

#[cfg(feature = "data-import")]
mod dev_load;
#[cfg(feature = "data-import")]
pub use dev_load::{DEV_DOODAD_CATALOG_RON_PATH, resolve_dev_doodad_catalog};
#[cfg(feature = "data-import")]
mod excel;
#[cfg(feature = "data-import")]
mod paths;
#[cfg(feature = "data-import")]
mod ron;

pub use error::{DataImportError, RowImportError};
pub use schema::{
    BIOME_COLUMN, DEFINITION_ID_COLUMN_ALIASES, DoodadImportRow, RANDOM_ROTATION_COLUMN_ALIASES,
    REQUIRED_COLUMNS, normalize_doodad_definition_id, normalize_file_path,
    normalize_file_path_to_render_key, parse_biome, parse_bool_yn, parse_category,
    parse_enabled_cell,
};

#[cfg(feature = "data-import")]
pub use animation::{
    ANIMATION_PROFILES_SHEET_NAME, import_animation_profiles_from_excel,
    resolve_dev_animation_profile_catalog,
};
#[cfg(feature = "data-import")]
pub use building::{
    BUILDING_CATEGORIES_SHEET_NAME, BUILDINGS_SHEET_NAME, DEV_BUILDING_CATALOG_RON_PATH,
    import_building_catalog_from_excel, import_buildings_from_excel, resolve_dev_building_catalog,
};
pub use building::{
    BUILDING_OPTIONAL_COLUMNS, BUILDING_REQUIRED_COLUMNS, BuildingCategoryImportRow,
    BuildingImportRow, normalize_building_file_path_to_render_key,
};
/// Same workbook as [`DEV_DESIGN_WORKBOOK`]; kept for older call sites.
#[cfg(feature = "data-import")]
pub use paths::DEV_DESIGN_WORKBOOK as DEV_DOODAD_EXCEL_PATH;
/// Same workbook as [`DEV_DESIGN_WORKBOOK`]; kept for older call sites.
#[cfg(feature = "data-import")]
pub use paths::DEV_DESIGN_WORKBOOK as DEV_UNIT_EXCEL_PATH;
#[cfg(feature = "data-import")]
pub use paths::{DEV_DESIGN_WORKBOOK, dev_design_workbook_path};
#[cfg(feature = "data-import")]
pub use ron::{
    BuildingCatalogRon, BuildingCategoryRon, BuildingDefinitionRon, DoodadCatalogRon,
    DoodadDefinitionRon, export_buildings_to_ron, export_doodads_to_ron,
};
pub use unit::{
    DEFAULT_COLLISION_RADIUS_METERS, DEFAULT_MAX_SLOPE_DEGREES, DEFAULT_MOVE_SPEED_MPS,
    IGNORED_COLUMNS, OPTIONAL_COLUMNS, UnitImportRow,
};
pub use unit::{
    REQUIRED_COLUMNS as UNIT_REQUIRED_COLUMNS,
    normalize_file_path_to_render_key as normalize_unit_file_path_to_render_key,
};
#[cfg(feature = "data-import")]
pub use unit::{UNITS_SHEET_NAME, import_units_from_excel, resolve_dev_unit_catalog};
#[cfg(feature = "data-import")]
pub use weapon::{WEAPONS_SHEET_NAME, import_weapons_from_excel, resolve_dev_weapon_catalog};

/// Dev footprint catalog (inline starter footprints + optional baked RON).
#[cfg(feature = "data-import")]
pub fn resolve_dev_footprint_catalog() -> crate::world::FootprintCatalog {
    crate::world::FootprintCatalog::default()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub rows_processed: usize,
    pub rows_valid: usize,
    pub rows_failed: usize,
    pub warnings: Vec<String>,
}

/// Import doodad definitions from an Excel workbook (`Doodads` sheet).
#[cfg(feature = "data-import")]
pub fn import_doodads_from_excel(
    path: &std::path::Path,
) -> Result<(Vec<crate::world::DoodadDefinition>, ImportSummary), DataImportError> {
    use std::collections::HashMap;

    use crate::world::DoodadDefinitionId;

    use excel::read_doodad_rows;
    use validate::validate_row;

    let rows = read_doodad_rows(path)?;
    let mut summary = ImportSummary {
        rows_processed: rows.len(),
        ..ImportSummary::default()
    };
    let mut definitions = Vec::new();
    let mut seen_names: HashMap<DoodadDefinitionId, usize> = HashMap::new();

    for row_result in rows {
        let row = match row_result {
            Ok(row) => row,
            Err(row_err) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("row {}: {}", row_err.row_number, row_err.message));
                continue;
            }
        };

        if let Err(row_err) = validate_row(&row) {
            summary.rows_failed += 1;
            summary
                .warnings
                .push(format!("row {}: {}", row_err.row_number, row_err.message));
            continue;
        }

        if !row.enabled {
            summary.warnings.push(format!(
                "row {}: Enabled=false — definition excluded from catalog",
                row.row_number
            ));
            continue;
        }

        let definition = match row.to_definition() {
            Ok(definition) => definition,
            Err(message) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("row {}: {message}", row.row_number));
                continue;
            }
        };

        let id = definition.id.clone();
        if let Some(first_row) = seen_names.insert(id.clone(), row.row_number) {
            return Err(DataImportError::DuplicateName {
                name: id,
                first_row,
                duplicate_row: row.row_number,
            });
        }

        if row.enabled_was_blank {
            summary.warnings.push(format!(
                "row {}: Enabled blank — defaulting to true",
                row.row_number
            ));
        }

        definitions.push(definition);
        summary.rows_valid += 1;
    }

    if summary.rows_valid == 0 {
        return Err(DataImportError::NoValidRows);
    }

    Ok((definitions, summary))
}

#[cfg(all(feature = "data-import", test))]
mod integration_tests {
    use super::*;
    use crate::world::DoodadKind;
    use excel::DOODADS_SHEET_NAME;
    use rust_xlsxwriter::Workbook;
    use std::path::{Path, PathBuf};

    fn write_workbook(path: &Path, headers: &[&str], rows: &[Vec<&str>]) {
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.set_name(DOODADS_SHEET_NAME).unwrap();
        for (col, header) in headers.iter().enumerate() {
            sheet.write_string(0, col as u16, *header).unwrap();
        }
        for (row_idx, row) in rows.iter().enumerate() {
            for (col, value) in row.iter().enumerate() {
                sheet
                    .write_string((row_idx + 1) as u32, col as u16, *value)
                    .unwrap();
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        workbook.save(path).unwrap();
    }

    fn standard_headers() -> [&'static str; 10] {
        [
            "Name",
            "Description",
            "Category",
            "Biome",
            "File Path",
            "Min Size",
            "Max Size",
            "Spawn Weight",
            "Random Rotation (Y/N)",
            "Enabled",
        ]
    }

    fn temp_workbook(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "chasma_import_mod_{}_{}.xlsx",
            std::process::id(),
            name
        ))
    }

    #[test]
    fn import_end_to_end_from_workbook() {
        let path = temp_workbook("e2e");
        let headers = standard_headers();
        let rows = vec![vec![
            "tree_oak",
            "Oak",
            "Tree",
            "Forest",
            "tree/oak.glb",
            "0.85",
            "1.15",
            "8",
            "Y",
            "Y",
        ]];
        write_workbook(&path, &headers, &rows);
        let (definitions, summary) = import_doodads_from_excel(&path).unwrap();
        assert_eq!(summary.rows_valid, 1);
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].id.as_str(), "tree_oak");
        assert_eq!(definitions[0].kind, DoodadKind::Tree);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn disabled_rows_excluded_from_catalog() {
        let path = temp_workbook("disabled");
        let headers = standard_headers();
        let rows = vec![
            vec![
                "tree_oak",
                "Oak",
                "Tree",
                "Forest",
                "tree/oak.glb",
                "0.85",
                "1.15",
                "8",
                "Y",
                "Y",
            ],
            vec![
                "tree_dead",
                "Dead",
                "Tree",
                "Forest",
                "tree/dead.glb",
                "0.9",
                "1.1",
                "2",
                "Y",
                "N",
            ],
        ];
        write_workbook(&path, &headers, &rows);
        let (definitions, summary) = import_doodads_from_excel(&path).unwrap();
        assert_eq!(summary.rows_valid, 1);
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].id.as_str(), "tree_oak");
        assert!(summary.warnings.iter().any(|w| w.contains("Enabled=false")));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn spawn_weight_preserved_by_import() {
        let path = temp_workbook("weight");
        let headers = standard_headers();
        let rows = vec![vec![
            "tree_oak",
            "Oak",
            "Tree",
            "Forest",
            "tree/oak.glb",
            "0.85",
            "1.15",
            "12.5",
            "Y",
            "Y",
        ]];
        write_workbook(&path, &headers, &rows);
        let (definitions, _) = import_doodads_from_excel(&path).unwrap();
        assert!((definitions[0].spawn_weight - 12.5).abs() < 1e-4);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn deterministic_import_output() {
        let path = temp_workbook("deterministic");
        let headers = standard_headers();
        let rows = vec![vec![
            "tree_oak",
            "Oak",
            "Tree",
            "Forest",
            r"\doodads\tree\oak.glb",
            "0.85",
            "1.15",
            "8",
            "Y",
            "Y",
        ]];
        write_workbook(&path, &headers, &rows);
        let a = import_doodads_from_excel(&path).unwrap();
        let b = import_doodads_from_excel(&path).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.0[0].render_key.0.as_deref(), Some("tree/oak"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn basic_tree_row_imports_catalog_fields() {
        let path = temp_workbook("basic_tree");
        let headers = [
            "Name",
            "Description",
            "Category",
            "File Path",
            "Min Size",
            "Max Size",
            "Spawn Weight",
            "Random Rotation",
            "Enabled",
        ];
        let rows = vec![vec![
            "Basic Tree",
            "Basic",
            "Flora",
            r"\doodads\tree",
            "0.5",
            "1.5",
            "10",
            "Y",
            "Y",
        ]];
        write_workbook(&path, &headers, &rows);
        let (definitions, summary) = import_doodads_from_excel(&path).unwrap();
        assert_eq!(summary.rows_valid, 1);
        let def = &definitions[0];
        assert_eq!(def.id.as_str(), "basic_tree");
        assert_eq!(def.kind, DoodadKind::Tree);
        assert_eq!(def.render_key.0.as_deref(), Some("tree/oak"));
        assert!((def.spawn_weight - 10.0).abs() < 1e-4);
        assert!(def.random_rotation_y);
        assert!((def.min_scale - 0.5).abs() < 1e-4);
        assert!((def.max_scale - 1.5).abs() < 1e-4);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn duplicate_normalized_definition_ids_reject_import() {
        let path = temp_workbook("duplicate_id");
        let headers = standard_headers();
        let rows = vec![
            vec![
                "Basic Tree",
                "Oak",
                "Tree",
                "Forest",
                "tree/oak.glb",
                "0.85",
                "1.15",
                "8",
                "Y",
                "Y",
            ],
            vec![
                "basic_tree",
                "Oak 2",
                "Tree",
                "Forest",
                "tree/oak.glb",
                "0.85",
                "1.15",
                "4",
                "Y",
                "Y",
            ],
        ];
        write_workbook(&path, &headers, &rows);
        let err = import_doodads_from_excel(&path).unwrap_err();
        assert!(matches!(err, DataImportError::DuplicateName { .. }));
        let _ = std::fs::remove_file(path);
    }
}
