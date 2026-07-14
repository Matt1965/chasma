//! Excel column schema and conversion into building catalog types (B1).

use crate::world::{
    AnimationProfileId, BuildingCategoryDefinition, BuildingCategoryId, BuildingDefinition,
    BuildingDefinitionId, BuildingRenderKey, FootprintSpec, FootprintType, InventoryProfileId,
};

/// Required worksheet column headers from the workbook `Buildings` sheet.
pub const REQUIRED_COLUMNS: &[&str] = &[
    "Building ID",
    "Name",
    "Category",
    "Model File Path",
    "Health",
    "Build Time",
    "Footprint Type",
    "Enabled",
];

/// Optional worksheet columns.
pub const OPTIONAL_COLUMNS: &[&str] = &[
    "Collision File Path",
    "Preview File Path",
    "Footprint Width",
    "Footprint Depth",
    "Footprint Radius",
    "Max Slope",
    "Construction Stages",
    "Task Provider",
    "Animation Profile",
    "Interaction Profile",
    "Default Space",
    "Inventory Profile ID",
];

pub const DEFAULT_MAX_SLOPE_DEGREES: f32 = 40.0;

/// Required worksheet column headers from the workbook `Building Categories` sheet.
pub const CATEGORY_REQUIRED_COLUMNS: &[&str] = &["Category ID", "Display Name", "Enabled"];

/// Raw row parsed from the `Building Categories` sheet before validation.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingCategoryImportRow {
    pub row_number: usize,
    pub category_id: String,
    pub display_name: String,
    pub description: String,
    pub enabled: bool,
    pub enabled_was_blank: bool,
}

impl BuildingCategoryImportRow {
    pub fn to_definition(&self) -> BuildingCategoryDefinition {
        BuildingCategoryDefinition::new(
            BuildingCategoryId::new(self.category_id.trim()),
            self.display_name.trim(),
            self.description.trim(),
            self.enabled,
        )
    }
}

/// Raw row parsed from the `Buildings` sheet before validation.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingImportRow {
    pub row_number: usize,
    pub building_id: String,
    pub name: String,
    pub category: String,
    pub model_file_path: String,
    pub collision_file_path: String,
    pub preview_file_path: String,
    pub health: u32,
    pub build_time_seconds: f32,
    pub footprint_type: FootprintType,
    pub footprint_width_meters: Option<f32>,
    pub footprint_depth_meters: Option<f32>,
    pub footprint_radius_meters: Option<f32>,
    pub max_slope_degrees: f32,
    pub construction_stages: String,
    pub task_provider: String,
    pub animation_profile: String,
    pub interaction_profile: String,
    pub default_space: String,
    pub inventory_profile_id: String,
    pub has_inventory_profile_column: bool,
    pub enabled: bool,
    pub enabled_was_blank: bool,
    pub has_collision_file_path_column: bool,
    pub has_preview_file_path_column: bool,
    pub has_footprint_width_column: bool,
    pub has_footprint_depth_column: bool,
    pub has_footprint_radius_column: bool,
}

impl BuildingImportRow {
    pub fn to_definition(&self) -> Result<BuildingDefinition, String> {
        let footprint = match self.footprint_type {
            FootprintType::Rectangle => {
                let width = self.footprint_width_meters.ok_or_else(|| {
                    "Footprint Width required for Rectangle footprint".to_string()
                })?;
                let depth = self.footprint_depth_meters.ok_or_else(|| {
                    "Footprint Depth required for Rectangle footprint".to_string()
                })?;
                FootprintSpec::Rectangle {
                    width_meters: width,
                    depth_meters: depth,
                }
            }
            FootprintType::Circle => {
                let radius = self
                    .footprint_radius_meters
                    .ok_or_else(|| "Footprint Radius required for Circle footprint".to_string())?;
                FootprintSpec::Circle {
                    radius_meters: radius,
                }
            }
            FootprintType::MeshDerived => FootprintSpec::MeshDerived,
        };

        let render_key = BuildingRenderKey::reserved(normalize_building_file_path_to_render_key(
            &self.model_file_path,
        )?);
        let collision_render_key = if self.collision_file_path.trim().is_empty() {
            if self.footprint_type == FootprintType::MeshDerived {
                return Err("Collision File Path required for MeshDerived footprint".to_string());
            }
            render_key.clone()
        } else {
            BuildingRenderKey::reserved(normalize_building_file_path_to_render_key(
                &self.collision_file_path,
            )?)
        };

        let mut definition = BuildingDefinition::new(
            BuildingDefinitionId::new(self.building_id.trim()),
            self.name.trim(),
            BuildingCategoryId::new(self.category.trim()),
            render_key,
            collision_render_key,
            self.health,
            self.build_time_seconds,
            footprint,
            self.max_slope_degrees,
            self.enabled,
        );

        if !self.preview_file_path.trim().is_empty() {
            definition = definition.with_preview_render_key(BuildingRenderKey::reserved(
                normalize_building_file_path_to_render_key(&self.preview_file_path)?,
            ));
        }
        if !self.construction_stages.trim().is_empty() {
            definition = definition.with_construction_stages_ref(self.construction_stages.trim());
        }
        if !self.task_provider.trim().is_empty() {
            definition = definition.with_task_provider_id(self.task_provider.trim());
        }
        if !self.animation_profile.trim().is_empty() {
            definition = definition
                .with_animation_profile_id(AnimationProfileId::new(self.animation_profile.trim()));
        }
        if !self.interaction_profile.trim().is_empty() {
            definition = definition.with_interaction_profile_id(self.interaction_profile.trim());
        }
        if !self.default_space.trim().is_empty() {
            definition = definition.with_default_space_id(self.default_space.trim());
        }
        if self.has_inventory_profile_column && !self.inventory_profile_id.trim().is_empty() {
            definition = definition.with_inventory_profile_id(InventoryProfileId::new(
                self.inventory_profile_id.trim(),
            ));
        }

        Ok(definition)
    }
}

/// Normalize a workbook asset path cell into a building render key (`hut`, `fort/wall`).
pub fn normalize_building_file_path_to_render_key(path: &str) -> Result<String, String> {
    let mut key = super::super::schema::normalize_file_path(path);
    if key.is_empty() {
        return Err("asset path must be non-empty".to_string());
    }
    for prefix in ["assets/buildings/", "buildings/"] {
        if let Some(rest) = key.strip_prefix(prefix) {
            key = rest.to_string();
        }
    }
    if let Some(stripped) = key.strip_suffix(".glb") {
        key = stripped.to_string();
    }
    if key.is_empty() {
        return Err("asset path must resolve to a render key".to_string());
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_path_normalization() {
        assert_eq!(
            normalize_building_file_path_to_render_key(r"\buildings\hut.glb").unwrap(),
            "hut"
        );
        assert_eq!(
            normalize_building_file_path_to_render_key("assets/buildings/fort/wall.glb").unwrap(),
            "fort/wall"
        );
    }

    #[test]
    fn rectangle_row_to_definition() {
        let row = BuildingImportRow {
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
        };
        let def = row.to_definition().unwrap();
        assert_eq!(def.id.as_str(), "hut");
        assert_eq!(def.render_key.0.as_deref(), Some("hut"));
    }
}
