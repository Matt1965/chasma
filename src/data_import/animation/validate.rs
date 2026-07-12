use super::schema::AnimationProfileImportRow;

pub fn validate_row(
    row: &AnimationProfileImportRow,
) -> Result<(), crate::data_import::RowImportError> {
    if row.profile_id.trim().is_empty() {
        return Err(crate::data_import::RowImportError {
            row_number: row.row_number,
            message: "Profile ID must be non-empty".to_string(),
        });
    }
    if row.idle_animation.trim().is_empty() {
        return Err(crate::data_import::RowImportError {
            row_number: row.row_number,
            message: "Idle Animation must be non-empty".to_string(),
        });
    }
    if row.locomotion_reference_speed_mps <= 0.0 {
        return Err(crate::data_import::RowImportError {
            row_number: row.row_number,
            message: "Locomotion Reference Speed must be positive".to_string(),
        });
    }
    Ok(())
}
