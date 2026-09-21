use super::schema::ArmorProfileImportRow;

pub fn validate_row(row: &ArmorProfileImportRow) -> Result<(), crate::data_import::RowImportError> {
    let fail = |message: String| crate::data_import::RowImportError {
        row_number: row.row_number,
        message,
    };

    if row.profile_id.trim().is_empty() {
        return Err(fail("Profile ID must be non-empty".to_string()));
    }
    if row.name.trim().is_empty() {
        return Err(fail("Name must be non-empty".to_string()));
    }

    Ok(())
}
