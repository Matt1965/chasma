use super::schema::{BuildingCategoryImportRow, BuildingImportRow};
use crate::world::FootprintType;

pub fn validate_category_row(
    row: &BuildingCategoryImportRow,
) -> Result<(), crate::data_import::RowImportError> {
    let fail = |message: String| crate::data_import::RowImportError {
        row_number: row.row_number,
        message,
    };

    if row.category_id.trim().is_empty() {
        return Err(fail("Category ID must be non-empty".to_string()));
    }
    if row.display_name.trim().is_empty() {
        return Err(fail("Display Name must be non-empty".to_string()));
    }

    Ok(())
}

pub fn validate_row(row: &BuildingImportRow) -> Result<(), crate::data_import::RowImportError> {
    let fail = |message: String| crate::data_import::RowImportError {
        row_number: row.row_number,
        message,
    };

    if row.building_id.trim().is_empty() {
        return Err(fail("Building ID must be non-empty".to_string()));
    }
    if row.name.trim().is_empty() {
        return Err(fail("Name must be non-empty".to_string()));
    }
    if row.category.trim().is_empty() {
        return Err(fail("Category must be non-empty".to_string()));
    }
    if row.model_file_path.trim().is_empty() {
        return Err(fail("Model File Path must be non-empty".to_string()));
    }
    if row.health == 0 {
        return Err(fail("Health must be > 0".to_string()));
    }
    if row.build_time_seconds < 0.0 || !row.build_time_seconds.is_finite() {
        return Err(fail(format!(
            "Build Time must be >= 0 (got {})",
            row.build_time_seconds
        )));
    }
    if row.max_slope_degrees < 0.0 || !row.max_slope_degrees.is_finite() {
        return Err(fail("Max Slope must be a finite number >= 0".to_string()));
    }

    if super::schema::normalize_building_file_path_to_render_key(&row.model_file_path).is_err() {
        return Err(fail("invalid Model File Path".to_string()));
    }
    if !row.collision_file_path.trim().is_empty()
        && super::schema::normalize_building_file_path_to_render_key(&row.collision_file_path)
            .is_err()
    {
        return Err(fail("invalid Collision File Path".to_string()));
    }
    if !row.preview_file_path.trim().is_empty()
        && super::schema::normalize_building_file_path_to_render_key(&row.preview_file_path)
            .is_err()
    {
        return Err(fail("invalid Preview File Path".to_string()));
    }

    match row.footprint_type {
        FootprintType::Rectangle => {
            let width = row.footprint_width_meters.ok_or_else(|| {
                fail("Footprint Width required for Rectangle footprint".to_string())
            })?;
            let depth = row.footprint_depth_meters.ok_or_else(|| {
                fail("Footprint Depth required for Rectangle footprint".to_string())
            })?;
            if width <= 0.0 || depth <= 0.0 || !width.is_finite() || !depth.is_finite() {
                return Err(fail(
                    "Footprint Width and Footprint Depth must be > 0".to_string(),
                ));
            }
        }
        FootprintType::Circle => {
            let radius = row.footprint_radius_meters.ok_or_else(|| {
                fail("Footprint Radius required for Circle footprint".to_string())
            })?;
            if radius <= 0.0 || !radius.is_finite() {
                return Err(fail("Footprint Radius must be > 0".to_string()));
            }
        }
        FootprintType::MeshDerived => {
            if row.collision_file_path.trim().is_empty() {
                return Err(fail(
                    "Collision File Path required for MeshDerived footprint".to_string(),
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_import::building::schema::BuildingImportRow;
    use crate::world::FootprintType;

    fn sample_row() -> BuildingImportRow {
        BuildingImportRow {
            row_number: 2,
            building_id: "hut".to_string(),
            name: "Hut".to_string(),
            category: "residential".to_string(),
            model_file_path: "hut.glb".to_string(),
            collision_file_path: String::new(),
            preview_file_path: String::new(),
            health: 100,
            build_time_seconds: 30.0,
            footprint_type: FootprintType::Rectangle,
            footprint_width_meters: Some(4.0),
            footprint_depth_meters: Some(4.0),
            footprint_radius_meters: None,
            max_slope_degrees: 35.0,
            construction_stages: String::new(),
            task_provider: String::new(),
            animation_profile: String::new(),
            interaction_profile: String::new(),
            default_space: String::new(),
            inventory_profile_id: String::new(),
            has_inventory_profile_column: false,
            enabled: true,
            enabled_was_blank: false,
            has_collision_file_path_column: false,
            has_preview_file_path_column: false,
            has_footprint_width_column: true,
            has_footprint_depth_column: true,
            has_footprint_radius_column: false,
            asset_sizing: Default::default(),
            transform_safety_class: crate::world::BuildingTransformSafetyClass::Navigable,
            allow_instance_scale: false,
            min_uniform_instance_scale: None,
            max_uniform_instance_scale: None,
        }
    }

    #[test]
    fn valid_rectangle_row_passes() {
        assert!(validate_row(&sample_row()).is_ok());
    }

    #[test]
    fn mesh_derived_requires_collision_path() {
        let mut row = sample_row();
        row.footprint_type = FootprintType::MeshDerived;
        row.footprint_width_meters = None;
        row.footprint_depth_meters = None;
        let err = validate_row(&row).unwrap_err();
        assert!(err.message.contains("Collision File Path"));
    }

    #[test]
    fn circle_requires_radius() {
        let mut row = sample_row();
        row.footprint_type = FootprintType::Circle;
        row.footprint_radius_meters = None;
        let err = validate_row(&row).unwrap_err();
        assert!(err.message.contains("Footprint Radius"));
    }
}
