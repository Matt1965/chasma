use super::schema::InventoryProfileImportRow;

pub fn validate_row(
    row: &InventoryProfileImportRow,
) -> Result<(), crate::data_import::RowImportError> {
    let fail = |message: String| crate::data_import::RowImportError {
        row_number: row.row_number,
        message,
    };

    if row.profile_id.trim().is_empty() {
        return Err(fail("Inventory Profile ID must be non-empty".to_string()));
    }
    if row.name.trim().is_empty() {
        return Err(fail("Name must be non-empty".to_string()));
    }
    if row.grid_width == 0 || row.grid_height == 0 {
        return Err(fail(format!(
            "Grid Width and Grid Height must be > 0 (got {}x{})",
            row.grid_width, row.grid_height
        )));
    }
    if let Some(cap) = row.global_stack_cap {
        if cap < 1 {
            return Err(fail(format!("Global Stack Cap must be >= 1 (got {cap})")));
        }
    }

    Ok(())
}
