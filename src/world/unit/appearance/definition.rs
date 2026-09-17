use bevy::prelude::*;

use crate::world::relationship::SpeciesId;
use crate::world::UnitRenderKey;

use super::id::{AppearanceParamId, AppearanceProfileId, BodyVariantId};

/// One body variant within an appearance profile.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct BodyVariantDefinition {
    pub id: BodyVariantId,
    pub display_name: String,
    pub render_key: UnitRenderKey,
    pub enabled: bool,
}

/// Which side of a parameter default activates a morph target mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum MorphMappingSide {
    AboveDefault,
    BelowDefault,
}

/// Maps one semantic parameter to one technical glTF morph target for a body variant.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct MorphTargetMapping {
    pub variant_id: BodyVariantId,
    pub param_id: AppearanceParamId,
    pub technical_target: String,
    pub side: MorphMappingSide,
    pub multiplier: f32,
    pub enabled: bool,
}

/// Semantic customization parameter authored for one profile.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct AppearanceParameterDefinition {
    pub id: AppearanceParamId,
    pub display_name: String,
    pub category: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub display_order: u32,
    pub enabled: bool,
}

/// Authoritative appearance schema for one species/body plan.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct AppearanceProfile {
    pub id: AppearanceProfileId,
    pub species_id: SpeciesId,
    pub schema_version: u32,
    pub height_scale_min: f32,
    pub height_scale_max: f32,
    pub height_scale_default: f32,
    pub body_variants: Vec<BodyVariantDefinition>,
    pub parameters: Vec<AppearanceParameterDefinition>,
    pub morph_mappings: Vec<MorphTargetMapping>,
    pub enabled: bool,
}

impl AppearanceProfile {
    pub fn body_variant(&self, id: &BodyVariantId) -> Option<&BodyVariantDefinition> {
        self.body_variants
            .iter()
            .find(|variant| variant.id == *id && variant.enabled)
    }

    pub fn parameter(&self, id: &AppearanceParamId) -> Option<&AppearanceParameterDefinition> {
        self.parameters
            .iter()
            .find(|parameter| parameter.id == *id && parameter.enabled)
    }

    pub fn enabled_parameters(&self) -> Vec<&AppearanceParameterDefinition> {
        self.parameters
            .iter()
            .filter(|parameter| parameter.enabled)
            .collect()
    }
}
