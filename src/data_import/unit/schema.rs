//! Excel column schema and conversion into [`UnitDefinition`].

use crate::world::{AnimationProfile, AnimationProfileId};
use crate::world::{UnitDefinition, UnitDefinitionId, UnitRenderKey, WeaponDefinitionId};

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

/// Optional v1 locomotion/render/combat columns — defaults apply when absent from header or blank.
pub const OPTIONAL_COLUMNS: &[&str] = &[
    "Max HP",
    "Default Weapon ID",
    "File Path",
    "Move Speed",
    "Collision Radius",
    "Max Slope",
    "Render Scale",
    "Animation Profile",
    "Enabled",
];

/// Computed workbook column — never imported as authoritative data.
pub const IGNORED_COLUMNS: &[&str] = &["Total Stats"];

pub const DEFAULT_MOVE_SPEED_MPS: f32 = 4.0;
pub const DEFAULT_COLLISION_RADIUS_METERS: f32 = 0.5;
pub const DEFAULT_MAX_SLOPE_DEGREES: f32 = 40.0;
pub const DEFAULT_RENDER_SCALE: f32 = 1.0;
/// `robot.glb` mesh bounds are ~0.81 m tall; scale to ~1.75 m humanoid when the sheet omits Render Scale.
pub const ROBOT_DEFAULT_RENDER_SCALE: f32 = 2.15;
/// Used when the workbook has no `Default Weapon ID` column or the cell is blank.
pub const DEFAULT_WEAPON_ID_WHEN_UNSPECIFIED: &str = "weapon_fists";

/// Raw row parsed from the `Units` sheet before validation.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitImportRow {
    pub row_number: usize,
    pub unit_id: String,
    pub name: String,
    pub faction: String,
    pub level: u32,
    pub base_hp: u32,
    pub max_hp: u32,
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
    pub render_scale: f32,
    pub default_weapon_id: String,
    pub enabled: bool,
    pub enabled_was_blank: bool,
    pub has_file_path_column: bool,
    pub has_default_weapon_column: bool,
    pub has_render_scale_column: bool,
    pub animation_profile: String,
    pub has_animation_profile_column: bool,
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

        let mut definition = UnitDefinition::new(
            UnitDefinitionId::new(self.unit_id.trim()),
            self.name.trim(),
            self.faction.trim(),
            self.level,
            self.base_hp,
            self.max_hp,
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
            WeaponDefinitionId::new(self.resolved_default_weapon_id()),
            self.enabled,
            render_key,
        );
        definition.render_scale = self.resolved_render_scale();
        definition.animation_profile_id = self.resolved_animation_profile_id();
        Ok(definition)
    }

    fn resolved_animation_profile_id(&self) -> Option<AnimationProfileId> {
        if !self.has_animation_profile_column {
            return None;
        }
        let trimmed = self.animation_profile.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(AnimationProfileId::new(trimmed))
        }
    }

    fn resolved_default_weapon_id(&self) -> &str {
        if self.default_weapon_id.trim().is_empty() {
            DEFAULT_WEAPON_ID_WHEN_UNSPECIFIED
        } else {
            self.default_weapon_id.trim()
        }
    }

    fn resolved_render_scale(&self) -> f32 {
        if self.has_render_scale_column {
            return self.render_scale;
        }
        if !self.file_path.trim().is_empty() {
            if let Ok(key) = normalize_file_path_to_render_key(&self.file_path) {
                if key == "robot" {
                    return ROBOT_DEFAULT_RENDER_SCALE;
                }
            }
        }
        DEFAULT_RENDER_SCALE
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
            max_hp: 5,
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
            render_scale: DEFAULT_RENDER_SCALE,
            default_weapon_id: "weapon_wolf_bite".to_string(),
            enabled: true,
            enabled_was_blank: false,
            has_file_path_column: true,
            has_default_weapon_column: true,
            has_render_scale_column: false,
            animation_profile: String::new(),
            has_animation_profile_column: false,
        }
    }

    #[test]
    fn file_path_normalization() {
        assert_eq!(normalize_file_path(r"\units\wolf.glb"), "units/wolf.glb");
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
        assert!((def.render_scale - DEFAULT_RENDER_SCALE).abs() < 1e-4);
        assert_eq!(def.render_key.0.as_deref(), Some("wolf"));
        assert_eq!(def.default_weapon_id.as_str(), "weapon_wolf_bite");
        assert_eq!(def.max_hp, 5);
    }

    #[test]
    fn blank_file_path_yields_unset_render_key() {
        let mut row = sample_row();
        row.file_path = String::new();
        let def = row.to_definition().unwrap();
        assert_eq!(def.render_key, UnitRenderKey::unset());
    }

    #[test]
    fn robot_without_render_scale_column_uses_humanoid_default() {
        let mut row = sample_row();
        row.file_path = r"\units\robot.glb".to_string();
        row.name = "Robot".to_string();
        row.has_render_scale_column = false;
        let def = row.to_definition().unwrap();
        assert!((def.render_scale - ROBOT_DEFAULT_RENDER_SCALE).abs() < 1e-4);
    }

    #[test]
    fn explicit_render_scale_column_is_preserved() {
        let mut row = sample_row();
        row.has_render_scale_column = true;
        row.render_scale = 1.75;
        let def = row.to_definition().unwrap();
        assert!((def.render_scale - 1.75).abs() < 1e-4);
    }
}
