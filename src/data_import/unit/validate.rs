use super::schema::UnitImportRow;

#[cfg_attr(not(feature = "data-import"), allow(dead_code))]
pub fn validate_row(row: &UnitImportRow) -> Result<(), crate::data_import::RowImportError> {
    let fail = |message: String| crate::data_import::RowImportError {
        row_number: row.row_number,
        message,
    };

    if row.unit_id.trim().is_empty() {
        return Err(fail("Unit ID must be non-empty".to_string()));
    }
    if row.name.trim().is_empty() {
        return Err(fail("Name must be non-empty".to_string()));
    }
    if row.tier.trim().is_empty() {
        return Err(fail("Tier must be non-empty".to_string()));
    }
    if row.move_speed_mps <= 0.0 {
        return Err(fail(format!(
            "Move Speed must be > 0 (got {})",
            row.move_speed_mps
        )));
    }
    if row.collision_radius_meters < 0.0 {
        return Err(fail(format!(
            "Collision Radius must be >= 0 (got {})",
            row.collision_radius_meters
        )));
    }
    if row.max_slope_degrees < 0.0 {
        return Err(fail(format!(
            "Max Slope must be >= 0 (got {})",
            row.max_slope_degrees
        )));
    }
    if !row.power_rating.is_finite()
        || !row.move_speed_mps.is_finite()
        || !row.collision_radius_meters.is_finite()
        || !row.max_slope_degrees.is_finite()
    {
        return Err(fail("numeric fields must be finite".to_string()));
    }
    if row.has_file_path_column && !row.file_path.trim().is_empty() {
        if super::schema::normalize_file_path_to_render_key(&row.file_path).is_err() {
            return Err(fail("invalid File Path".to_string()));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_import::unit::schema::UnitImportRow;

    fn row_with(move_speed: f32, collision: f32) -> UnitImportRow {
        UnitImportRow {
            row_number: 3,
            unit_id: "U-0001".to_string(),
            name: "Wolf".to_string(),
            faction: "Wild".to_string(),
            level: 2,
            base_hp: 5,
            strength: 4,
            dexterity: 6,
            constitution: 3,
            agility: 7,
            charisma: 2,
            intelligence: 3,
            power_rating: 26.5,
            tier: "Elite".to_string(),
            file_path: "units/wolf.glb".to_string(),
            move_speed_mps: move_speed,
            collision_radius_meters: collision,
            max_slope_degrees: 40.0,
            enabled: true,
            enabled_was_blank: false,
            has_file_path_column: true,
        }
    }

    #[test]
    fn rejects_non_positive_move_speed() {
        assert!(validate_row(&row_with(0.0, 0.5)).is_err());
    }

    #[test]
    fn accepts_valid_row() {
        assert!(validate_row(&row_with(4.5, 0.6)).is_ok());
    }
}
