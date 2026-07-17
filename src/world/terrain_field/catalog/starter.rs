//! Initial terrain field definitions for TF1.

use bevy::prelude::*;

use super::super::category::TerrainFieldCategory;
use super::super::definition::TerrainFieldDefinition;
use super::super::overlay::TerrainFieldOverlayStyle;
use super::super::semantics::FieldValueSemantics;

pub fn starter_definitions() -> Vec<TerrainFieldDefinition> {
    vec![
        water_definition(),
        iron_definition(),
        copper_definition(),
        stone_definition(),
    ]
}

fn water_definition() -> TerrainFieldDefinition {
    TerrainFieldDefinition::new(
        "water",
        "Water",
        TerrainFieldCategory::Hydrological,
        FieldValueSemantics::EnvironmentalPotential,
    )
    .with_description("Relative accessible water potential for roots, wells, and irrigation.")
    .with_overlay_style(TerrainFieldOverlayStyle {
        enabled: true,
        low_color: Color::srgba(0.55, 0.45, 0.25, 1.0),
        mid_color: Some(Color::srgba(0.2, 0.45, 0.85, 1.0)),
        high_color: Color::srgba(0.1, 0.2, 0.9, 1.0),
        default_opacity: 0.55,
        visibility_cutoff: 2_000,
        qualitative_thresholds: vec![8_192, 32_768, 52_000],
        qualitative_labels: vec!["Dry".to_string(), "Moderate".to_string(), "Wet".to_string()],
        icon_key: Some("water".to_string()),
    })
    .with_source_profile_id("water_generated_v1")
}

fn iron_definition() -> TerrainFieldDefinition {
    TerrainFieldDefinition::new(
        "iron",
        "Iron",
        TerrainFieldCategory::Geological,
        FieldValueSemantics::GeologicalPotential,
    )
    .with_description("Relative extractable iron potential beneath the terrain.")
    .with_overlay_style(TerrainFieldOverlayStyle {
        enabled: true,
        low_color: Color::srgba(0.2, 0.15, 0.12, 1.0),
        mid_color: Some(Color::srgba(0.55, 0.35, 0.25, 1.0)),
        high_color: Color::srgba(0.85, 0.45, 0.2, 1.0),
        default_opacity: 0.55,
        visibility_cutoff: 2_000,
        qualitative_thresholds: vec![8_192, 32_768, 52_000],
        qualitative_labels: vec!["Poor".to_string(), "Fair".to_string(), "Rich".to_string()],
        icon_key: Some("iron".to_string()),
    })
    .with_source_profile_id("iron_generated_v1")
}

fn copper_definition() -> TerrainFieldDefinition {
    TerrainFieldDefinition::new(
        "copper",
        "Copper",
        TerrainFieldCategory::Geological,
        FieldValueSemantics::GeologicalPotential,
    )
    .with_description("Relative extractable copper potential beneath the terrain.")
    .with_overlay_style(TerrainFieldOverlayStyle {
        enabled: true,
        low_color: Color::srgba(0.15, 0.12, 0.1, 1.0),
        mid_color: Some(Color::srgba(0.45, 0.35, 0.2, 1.0)),
        high_color: Color::srgba(0.75, 0.5, 0.15, 1.0),
        default_opacity: 0.55,
        visibility_cutoff: 2_000,
        qualitative_thresholds: vec![8_192, 32_768, 52_000],
        qualitative_labels: vec!["Poor".to_string(), "Fair".to_string(), "Rich".to_string()],
        icon_key: Some("copper".to_string()),
    })
    .with_source_profile_id("copper_generated_v1")
}

fn stone_definition() -> TerrainFieldDefinition {
    TerrainFieldDefinition::new(
        "stone",
        "Stone",
        TerrainFieldCategory::Geological,
        FieldValueSemantics::Suitability,
    )
    .with_description("Relative quarry suitability and accessible stone potential.")
    .with_overlay_style(TerrainFieldOverlayStyle {
        enabled: true,
        low_color: Color::srgba(0.2, 0.2, 0.2, 1.0),
        mid_color: Some(Color::srgba(0.45, 0.45, 0.45, 1.0)),
        high_color: Color::srgba(0.75, 0.75, 0.75, 1.0),
        default_opacity: 0.5,
        visibility_cutoff: 2_000,
        qualitative_thresholds: vec![8_192, 32_768, 52_000],
        qualitative_labels: vec!["Poor".to_string(), "Fair".to_string(), "Rich".to_string()],
        icon_key: Some("stone".to_string()),
    })
    .with_source_profile_id("stone_generated_v1")
}
