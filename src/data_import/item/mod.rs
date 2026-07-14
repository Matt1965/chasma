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
    use crate::world::{ItemCategoryCatalog, ItemDefinitionId, validate_item_definition};

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
            enabled: true,
            enabled_was_blank: false,
        };
        validate::validate_row(&row).expect("row valid");
        let definition = row.to_definition();
        validate_item_definition(&definition, &ItemCategoryCatalog::default(), Some(2))
            .expect("gold validates");
        assert_eq!(definition.id, ItemDefinitionId::new("gold"));
    }
}
