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
    if row.render_scale <= 0.0 {
        return Err(fail(format!(
            "Render Scale must be > 0 (got {})",
            row.render_scale
        )));
    }
    if row.has_default_weapon_column && row.default_weapon_id.trim().is_empty() {
        return Err(fail("Default Weapon ID must be non-empty".to_string()));
    }
    if row.max_hp == 0 {
        return Err(fail(format!("Max HP must be > 0 (got {})", row.max_hp)));
    }
    if !row.power_rating.is_finite()
        || !row.move_speed_mps.is_finite()
        || !row.collision_radius_meters.is_finite()
        || !row.max_slope_degrees.is_finite()
        || !row.render_scale.is_finite()
        || !row.turn_speed_degrees_per_second.is_finite()
        || !row.sight_range_meters.is_finite()
    {
        return Err(fail("numeric fields must be finite".to_string()));
    }
    if row.has_turn_speed_column && row.turn_speed_degrees_per_second <= 0.0 {
        return Err(fail(format!(
            "Turn Speed Deg/s must be > 0 (got {})",
            row.turn_speed_degrees_per_second
        )));
    }
    if row.has_sight_range_column && row.sight_range_meters <= 0.0 {
        return Err(fail(format!(
            "Sight Range must be > 0 (got {})",
            row.sight_range_meters
        )));
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
    use crate::world::DEFAULT_TURN_SPEED_DEGREES_PER_SECOND;

    fn row_with(move_speed: f32, collision: f32) -> UnitImportRow {
        UnitImportRow {
            row_number: 3,
            unit_id: "U-0001".to_string(),
            name: "Wolf".to_string(),
            faction_key: "wild".to_string(),
            species_key: "wolf".to_string(),
            level: 2,
            base_hp: 5,
            max_hp: 5,
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
            render_scale: 1.0,
            default_weapon_id: "weapon_fists".to_string(),
            enabled: true,
            enabled_was_blank: false,
            has_file_path_column: true,
            has_default_weapon_column: true,
            has_render_scale_column: false,
            animation_profile: String::new(),
            has_animation_profile_column: false,
            inventory_profile_id: String::new(),
            has_inventory_profile_column: false,
            turn_speed_degrees_per_second: DEFAULT_TURN_SPEED_DEGREES_PER_SECOND,
            has_turn_speed_column: false,
            sight_range_meters: crate::data_import::unit::schema::DEFAULT_SIGHT_RANGE_METERS,
            has_sight_range_column: false,
            asset_sizing: Default::default(),
        }
    }

    #[test]
    fn rejects_non_positive_move_speed() {
        assert!(validate_row(&row_with(0.0, 0.5)).is_err());
    }

    #[test]
    fn rejects_non_positive_max_hp() {
        let mut row = row_with(4.5, 0.6);
        row.max_hp = 0;
        assert!(validate_row(&row).is_err());
    }

    #[test]
    fn rejects_empty_default_weapon_id() {
        let mut row = row_with(4.5, 0.6);
        row.default_weapon_id.clear();
        row.has_default_weapon_column = true;
        assert!(validate_row(&row).is_err());
    }

    #[test]
    fn accepts_valid_row() {
        assert!(validate_row(&row_with(4.5, 0.6)).is_ok());
    }

    #[test]
    fn rejects_non_positive_turn_speed_when_column_present() {
        let mut row = row_with(4.5, 0.6);
        row.has_turn_speed_column = true;
        row.turn_speed_degrees_per_second = 0.0;
        assert!(validate_row(&row).is_err());
    }

    #[test]
    fn rejects_non_positive_sight_range_when_column_present() {
        let mut row = row_with(4.5, 0.6);
        row.has_sight_range_column = true;
        row.sight_range_meters = 0.0;
        assert!(validate_row(&row).is_err());
    }

    #[test]
    fn rejects_negative_turn_speed_when_column_present() {
        let mut row = row_with(4.5, 0.6);
        row.has_turn_speed_column = true;
        row.turn_speed_degrees_per_second = -90.0;
        assert!(validate_row(&row).is_err());
    }
}
