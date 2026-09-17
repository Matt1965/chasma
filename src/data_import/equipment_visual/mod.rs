//! Equipment visual variant Excel import (Slice 7.1).

#[cfg(feature = "data-import")]
mod dev_load;
#[cfg(feature = "data-import")]
mod excel;
mod schema;
mod validate;

pub use schema::{
    EquipmentVisualImportRow, OPTIONAL_COLUMNS as EQUIPMENT_VISUAL_OPTIONAL_COLUMNS,
    REQUIRED_COLUMNS as EQUIPMENT_VISUAL_REQUIRED_COLUMNS,
};

#[cfg(feature = "data-import")]
pub use dev_load::resolve_dev_equipment_visual_catalog;
#[cfg(feature = "data-import")]
pub use excel::EQUIPMENT_VISUALS_SHEET_NAME;

#[cfg(test)]
mod fit_tests;
#[cfg(test)]
mod tests;

#[cfg(feature = "data-import")]
pub fn import_equipment_visuals_from_excel(
    path: &std::path::Path,
    items: &crate::world::ItemCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
) -> Result<
    (
        crate::world::EquipmentVisualCatalog,
        crate::data_import::ImportSummary,
    ),
    crate::data_import::DataImportError,
> {
    use excel::read_equipment_visual_rows;

    let rows = read_equipment_visual_rows(path)?;
    let mut summary = crate::data_import::ImportSummary {
        rows_processed: rows.len(),
        ..crate::data_import::ImportSummary::default()
    };
    let mut mappings = Vec::new();
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
        if items
            .get(&crate::world::ItemDefinitionId::new(row.item_id.trim()))
            .is_none()
        {
            summary.rows_failed += 1;
            summary.warnings.push(format!(
                "row {}: Item ID `{}` not found in Items sheet",
                row.row_number,
                row.item_id.trim()
            ));
            continue;
        }
        let mapping = match row.to_mapping() {
            Ok(mapping) => mapping,
            Err(message) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("row {}: {message}", row.row_number));
                continue;
            }
        };
        if let Err(message) =
            validate::validate_equipment_visual_mapping(&mapping, appearance_profiles, row.row_number)
        {
            summary.rows_failed += 1;
            summary
                .warnings
                .push(format!("row {}: {message}", row.row_number));
            continue;
        }
        mappings.push(mapping);
        summary.rows_valid += 1;
    }
    let catalog = crate::world::EquipmentVisualCatalog::from_mappings(mappings).map_err(|err| {
        crate::data_import::DataImportError::WorkbookOpen(format!(
            "equipment visual catalog build failed: {err}"
        ))
    })?;
    Ok((catalog, summary))
}
