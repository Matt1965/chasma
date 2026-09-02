//! Item definition Excel import (ADR-087 I1).

#[cfg(feature = "data-import")]
mod category_excel;
mod category_schema;
#[cfg(feature = "data-import")]
mod dev_load;
#[cfg(feature = "data-import")]
mod excel;
mod schema;
mod validate;

pub use category_schema::{
    ItemCategoryImportRow, REQUIRED_COLUMNS as ITEM_CATEGORY_REQUIRED_COLUMNS,
};
pub use schema::{
    ItemImportRow, OPTIONAL_COLUMNS as ITEM_OPTIONAL_COLUMNS,
    REQUIRED_COLUMNS as ITEM_REQUIRED_COLUMNS,
};

#[cfg(feature = "data-import")]
pub use category_excel::ITEM_CATEGORIES_SHEET_NAME;
#[cfg(feature = "data-import")]
pub use dev_load::{DEV_ITEM_CATALOG_RON_PATH, resolve_dev_item_catalog};
#[cfg(feature = "data-import")]
pub use excel::ITEMS_SHEET_NAME;

#[cfg(feature = "data-import")]
pub fn import_item_catalog_from_excel(
    path: &std::path::Path,
) -> Result<
    (
        crate::world::ItemCategoryCatalog,
        crate::world::ItemCatalog,
        crate::data_import::ImportSummary,
    ),
    crate::data_import::DataImportError,
> {
    let (categories, category_summary) = import_item_categories_from_excel(path)?;
    let (items, item_summary) = import_items_from_excel(path, &categories)?;
    let summary = crate::data_import::ImportSummary {
        rows_processed: category_summary.rows_processed + item_summary.rows_processed,
        rows_valid: category_summary.rows_valid + item_summary.rows_valid,
        rows_failed: category_summary.rows_failed + item_summary.rows_failed,
        warnings: category_summary
            .warnings
            .into_iter()
            .chain(item_summary.warnings)
            .collect(),
        sizing_reports: category_summary
            .sizing_reports
            .into_iter()
            .chain(item_summary.sizing_reports)
            .collect(),
    };
    let catalog =
        crate::world::ItemCatalog::from_definitions(items, &categories).map_err(|err| {
            crate::data_import::DataImportError::WorkbookOpen(format!(
                "item catalog build failed: {err}"
            ))
        })?;
    Ok((categories, catalog, summary))
}

#[cfg(feature = "data-import")]
pub fn import_item_categories_from_excel(
    path: &std::path::Path,
) -> Result<
    (
        crate::world::ItemCategoryCatalog,
        crate::data_import::ImportSummary,
    ),
    crate::data_import::DataImportError,
> {
    use std::collections::HashMap;

    use crate::world::ItemCategoryId;

    use category_excel::read_item_category_rows;

    let rows = read_item_category_rows(path)?;
    let mut summary = crate::data_import::ImportSummary {
        rows_processed: rows.len(),
        ..crate::data_import::ImportSummary::default()
    };
    let mut definitions = Vec::new();
    let mut seen_ids: HashMap<ItemCategoryId, usize> = HashMap::new();

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

        if row.category_id.trim().is_empty() {
            summary.rows_failed += 1;
            summary.warnings.push(format!(
                "row {}: Category ID must be non-empty",
                row.row_number
            ));
            continue;
        }
        if row.name.trim().is_empty() {
            summary.rows_failed += 1;
            summary
                .warnings
                .push(format!("row {}: Name must be non-empty", row.row_number));
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
                crate::data_import::DataImportError::DuplicateItemCategoryId {
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

    let catalog =
        crate::world::ItemCategoryCatalog::from_definitions(definitions).map_err(|err| {
            crate::data_import::DataImportError::WorkbookOpen(format!(
                "item category catalog build failed: {err}"
            ))
        })?;

    Ok((catalog, summary))
}

#[cfg(feature = "data-import")]
pub fn import_items_from_excel(
    path: &std::path::Path,
    categories: &crate::world::ItemCategoryCatalog,
) -> Result<
    (
        Vec<crate::world::ItemDefinition>,
        crate::data_import::ImportSummary,
    ),
    crate::data_import::DataImportError,
> {
    use std::collections::HashMap;

    use crate::world::ItemDefinitionId;
    use crate::world::validate_item_definition;

    use excel::read_item_rows;
    use validate::validate_row;

    let rows = read_item_rows(path)?;
    let mut summary = crate::data_import::ImportSummary {
        rows_processed: rows.len(),
        ..crate::data_import::ImportSummary::default()
    };
    let mut definitions = Vec::new();
    let mut seen_ids: HashMap<ItemDefinitionId, usize> = HashMap::new();

    for row_result in rows {
        let row = match row_result {
            Ok(row) => row,
            Err(row_err) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("row {}: {}", row_err.message, row_err.row_number));
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

        let definition = row.to_definition();
        if let Err(err) = validate_item_definition(&definition, categories, Some(row.row_number)) {
            summary.rows_failed += 1;
            summary
                .warnings
                .push(format!("row {}: {err}", row.row_number));
            continue;
        }

        if !definition.enabled {
            summary.warnings.push(format!(
                "row {}: Enabled=false — item excluded from catalog",
                row.row_number
            ));
            continue;
        }

        let id = definition.id.clone();
        if let Some(first_row) = seen_ids.insert(id.clone(), row.row_number) {
            return Err(crate::data_import::DataImportError::DuplicateItemId {
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

        if definition.category_id.as_str() == "food" && definition.nutrition == 0 {
            summary.warnings.push(format!(
                "row {}: food item `{}` has nutrition 0 — provides no usable food value",
                row.row_number,
                definition.id.as_str()
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

#[cfg(test)]
mod tests {
    use super::schema::ItemImportRow;
    use super::validate;
    use crate::world::{
        ItemCategoryCatalog, ItemCategoryDefinition, ItemCategoryId, ItemDefinitionId,
        validate_item_definition,
    };

    fn currency_categories() -> ItemCategoryCatalog {
        ItemCategoryCatalog::from_definitions(vec![ItemCategoryDefinition::new(
            ItemCategoryId::new("currency"),
            "Currency",
            "",
            true,
        )])
        .expect("currency category")
    }

    #[test]
    fn physical_gold_import_row_validates() {
        let row = ItemImportRow {
            row_number: 2,
            item_id: "gold".to_string(),
            name: "Gold".to_string(),
            description: String::new(),
            category: "currency".to_string(),
            width: 1,
            height: 1,
            stackable: true,
            max_stack: 999,
            mass_grams: 1,
            base_value: 1,
            render_key: None,
            icon_key: Some("gold".to_string()),
            tags: vec![],
            unique_instance_required: false,
            nutrition: 0,
            enabled: true,
            enabled_was_blank: false,
        };
        validate::validate_row(&row).expect("row valid");
        let definition = row.to_definition();
        validate_item_definition(&definition, &currency_categories(), Some(2))
            .expect("gold validates");
        assert_eq!(definition.id, ItemDefinitionId::new("gold"));
    }
}

#[cfg(all(feature = "data-import", test))]
mod integration_tests {
    use std::path::{Path, PathBuf};

    use rust_xlsxwriter::Workbook;

    use super::{
        DEV_ITEM_CATALOG_RON_PATH, import_item_catalog_from_excel,
        import_item_categories_from_excel, import_items_from_excel,
    };
    use crate::data_import::DataImportError;
    use crate::data_import::item::category_excel::ITEM_CATEGORIES_SHEET_NAME;
    use crate::data_import::item::excel::ITEMS_SHEET_NAME;
    use crate::data_import::paths::dev_design_workbook_path;
    use crate::world::{ItemCategoryCatalog, ItemDefinitionId};

    fn assert_footprint(items: &crate::world::ItemCatalog, id: &str, width: u8, height: u8) {
        let item = items
            .get(&ItemDefinitionId::new(id))
            .unwrap_or_else(|| panic!("expected workbook item `{id}`"));
        assert_eq!(
            (item.grid_width, item.grid_height),
            (width, height),
            "footprint for `{id}`"
        );
    }

    fn assert_no_legacy_sequence_ids(items: &crate::world::ItemCatalog) {
        for definition in items.definitions() {
            let id = definition.id.as_str();
            assert!(
                !id.starts_with("I-00"),
                "legacy sequence item id `{id}` must not appear in imported catalog"
            );
        }
    }

    fn temp_workbook(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "chasma_item_import_{}_{}.xlsx",
            std::process::id(),
            name
        ))
    }

    fn write_dual_sheet_workbook(
        path: &Path,
        category_headers: &[&str],
        category_rows: &[Vec<&str>],
        item_headers: &[&str],
        item_rows: &[Vec<&str>],
    ) {
        let mut workbook = Workbook::new();
        let categories = workbook.add_worksheet();
        categories.set_name(ITEM_CATEGORIES_SHEET_NAME).unwrap();
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
        let items = workbook.add_worksheet();
        items.set_name(ITEMS_SHEET_NAME).unwrap();
        for (col, header) in item_headers.iter().enumerate() {
            items.write_string(0, col as u16, *header).unwrap();
        }
        for (row_idx, row) in item_rows.iter().enumerate() {
            for (col, value) in row.iter().enumerate() {
                items
                    .write_string((row_idx + 1) as u32, col as u16, *value)
                    .unwrap();
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        workbook.save(path).unwrap();
    }

    fn item_headers() -> Vec<&'static str> {
        vec![
            "Item ID",
            "Name",
            "Category",
            "Width",
            "Height",
            "Stackable",
            "Max Stack",
            "Mass Grams",
            "Enabled",
        ]
    }

    fn category_headers() -> Vec<&'static str> {
        vec!["Category ID", "Name", "Enabled"]
    }

    #[test]
    fn import_from_design_workbook() {
        let path = dev_design_workbook_path();
        assert!(
            path.exists(),
            "expected authoritative workbook at {}",
            path.display()
        );

        let (categories, items, summary) = import_item_catalog_from_excel(&path).unwrap();
        assert_eq!(
            summary.rows_failed, 0,
            "item import failures: {:?}",
            summary.warnings
        );
        assert!(
            summary.rows_valid >= 35,
            "expected 9 categories + 27 items; valid={} processed={}",
            summary.rows_valid,
            summary.rows_processed
        );
        assert_eq!(items.len(), 27, "expected 27 authored items");
        assert_eq!(categories.len(), 9, "expected 9 item categories");

        let gold = items
            .get(&ItemDefinitionId::new("gold"))
            .expect("gold from workbook");
        assert!(gold.stackable);
        assert_eq!(gold.max_stack, 999);
        assert_eq!(gold.mass_grams_per_unit, 1);
        assert!(categories.get(&gold.category_id).is_some());
        assert_footprint(&items, "gold", 1, 1);

        let iron_ore = items
            .get(&ItemDefinitionId::new("iron_ore"))
            .expect("iron_ore from workbook");
        assert!(iron_ore.stackable);
        assert_eq!(iron_ore.max_stack, 50);
        assert_eq!(iron_ore.mass_grams_per_unit, 2000);
        assert!(categories.get(&iron_ore.category_id).is_some());
        assert_footprint(&items, "iron_ore", 2, 2);

        // Pipeline preserves non-square authored footprints from the workbook.
        assert_footprint(&items, "plant_fiber", 1, 3);
        assert_footprint(&items, "iron_bar", 1, 3);
        assert_footprint(&items, "copper_bar", 3, 1);
        assert_footprint(&items, "ancient_parts", 3, 1);
        assert_footprint(&items, "iron_sword", 1, 3);
        assert_footprint(&items, "sledgehammer", 2, 4);
        assert_footprint(&items, "iron_plate_armor", 3, 3);
        assert_footprint(&items, "crossbow", 2, 3);

        assert_no_legacy_sequence_ids(&items);
    }

    #[test]
    fn design_workbook_categories_import() {
        let path = dev_design_workbook_path();
        assert!(path.exists());
        let (categories, summary) = import_item_categories_from_excel(&path).unwrap();
        assert!(summary.rows_failed == 0 && summary.rows_valid >= 1);
        assert!(
            categories
                .get(&crate::world::ItemCategoryId::new("currency"))
                .is_some()
        );
        assert!(
            categories
                .get(&crate::world::ItemCategoryId::new("raw_material"))
                .is_some()
        );
        assert!(
            categories
                .get(&crate::world::ItemCategoryId::new("construction_material"))
                .is_some()
        );
    }

    #[test]
    fn design_workbook_items_import() {
        let path = dev_design_workbook_path();
        assert!(path.exists());
        let (categories, _) = import_item_categories_from_excel(&path).unwrap();
        let (definitions, summary) = import_items_from_excel(&path, &categories).unwrap();
        assert_eq!(summary.rows_failed, 0);
        assert_eq!(definitions.len(), 26);
        assert!(definitions.iter().any(|def| def.id.as_str() == "gold"));
        assert!(definitions.iter().any(|def| def.id.as_str() == "iron_ore"));
        assert!(
            definitions
                .iter()
                .any(|def| def.id.as_str() == "sledgehammer")
        );
        assert!(
            !definitions
                .iter()
                .any(|def| def.id.as_str().starts_with("I-00")),
            "legacy I-00xx ids must not remain in item definitions"
        );
    }

    fn write_items_only_workbook(path: &Path, item_headers: &[&str], item_rows: &[Vec<&str>]) {
        let mut workbook = Workbook::new();
        let items = workbook.add_worksheet();
        items.set_name(ITEMS_SHEET_NAME).unwrap();
        for (col, header) in item_headers.iter().enumerate() {
            items.write_string(0, col as u16, *header).unwrap();
        }
        for (row_idx, row) in item_rows.iter().enumerate() {
            for (col, value) in row.iter().enumerate() {
                items
                    .write_string((row_idx + 1) as u32, col as u16, *value)
                    .unwrap();
            }
        }
        workbook.save(path).unwrap();
    }

    #[test]
    fn missing_item_categories_sheet_fails_clearly() {
        let path = temp_workbook("no_categories");
        write_items_only_workbook(
            &path,
            &item_headers(),
            &[vec![
                "gold", "Gold", "currency", "1", "1", "Y", "999", "1", "Y",
            ]],
        );
        let err = import_item_catalog_from_excel(&path).unwrap_err();
        assert!(matches!(
            err,
            DataImportError::SheetNotFound { .. } | DataImportError::MissingRequiredColumn { .. }
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_required_item_columns_fails_clearly() {
        let path = temp_workbook("bad_items");
        write_dual_sheet_workbook(
            &path,
            &category_headers(),
            &[vec!["currency", "Currency", "Y"]],
            &["Item ID", "Name", "Category"],
            &[vec!["gold", "Gold", "currency"]],
        );
        let err = import_item_catalog_from_excel(&path).unwrap_err();
        assert!(matches!(err, DataImportError::MissingRequiredColumn { .. }));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn design_workbook_exports_generated_ron() {
        let path = dev_design_workbook_path();
        assert!(path.exists());
        let (categories, items, _) = import_item_catalog_from_excel(&path).unwrap();
        let export_path = std::env::temp_dir().join(format!(
            "chasma_item_catalog_export_{}.ron",
            std::process::id()
        ));
        crate::data_import::ron::export_items_to_ron(
            &export_path,
            categories.definitions(),
            items.definitions(),
        )
        .unwrap();
        let text = std::fs::read_to_string(&export_path).unwrap();
        assert!(text.contains("gold"));
        assert!(text.contains("iron_ore"));
        assert!(text.contains("sledgehammer"));
        assert!(text.contains("iron_plate_armor"));
        assert!(text.contains("currency"));
        assert!(!text.contains("I-0014"));
        assert!(!text.contains("I-00"));
        let _ = std::fs::remove_file(export_path);
    }

    #[test]
    fn dev_ron_export_path_is_derivative_not_authority() {
        assert_eq!(
            DEV_ITEM_CATALOG_RON_PATH, "assets/items/catalog.ron",
            "generated RON path must remain derivative export target"
        );
    }
}
