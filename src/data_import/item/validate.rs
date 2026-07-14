use super::schema::ItemImportRow;

pub fn validate_row(row: &ItemImportRow) -> Result<(), crate::data_import::RowImportError> {
    let fail = |message: String| crate::data_import::RowImportError {
        row_number: row.row_number,
        message,
    };

    if row.item_id.trim().is_empty() {
        return Err(fail("Item ID must be non-empty".to_string()));
    }
    if row.name.trim().is_empty() {
        return Err(fail("Name must be non-empty".to_string()));
    }
    if row.category.trim().is_empty() {
        return Err(fail("Category must be non-empty".to_string()));
    }
    if row.width == 0 || row.height == 0 {
        return Err(fail(format!(
            "Width and Height must be > 0 (got {}x{})",
            row.width, row.height
        )));
    }
    if row.mass_grams == 0 {
        return Err(fail(format!(
            "Mass Grams must be > 0 (got {})",
            row.mass_grams
        )));
    }
    if row.stackable && row.max_stack < 1 {
        return Err(fail(format!(
            "Max Stack must be >= 1 for stackable items (got {})",
            row.max_stack
        )));
    }
    if row.stackable && row.unique_instance_required {
        return Err(fail(
            "Stackable items cannot require unique instances".to_string(),
        ));
    }
    if row.unique_instance_required && row.max_stack != 1 {
        return Err(fail(format!(
            "Unique items must have Max Stack = 1 (got {})",
            row.max_stack
        )));
    }

    Ok(())
}
