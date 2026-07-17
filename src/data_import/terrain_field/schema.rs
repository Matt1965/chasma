//! Excel column schema for terrain fields (ADR-101 TF1).

use bevy::prelude::*;

use crate::world::{
    FieldValueSemantics, TerrainFieldCategory, TerrainFieldDefinition, TerrainFieldId,
    TerrainFieldOverlayStyle, TerrainFieldSourceProfileId, validate_terrain_field_id,
};

pub const REQUIRED_COLUMNS: &[&str] = &[
    "Terrain Field ID",
    "Name",
    "Category",
    "Value Semantics",
    "Enabled",
];

pub const OPTIONAL_COLUMNS: &[&str] = &[
    "Description",
    "Overlay Enabled",
    "Overlay Low Color",
    "Overlay Mid Color",
    "Overlay High Color",
    "Overlay Opacity",
    "Visibility Cutoff",
    "Qualitative Thresholds",
    "Qualitative Labels",
    "Source Profile ID",
    "Icon Key",
];

#[derive(Debug, Clone, PartialEq)]
pub struct TerrainFieldImportRow {
    pub row_number: usize,
    pub field_id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub value_semantics: String,
    pub enabled: bool,
    pub enabled_was_blank: bool,
    pub overlay_enabled: Option<bool>,
    pub overlay_low_color: Option<String>,
    pub overlay_mid_color: Option<String>,
    pub overlay_high_color: Option<String>,
    pub overlay_opacity: Option<f32>,
    pub visibility_cutoff: Option<u16>,
    pub qualitative_thresholds: Option<String>,
    pub qualitative_labels: Option<String>,
    pub source_profile_id: Option<String>,
    pub icon_key: Option<String>,
}

impl TerrainFieldImportRow {
    pub fn to_definition(&self) -> Result<TerrainFieldDefinition, String> {
        validate_terrain_field_id(self.field_id.trim()).map_err(|err| err.to_string())?;
        let category = TerrainFieldCategory::parse(&self.category)
            .ok_or_else(|| format!("invalid category `{}`", self.category))?;
        let value_semantics = FieldValueSemantics::parse(&self.value_semantics)
            .ok_or_else(|| format!("invalid value semantics `{}`", self.value_semantics))?;

        let mut overlay = TerrainFieldOverlayStyle::default();
        if let Some(enabled) = self.overlay_enabled {
            overlay.enabled = enabled;
        }
        if let Some(text) = &self.overlay_low_color {
            overlay.low_color = parse_color_cell(text)?;
        }
        if let Some(text) = &self.overlay_mid_color {
            overlay.mid_color = Some(parse_color_cell(text)?);
        }
        if let Some(text) = &self.overlay_high_color {
            overlay.high_color = parse_color_cell(text)?;
        }
        if let Some(opacity) = self.overlay_opacity {
            overlay.default_opacity = opacity;
        }
        if let Some(cutoff) = self.visibility_cutoff {
            overlay.visibility_cutoff = cutoff;
        }
        if let Some(text) = &self.qualitative_thresholds {
            overlay.qualitative_thresholds = parse_u16_list(text)?;
        }
        if let Some(text) = &self.qualitative_labels {
            overlay.qualitative_labels = parse_label_list(text);
        }
        if let Some(key) = &self.icon_key {
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                overlay.icon_key = Some(trimmed.to_string());
            }
        }
        overlay.validate().map_err(|err| err.to_string())?;

        let mut definition = TerrainFieldDefinition::new(
            self.field_id.trim(),
            self.name.trim(),
            category,
            value_semantics,
        )
        .with_description(self.description.trim())
        .with_overlay_style(overlay);
        definition.enabled = self.enabled;
        if let Some(profile) = &self.source_profile_id {
            let trimmed = profile.trim();
            if !trimmed.is_empty() {
                definition = definition.with_source_profile_id(trimmed);
            }
        }
        definition.id = TerrainFieldId::new(self.field_id.trim());
        Ok(definition)
    }
}

pub fn parse_color_cell(text: &str) -> Result<Color, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("color cell is empty".to_string());
    }
    if let Some(hex) = trimmed.strip_prefix('#') {
        if hex.len() != 6 && hex.len() != 8 {
            return Err(format!("invalid hex color `{trimmed}`"));
        }
        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "invalid hex color")?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "invalid hex color")?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "invalid hex color")?;
        let a = if hex.len() == 8 {
            u8::from_str_radix(&hex[6..8], 16).map_err(|_| "invalid hex color")?
        } else {
            255
        };
        return Ok(Color::srgba(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        ));
    }
    let parts: Vec<&str> = trimmed.split(',').map(str::trim).collect();
    if parts.len() < 3 {
        return Err(format!("expected r,g,b color components in `{trimmed}`"));
    }
    let r: f32 = parts[0].parse().map_err(|_| "invalid color component")?;
    let g: f32 = parts[1].parse().map_err(|_| "invalid color component")?;
    let b: f32 = parts[2].parse().map_err(|_| "invalid color component")?;
    let a = if parts.len() > 3 {
        parts[3].parse().map_err(|_| "invalid color alpha")?
    } else {
        1.0
    };
    Ok(Color::srgba(r, g, b, a))
}

fn parse_u16_list(text: &str) -> Result<Vec<u16>, String> {
    let mut values = Vec::new();
    for part in text.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        values.push(
            trimmed
                .parse::<u16>()
                .map_err(|_| format!("invalid threshold `{trimmed}`"))?,
        );
    }
    Ok(values)
}

fn parse_label_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}
