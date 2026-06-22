//! Excel column schema and conversion into [`UnitDefinition`].

use crate::world::{
    UnitDefinition, UnitDefinitionId, UnitRenderKey,
};

use super::super::schema::normalize_file_path;

/// Required worksheet column headers from the workbook `Units` sheet (order irrelevant).
pub const REQUIRED_COLUMNS: &[&str] = &[
    "Unit ID",
    "Name",
    "Faction",
    "Level",
    "Base HP",
    "Strength",
    "Dexterity",
    "Constitution",
    "Agility",
    "Charisma",
    "Intelligence",
    "Power Rating",
    "Tier",
];

/// Optional v1 locomotion/render columns — defaults apply when absent from header or blank.
pub const OPTIONAL_COLUMNS: &[&str] = &[
    "File Path",
    "Move Speed",
    "Collision Radius",
    "Max Slope",
    "Enabled",
];

/// Computed workbook column — never imported as authoritative data.
pub const IGNORED_COLUMNS: &[&str] = &["Total Stats"];

pub const DEFAULT_MOVE_SPEED_MPS: f32 = 4.0;
pub const DEFAULT_COLLISION_RADIUS_METERS: f32 = 0.5;
pub const DEFAULT_MAX_SLOPE_DEGREES: f32 = 40.0;

/// Raw row parsed from the `Units` sheet before validation.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitImportRow {
    pub row_number: usize,
    pub unit_id: String,
    pub name: String,
    pub faction: String,
    pub level: u32,
    pub base_hp: u32,
    pub strength: u32,
    pub dexterity: u32,
    pub constitution: u32,
    pub agility: u32,
    pub charisma: u32,
    pub intelligence: u32,
    pub power_rating: f32,
    pub tier: String,
    pub file_path: String,
    pub move_speed_mps: f32,
    pub collision_radius_meters: f32,
    pub max_slope_degrees: f32,
    pub enabled: bool,
    pub enabled_was_blank: bool,
    pub has_file_path_column: bool,
}

/// Normalize a workbook file-path cell into a [`UnitRenderKey`] path segment (`wolf`).
pub fn normalize_file_path_to_render_key(path: &str) -> Result<String, String> {
    let mut key = normalize_file_path(path);
    if key.is_empty() {
        return Err("File Path must be non-empty when provided".to_string());
    }
    for prefix in ["assets/units/", "units/"] {
        if let Some(rest) = key.strip_prefix(prefix) {
            key = rest.to_string();
        }
    }
    for ext in [".glb", ".gltf"] {
        if let Some(stripped) = key.strip_suffix(ext) {
            key = stripped.to_string();
        }
    }
    if key.is_empty() {
        return Err("File Path must resolve to a render key".to_string());
    }
    Ok(key)
}

impl UnitImportRow {
    pub fn to_definition(&self) -> Result<UnitDefinition, String> {
        let render_key = if self.file_path.trim().is_empty() {
            UnitRenderKey::unset()
        } else {
            UnitRenderKey::reserved(normalize_file_path_to_render_key(&self.file_path)?)
        };

        Ok(UnitDefinition::new(
            UnitDefinitionId::new(self.unit_id.trim()),
            self.name.trim(),
            self.faction.trim(),
            self.level,
            self.base_hp,
            self.strength,
            self.dexterity,
            self.constitution,
            self.agility,
            self.charisma,
            self.intelligence,
            self.power_rating,
            self.tier.trim(),
            self.move_speed_mps,
            self.collision_radius_meters,
            self.max_slope_degrees,
            self.enabled,
            render_key,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row() -> UnitImportRow {
        UnitImportRow {
            row_number: 2,
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
            file_path: r"\units\wolf.glb".to_string(),
            move_speed_mps: 4.5,
            collision_radius_meters: 0.6,
            max_slope_degrees: 40.0,
            enabled: true,
            enabled_was_blank: false,
            has_file_path_column: true,
        }
    }

    #[test]
    fn file_path_normalization() {
        assert_eq!(
            normalize_file_path(r"\units\wolf.glb"),
            "units/wolf.glb"
        );
        assert_eq!(
            normalize_file_path_to_render_key(r"\units\wolf.glb").unwrap(),
            "wolf"
        );
        assert_eq!(
            normalize_file_path_to_render_key(r"\units\wolf").unwrap(),
            "wolf"
        );
    }

    #[test]
    fn converts_row_to_definition_preserving_stats() {
        let def = sample_row().to_definition().unwrap();
        assert_eq!(def.id.as_str(), "U-0001");
        assert_eq!(def.display_name, "Wolf");
        assert_eq!(def.faction_tag, "Wild");
        assert_eq!(def.level, 2);
        assert_eq!(def.base_hp, 5);
        assert_eq!(def.strength, 4);
        assert_eq!(def.dexterity, 6);
        assert_eq!(def.constitution, 3);
        assert_eq!(def.agility, 7);
        assert_eq!(def.charisma, 2);
        assert_eq!(def.intelligence, 3);
        assert!((def.power_rating - 26.5).abs() < 1e-4);
        assert_eq!(def.tier, "Elite");
        assert!((def.move_speed_mps - 4.5).abs() < 1e-4);
        assert!((def.collision_radius_meters - 0.6).abs() < 1e-4);
        assert_eq!(def.render_key.0.as_deref(), Some("wolf"));
    }

    #[test]
    fn blank_file_path_yields_unset_render_key() {
        let mut row = sample_row();
        row.file_path = String::new();
        let def = row.to_definition().unwrap();
        assert_eq!(def.render_key, UnitRenderKey::unset());
    }
}
