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
    if matches!(row.turn_left_duration_seconds, Some(value) if value <= 0.0) {
        return Err(crate::data_import::RowImportError {
            row_number: row.row_number,
            message: "Turn Left Duration must be positive when provided".to_string(),
        });
    }
    if matches!(row.turn_right_duration_seconds, Some(value) if value <= 0.0) {
        return Err(crate::data_import::RowImportError {
            row_number: row.row_number,
            message: "Turn Right Duration must be positive when provided".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_import::animation::schema::AnimationProfileImportRow;

    fn base_row() -> AnimationProfileImportRow {
        AnimationProfileImportRow {
            row_number: 2,
            profile_id: "humanoid".to_string(),
            idle_animation: "Idle".to_string(),
            walk_animation: String::new(),
            run_animation: String::new(),
            locomotion_reference_speed_mps: 4.0,
            enabled: true,
            enabled_was_blank: false,
            has_walk_column: false,
            has_run_column: false,
            has_reference_speed_column: false,
            death_animation: String::new(),
            hit_reaction_animation: String::new(),
            upper_body_split_bone: String::new(),
            turn_left_animation: String::new(),
            turn_right_animation: String::new(),
            turn_left_duration_seconds: None,
            turn_right_duration_seconds: None,
            has_death_column: false,
            has_hit_reaction_column: false,
            has_upper_body_split_bone_column: false,
            has_turn_left_column: false,
            has_turn_right_column: false,
            has_turn_left_duration_column: false,
            has_turn_right_duration_column: false,
        }
    }

    #[test]
    fn rejects_non_positive_turn_left_duration() {
        let mut row = base_row();
        row.turn_left_duration_seconds = Some(0.0);
        let err = validate_row(&row).unwrap_err();
        assert!(err.message.contains("Turn Left Duration"));
    }

    #[test]
    fn rejects_non_positive_turn_right_duration() {
        let mut row = base_row();
        row.turn_right_duration_seconds = Some(-1.0);
        let err = validate_row(&row).unwrap_err();
        assert!(err.message.contains("Turn Right Duration"));
    }
}
