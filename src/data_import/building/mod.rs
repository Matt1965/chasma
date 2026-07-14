//! Building definition Excel import (B1).

#[cfg(feature = "data-import")]
mod dev_load;
#[cfg(feature = "data-import")]
mod excel;
mod schema;
mod validate;

pub use schema::{
    BuildingCategoryImportRow, BuildingImportRow,
    CATEGORY_REQUIRED_COLUMNS as BUILDING_CATEGORY_REQUIRED_COLUMNS,
    OPTIONAL_COLUMNS as BUILDING_OPTIONAL_COLUMNS, REQUIRED_COLUMNS as BUILDING_REQUIRED_COLUMNS,
    normalize_building_file_path_to_render_key,
};

#[cfg(feature = "data-import")]
pub use dev_load::{DEV_BUILDING_CATALOG_RON_PATH, resolve_dev_building_catalog};
#[cfg(feature = "data-import")]
pub use excel::{BUILDING_CATEGORIES_SHEET_NAME, BUILDINGS_SHEET_NAME};

#[cfg(feature = "data-import")]
pub fn import_building_categories_from_excel(
    path: &std::path::Path,
) -> Result<
    (
        Vec<crate::world::BuildingCategoryDefinition>,
        crate::data_import::ImportSummary,
    ),
    crate::data_import::DataImportError,
> {
    use std::collections::HashMap;

    use crate::world::BuildingCategoryId;

    use excel::read_building_category_rows;
    use validate::validate_category_row;

    let rows = read_building_category_rows(path)?;
    let mut summary = crate::data_import::ImportSummary {
        rows_processed: rows.len(),
        ..crate::data_import::ImportSummary::default()
    };
    let mut definitions = Vec::new();
    let mut seen_ids: HashMap<BuildingCategoryId, usize> = HashMap::new();

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

        if let Err(row_err) = validate_category_row(&row) {
            summary.rows_failed += 1;
            summary
                .warnings
                .push(format!("row {}: {}", row_err.row_number, row_err.message));
            continue;
        }

        if !row.enabled {
            summary.warnings.push(format!(
                "row {}: Enabled=false — category excluded from catalog",
                row.row_number
            ));
            continue;
        }

        let definition = row.to_definition();
        let id = definition.id.clone();
        if let Some(first_row) = seen_ids.insert(id.clone(), row.row_number) {
            return Err(
                crate::data_import::DataImportError::DuplicateBuildingCategoryId {
                    id,
                    first_row,
                    duplicate_row: row.row_number,
                },
            );
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
        return Err(crate::data_import::DataImportError::NoValidRows);
    }

    Ok((definitions, summary))
}

#[cfg(feature = "data-import")]
pub fn import_buildings_from_excel(
    path: &std::path::Path,
    categories: &crate::world::BuildingCategoryCatalog,
    inventory_profiles: &crate::world::InventoryProfileCatalog,
) -> Result<
    (
        Vec<crate::world::BuildingDefinition>,
        crate::data_import::ImportSummary,
    ),
    crate::data_import::DataImportError,
> {
    use std::collections::HashMap;

    use crate::world::BuildingDefinitionId;

    use excel::read_building_rows;
    use validate::validate_row;

    let rows = read_building_rows(path)?;
    let mut summary = crate::data_import::ImportSummary {
        rows_processed: rows.len(),
        ..crate::data_import::ImportSummary::default()
    };
    let mut definitions = Vec::new();
    let mut seen_ids: HashMap<BuildingDefinitionId, usize> = HashMap::new();

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

        if !categories.contains(&crate::world::BuildingCategoryId::new(row.category.trim())) {
            summary.rows_failed += 1;
            summary.warnings.push(format!(
                "row {}: unknown Category `{}`",
                row.row_number,
                row.category.trim()
            ));
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

        if let Some(profile_id) = &definition.inventory_profile_id {
            if let Err(err) = inventory_profiles.validate_profile_reference(
                "building",
                definition.id.as_str(),
                profile_id,
            ) {
                summary.rows_failed += 1;
                summary.warnings.push(format!(
                    "row {}: inventory profile validation: {err}",
                    row.row_number
                ));
                continue;
            }
        }

        warn_if_asset_missing(&definition, &mut summary, row.row_number);

        let id = definition.id.clone();
        if let Some(first_row) = seen_ids.insert(id.clone(), row.row_number) {
            return Err(crate::data_import::DataImportError::DuplicateBuildingId {
                id,
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
        return Err(crate::data_import::DataImportError::NoValidRows);
    }

    Ok((definitions, summary))
}

#[cfg(feature = "data-import")]
pub fn import_building_catalog_from_excel(
    path: &std::path::Path,
    inventory_profiles: &crate::world::InventoryProfileCatalog,
) -> Result<
    (
        crate::world::BuildingCategoryCatalog,
        crate::world::BuildingCatalog,
        crate::data_import::ImportSummary,
    ),
    crate::data_import::DataImportError,
> {
    let (category_defs, mut category_summary) = import_building_categories_from_excel(path)?;
    let categories = crate::world::BuildingCategoryCatalog::from_definitions(category_defs)
        .map_err(|err| {
            crate::data_import::DataImportError::WorkbookOpen(format!(
                "building category catalog build failed: {err}"
            ))
        })?;

    let (building_defs, building_summary) =
        import_buildings_from_excel(path, &categories, inventory_profiles)?;
    let buildings = crate::world::BuildingCatalog::from_definitions(building_defs, &categories)
        .map_err(|err| {
            crate::data_import::DataImportError::WorkbookOpen(format!(
                "building catalog build failed: {err}"
            ))
        })?;

    category_summary.rows_processed += building_summary.rows_processed;
    category_summary.rows_valid += building_summary.rows_valid;
    category_summary.rows_failed += building_summary.rows_failed;
    category_summary.warnings.extend(building_summary.warnings);

    Ok((categories, buildings, category_summary))
}

#[cfg(feature = "data-import")]
fn warn_if_asset_missing(
    definition: &crate::world::BuildingDefinition,
    summary: &mut crate::data_import::ImportSummary,
    row_number: usize,
) {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (label, key) in [
        ("Model", definition.render_key.0.as_deref()),
        ("Collision", definition.collision_render_key.0.as_deref()),
        (
            "Preview",
            definition
                .preview_render_key
                .as_ref()
                .and_then(|key| key.0.as_deref()),
        ),
    ] {
        let Some(key) = key else {
            continue;
        };
        let path = manifest_dir
            .join("assets/buildings")
            .join(format!("{key}.glb"));
        if !path.exists() {
            summary.warnings.push(format!(
                "row {row_number}: {label} asset missing at `{}`",
                path.display()
            ));
        }
    }
}

#[cfg(all(feature = "data-import", test))]
mod tests {
    use super::*;
    use crate::world::{BuildingCategoryCatalog, FootprintType};
    use excel::{BUILDING_CATEGORIES_SHEET_NAME, BUILDINGS_SHEET_NAME};
    use rust_xlsxwriter::Workbook;
    use std::path::Path;

    fn write_dual_sheet_workbook(
        path: &Path,
        category_headers: &[&str],
        category_rows: &[Vec<&str>],
        building_headers: &[&str],
        building_rows: &[Vec<&str>],
    ) {
        let mut workbook = Workbook::new();
        let categories = workbook.add_worksheet();
        categories.set_name(BUILDING_CATEGORIES_SHEET_NAME).unwrap();
        for (col, header) in category_headers.iter().enumerate() {
            categories.write_string(0, col as u16, *header).unwrap();
        }
        for (row_idx, row) in category_rows.iter().enumerate() {
            for (col, value) in row.iter().enumerate() {
                categories
                    .write_string((row_idx + 1) as u32, col as u16, *value)
                    .unwrap();
            }
        }
        let buildings = workbook.add_worksheet();
        buildings.set_name(BUILDINGS_SHEET_NAME).unwrap();
        for (col, header) in building_headers.iter().enumerate() {
            buildings.write_string(0, col as u16, *header).unwrap();
        }
        for (row_idx, row) in building_rows.iter().enumerate() {
            for (col, value) in row.iter().enumerate() {
                buildings
                    .write_string((row_idx + 1) as u32, col as u16, *value)
                    .unwrap();
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        workbook.save(path).unwrap();
    }

    fn category_headers() -> Vec<&'static str> {
        vec!["Category ID", "Display Name", "Description", "Enabled"]
    }

    fn building_headers() -> Vec<&'static str> {
        vec![
            "Building ID",
            "Name",
            "Category",
            "Model File Path",
            "Collision File Path",
            "Health",
            "Build Time",
            "Footprint Type",
            "Footprint Width",
            "Footprint Depth",
            "Enabled",
        ]
    }

    fn temp_workbook(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "chasma_building_import_{}_{}.xlsx",
            std::process::id(),
            name
        ))
    }

    #[test]
    fn import_end_to_end_from_workbook() {
        let path = temp_workbook("e2e");
        write_dual_sheet_workbook(
            &path,
            &category_headers(),
            &[vec!["residential", "Residential", "Shelter", "Y"]],
            &building_headers(),
            &[vec![
                "hut",
                "Hut",
                "residential",
                "hut.glb",
                "hut_collision.glb",
                "100",
                "30",
                "Rectangle",
                "4",
                "4",
                "Y",
            ]],
        );
        let profiles = crate::world::InventoryProfileCatalog::default();
        let (categories, buildings, summary) =
            import_building_catalog_from_excel(&path, &profiles).unwrap();
        assert_eq!(summary.rows_valid, 2);
        assert_eq!(categories.len(), 1);
        assert_eq!(buildings.len(), 1);
        assert_eq!(buildings.definitions()[0].id.as_str(), "hut");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn duplicate_building_ids_reject_import() {
        let path = temp_workbook("dup_building");
        write_dual_sheet_workbook(
            &path,
            &category_headers(),
            &[vec!["residential", "Residential", "", "Y"]],
            &building_headers(),
            &[
                vec![
                    "hut",
                    "Hut",
                    "residential",
                    "hut.glb",
                    "",
                    "100",
                    "30",
                    "Rectangle",
                    "4",
                    "4",
                    "Y",
                ],
                vec![
                    "hut",
                    "Hut 2",
                    "residential",
                    "hut.glb",
                    "",
                    "100",
                    "30",
                    "Rectangle",
                    "4",
                    "4",
                    "Y",
                ],
            ],
        );
        let profiles = crate::world::InventoryProfileCatalog::default();
        let err = import_building_catalog_from_excel(&path, &profiles).unwrap_err();
        assert!(matches!(
            err,
            crate::data_import::DataImportError::DuplicateBuildingId { .. }
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn unknown_category_rejected() {
        let path = temp_workbook("unknown_category");
        write_dual_sheet_workbook(
            &path,
            &category_headers(),
            &[vec!["residential", "Residential", "", "Y"]],
            &building_headers(),
            &[vec![
                "hut",
                "Hut",
                "missing",
                "hut.glb",
                "",
                "100",
                "30",
                "Rectangle",
                "4",
                "4",
                "Y",
            ]],
        );
        let err = import_buildings_from_excel(
            &path,
            &BuildingCategoryCatalog::default(),
            &crate::world::InventoryProfileCatalog::default(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            crate::data_import::DataImportError::NoValidRows
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn invalid_footprint_type_rejected_at_parse() {
        let path = temp_workbook("bad_footprint");
        write_dual_sheet_workbook(
            &path,
            &category_headers(),
            &[vec!["residential", "Residential", "", "Y"]],
            &building_headers(),
            &[vec![
                "hut",
                "Hut",
                "residential",
                "hut.glb",
                "",
                "100",
                "30",
                "Triangle",
                "4",
                "4",
                "Y",
            ]],
        );
        let rows = excel::read_building_rows(&path).unwrap();
        assert!(rows[0].is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn circle_footprint_imports() {
        let path = temp_workbook("circle");
        let headers = vec![
            "Building ID",
            "Name",
            "Category",
            "Model File Path",
            "Health",
            "Build Time",
            "Footprint Type",
            "Footprint Radius",
            "Enabled",
        ];
        write_dual_sheet_workbook(
            &path,
            &category_headers(),
            &[vec!["production", "Production", "", "Y"]],
            &headers,
            &[vec![
                "smelter",
                "Smelter",
                "production",
                "smelter.glb",
                "400",
                "90",
                "Circle",
                "2.5",
                "Y",
            ]],
        );
        let profiles = crate::world::InventoryProfileCatalog::default();
        let (categories, buildings, _) =
            import_building_catalog_from_excel(&path, &profiles).unwrap();
        assert_eq!(categories.len(), 1);
        let def = &buildings.definitions()[0];
        assert_eq!(def.footprint_type, FootprintType::Circle);
        let _ = std::fs::remove_file(path);
    }
}
