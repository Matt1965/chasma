use super::error::RowImportError;
use super::schema::DoodadImportRow;

#[cfg_attr(not(feature = "data-import"), allow(dead_code))]
pub fn validate_row(row: &DoodadImportRow) -> Result<(), RowImportError> {
    let fail = |message: String| RowImportError {
        row_number: row.row_number,
        message,
    };

    if row.name.trim().is_empty() {
        return Err(fail("Name must be non-empty".to_string()));
    }
    if row.file_path.trim().is_empty() {
        return Err(fail("File Path must be non-empty".to_string()));
    }
    if row.min_size > row.max_size {
        return Err(fail(format!(
            "Min Size ({}) must be <= Max Size ({})",
            row.min_size, row.max_size
        )));
    }
    if row.min_size < 0.0 || row.max_size < 0.0 {
        return Err(fail("Min Size and Max Size must be >= 0".to_string()));
    }
    if row.spawn_weight < 0.0 {
        return Err(fail(format!(
            "Spawn Weight must be >= 0 (got {})",
            row.spawn_weight
        )));
    }
    if !row.spawn_weight.is_finite() || !row.min_size.is_finite() || !row.max_size.is_finite() {
        return Err(fail("numeric fields must be finite".to_string()));
    }

    // Validate enum conversions early for clearer row errors.
    if super::schema::parse_category(&row.category).is_err() {
        return Err(fail(format!("invalid Category `{}`", row.category.trim())));
    }
    if !row.biome.trim().is_empty() && super::schema::parse_biome(&row.biome).is_err() {
        return Err(fail(format!("invalid Biome `{}`", row.biome.trim())));
    }
    if super::schema::normalize_file_path_to_render_key(&row.file_path).is_err() {
        return Err(fail("invalid File Path".to_string()));
    }
    if let Some(radius) = row.block_radius_meters {
        if radius < 0.0 || !radius.is_finite() {
            return Err(fail(format!(
                "Block Radius must be >= 0 (got {radius})"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_import::schema::DoodadImportRow;

    fn row_with(min: f32, max: f32, weight: f32) -> DoodadImportRow {
        DoodadImportRow {
            row_number: 3,
            name: "tree_oak".to_string(),
            definition_id: "tree_oak".to_string(),
            description: "Oak".to_string(),
            category: "Tree".to_string(),
            biome: "Forest".to_string(),
            file_path: "tree/oak.glb".to_string(),
            min_size: min,
            max_size: max,
            spawn_weight: weight,
            random_rotation: true,
            enabled: true,
            enabled_was_blank: false,
            blocks_movement: None,
            block_radius_meters: None,
        }
    }

    #[test]
    fn rejects_min_greater_than_max() {
        assert!(validate_row(&row_with(2.0, 1.0, 1.0)).is_err());
    }

    #[test]
    fn rejects_negative_spawn_weight() {
        assert!(validate_row(&row_with(1.0, 2.0, -1.0)).is_err());
    }

    #[test]
    fn accepts_valid_row() {
        assert!(validate_row(&row_with(0.8, 1.2, 5.0)).is_ok());
    }
}
