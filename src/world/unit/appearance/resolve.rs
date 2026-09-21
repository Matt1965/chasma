use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::world::unit::catalog::{UnitDefinition, UnitRenderKey};
use crate::world::UnitRecord;

use super::catalog::AppearanceProfileCatalog;
use super::definition::AppearanceProfile;
use super::id::{AppearanceParamId, AppearanceProfileId, BodyVariantId};
use super::record::UnitAppearance;

/// Why appearance resolution or validation failed.
#[derive(Debug, Clone, PartialEq)]
pub enum AppearanceError {
    DefinitionMissingAppearanceProfile {
        definition_id: String,
    },
    DefinitionMissingDefaultBodyVariant {
        definition_id: String,
        profile_id: String,
    },
    UnknownAppearanceProfile {
        profile_id: String,
    },
    DisabledAppearanceProfile {
        profile_id: String,
    },
    UnknownBodyVariant {
        profile_id: String,
        body_variant_id: String,
    },
    DisabledBodyVariant {
        profile_id: String,
        body_variant_id: String,
    },
    BodyVariantMissingRenderKey {
        profile_id: String,
        body_variant_id: String,
    },
    AppearanceProfileMismatch {
        definition_id: String,
        expected_profile_id: String,
        actual_profile_id: String,
    },
    MissingSemanticParameter {
        profile_id: String,
        param_id: String,
    },
    UnknownSemanticParameter {
        profile_id: String,
        param_id: String,
    },
    ParameterOutOfRange {
        profile_id: String,
        param_id: String,
        value: f32,
        min: f32,
        max: f32,
    },
    InvalidHeightScale {
        profile_id: String,
        value: f32,
        min: f32,
        max: f32,
    },
    NonFiniteValue {
        field: &'static str,
    },
}

impl std::fmt::Display for AppearanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DefinitionMissingAppearanceProfile { definition_id } => {
                write!(
                    f,
                    "unit definition `{definition_id}` requires Appearance Profile ID"
                )
            }
            Self::DefinitionMissingDefaultBodyVariant {
                definition_id,
                profile_id,
            } => write!(
                f,
                "unit definition `{definition_id}` requires Default Body Variant ID for profile `{profile_id}`"
            ),
            Self::UnknownAppearanceProfile { profile_id } => {
                write!(f, "unknown appearance profile `{profile_id}`")
            }
            Self::DisabledAppearanceProfile { profile_id } => {
                write!(f, "appearance profile `{profile_id}` is disabled")
            }
            Self::UnknownBodyVariant {
                profile_id,
                body_variant_id,
            } => write!(
                f,
                "unknown body variant `{body_variant_id}` for profile `{profile_id}`"
            ),
            Self::DisabledBodyVariant {
                profile_id,
                body_variant_id,
            } => write!(
                f,
                "disabled body variant `{body_variant_id}` for profile `{profile_id}`"
            ),
            Self::BodyVariantMissingRenderKey {
                profile_id,
                body_variant_id,
            } => write!(
                f,
                "body variant `{body_variant_id}` in profile `{profile_id}` has no render key"
            ),
            Self::AppearanceProfileMismatch {
                definition_id,
                expected_profile_id,
                actual_profile_id,
            } => write!(
                f,
                "unit `{definition_id}` appearance profile `{actual_profile_id}` does not match definition profile `{expected_profile_id}`"
            ),
            Self::MissingSemanticParameter { profile_id, param_id } => write!(
                f,
                "appearance profile `{profile_id}` missing semantic parameter `{param_id}`"
            ),
            Self::UnknownSemanticParameter { profile_id, param_id } => write!(
                f,
                "appearance profile `{profile_id}` has no parameter `{param_id}`"
            ),
            Self::ParameterOutOfRange {
                profile_id,
                param_id,
                value,
                min,
                max,
            } => write!(
                f,
                "appearance parameter `{param_id}` value {value} out of range [{min}, {max}] for profile `{profile_id}`"
            ),
            Self::InvalidHeightScale {
                profile_id,
                value,
                min,
                max,
            } => write!(
                f,
                "height scale {value} out of range [{min}, {max}] for profile `{profile_id}`"
            ),
            Self::NonFiniteValue { field } => write!(f, "appearance field `{field}` must be finite"),
        }
    }
}

impl std::error::Error for AppearanceError {}

pub fn definition_has_appearance_support(definition: &UnitDefinition) -> bool {
    definition.appearance_profile_id.is_some()
}

/// Canonical authored default appearance for a definition with appearance support.
pub fn resolve_canonical_default_appearance(
    definition: &UnitDefinition,
    profiles: &AppearanceProfileCatalog,
) -> Result<UnitAppearance, AppearanceError> {
    let profile_id = definition
        .appearance_profile_id
        .as_ref()
        .ok_or_else(|| AppearanceError::DefinitionMissingAppearanceProfile {
            definition_id: definition.id.as_str().to_string(),
        })?;
    let default_variant = definition
        .default_body_variant_id
        .as_ref()
        .ok_or_else(|| AppearanceError::DefinitionMissingDefaultBodyVariant {
            definition_id: definition.id.as_str().to_string(),
            profile_id: profile_id.as_str().to_string(),
        })?;

    let profile = load_enabled_profile(profiles, profile_id)?;
    validate_height_scale(profile, profile.height_scale_default)?;
    let morphs = default_morph_values(profile)?;

    Ok(UnitAppearance {
        profile_id: profile_id.clone(),
        body_variant_id: default_variant.clone(),
        height_scale: profile.height_scale_default,
        morphs,
        generation_seed: None,
    })
}

/// Validate an instantiated appearance against its definition and profile schema.
pub fn validate_unit_appearance(
    appearance: &UnitAppearance,
    definition: &UnitDefinition,
    profiles: &AppearanceProfileCatalog,
) -> Result<(), AppearanceError> {
    let expected_profile = definition
        .appearance_profile_id
        .as_ref()
        .ok_or_else(|| AppearanceError::DefinitionMissingAppearanceProfile {
            definition_id: definition.id.as_str().to_string(),
        })?;
    if appearance.profile_id != *expected_profile {
        return Err(AppearanceError::AppearanceProfileMismatch {
            definition_id: definition.id.as_str().to_string(),
            expected_profile_id: expected_profile.as_str().to_string(),
            actual_profile_id: appearance.profile_id.as_str().to_string(),
        });
    }

    let profile = load_enabled_profile(profiles, &appearance.profile_id)?;
    let variant = load_enabled_body_variant(
        profile,
        &appearance.body_variant_id,
        &appearance.profile_id,
    )?;
    if variant.render_key.0.as_deref().unwrap_or("").trim().is_empty() {
        return Err(AppearanceError::BodyVariantMissingRenderKey {
            profile_id: appearance.profile_id.as_str().to_string(),
            body_variant_id: appearance.body_variant_id.as_str().to_string(),
        });
    }

    validate_height_scale(profile, appearance.height_scale)?;
    validate_morph_values(profile, &appearance.morphs)?;
    Ok(())
}

/// Effective render key from an appearance instance (CG1/CG3).
pub fn effective_render_key_for_appearance(
    appearance: &UnitAppearance,
    profiles: &AppearanceProfileCatalog,
) -> Result<UnitRenderKey, AppearanceError> {
    let profile = load_enabled_profile(profiles, &appearance.profile_id)?;
    let variant = load_enabled_body_variant(
        profile,
        &appearance.body_variant_id,
        &appearance.profile_id,
    )?;
    let key = variant.render_key.0.as_deref().unwrap_or("").trim();
    if key.is_empty() {
        return Err(AppearanceError::BodyVariantMissingRenderKey {
            profile_id: appearance.profile_id.as_str().to_string(),
            body_variant_id: appearance.body_variant_id.as_str().to_string(),
        });
    }
    Ok(UnitRenderKey::reserved(key))
}

/// Effective render key for an instantiated unit.
pub fn effective_unit_render_key(
    record: &UnitRecord,
    definition: &UnitDefinition,
    profiles: &AppearanceProfileCatalog,
) -> Result<UnitRenderKey, AppearanceError> {
    if let Some(appearance) = &record.appearance {
        return effective_render_key_for_appearance(appearance, profiles);
    }

    if definition_has_appearance_support(definition) {
        return Err(AppearanceError::DefinitionMissingAppearanceProfile {
            definition_id: definition.id.as_str().to_string(),
        });
    }

    Ok(definition.render_key.clone())
}

pub fn effective_unit_render_key_str(
    record: &UnitRecord,
    definition: &UnitDefinition,
    profiles: &AppearanceProfileCatalog,
) -> Result<String, AppearanceError> {
    effective_unit_render_key(record, definition, profiles)?
        .0
        .clone()
        .ok_or_else(|| AppearanceError::BodyVariantMissingRenderKey {
            profile_id: record
                .appearance
                .as_ref()
                .map(|value| value.profile_id.as_str().to_string())
                .unwrap_or_default(),
            body_variant_id: record
                .appearance
                .as_ref()
                .map(|value| value.body_variant_id.as_str().to_string())
                .unwrap_or_default(),
        })
}

fn load_enabled_profile<'a>(
    profiles: &'a AppearanceProfileCatalog,
    profile_id: &AppearanceProfileId,
) -> Result<&'a AppearanceProfile, AppearanceError> {
    let profile = profiles
        .get(profile_id)
        .ok_or_else(|| AppearanceError::UnknownAppearanceProfile {
            profile_id: profile_id.as_str().to_string(),
        })?;
    if !profile.enabled {
        return Err(AppearanceError::DisabledAppearanceProfile {
            profile_id: profile_id.as_str().to_string(),
        });
    }
    Ok(profile)
}

fn load_enabled_body_variant<'a>(
    profile: &'a AppearanceProfile,
    body_variant_id: &BodyVariantId,
    profile_id: &AppearanceProfileId,
) -> Result<&'a super::definition::BodyVariantDefinition, AppearanceError> {
    let variant = profile
        .body_variant(body_variant_id)
        .ok_or_else(|| AppearanceError::UnknownBodyVariant {
            profile_id: profile_id.as_str().to_string(),
            body_variant_id: body_variant_id.as_str().to_string(),
        })?;
    if !variant.enabled {
        return Err(AppearanceError::DisabledBodyVariant {
            profile_id: profile_id.as_str().to_string(),
            body_variant_id: body_variant_id.as_str().to_string(),
        });
    }
    Ok(variant)
}

fn default_morph_values(profile: &AppearanceProfile) -> Result<BTreeMap<AppearanceParamId, f32>, AppearanceError> {
    let mut morphs = BTreeMap::new();
    for parameter in profile.enabled_parameters() {
        if !parameter.default.is_finite() {
            return Err(AppearanceError::NonFiniteValue {
                field: "parameter default",
            });
        }
        if parameter.default < parameter.min || parameter.default > parameter.max {
            return Err(AppearanceError::ParameterOutOfRange {
                profile_id: profile.id.as_str().to_string(),
                param_id: parameter.id.as_str().to_string(),
                value: parameter.default,
                min: parameter.min,
                max: parameter.max,
            });
        }
        morphs.insert(parameter.id.clone(), parameter.default);
    }
    validate_morph_values(profile, &morphs)?;
    Ok(morphs)
}

fn validate_morph_values(
    profile: &AppearanceProfile,
    morphs: &BTreeMap<AppearanceParamId, f32>,
) -> Result<(), AppearanceError> {
    for parameter in profile.enabled_parameters() {
        let value = morphs
            .get(&parameter.id)
            .ok_or_else(|| AppearanceError::MissingSemanticParameter {
                profile_id: profile.id.as_str().to_string(),
                param_id: parameter.id.as_str().to_string(),
            })?;
        if !value.is_finite() {
            return Err(AppearanceError::NonFiniteValue {
                field: "semantic parameter value",
            });
        }
        if *value < parameter.min || *value > parameter.max {
            return Err(AppearanceError::ParameterOutOfRange {
                profile_id: profile.id.as_str().to_string(),
                param_id: parameter.id.as_str().to_string(),
                value: *value,
                min: parameter.min,
                max: parameter.max,
            });
        }
    }

    for (param_id, value) in morphs {
        if profile.parameter(param_id).is_none() {
            return Err(AppearanceError::UnknownSemanticParameter {
                profile_id: profile.id.as_str().to_string(),
                param_id: param_id.as_str().to_string(),
            });
        }
        if !value.is_finite() {
            return Err(AppearanceError::NonFiniteValue {
                field: "semantic parameter value",
            });
        }
    }

    Ok(())
}

fn validate_height_scale(profile: &AppearanceProfile, value: f32) -> Result<(), AppearanceError> {
    if !value.is_finite() {
        return Err(AppearanceError::NonFiniteValue {
            field: "height_scale",
        });
    }
    if value < profile.height_scale_min || value > profile.height_scale_max {
        return Err(AppearanceError::InvalidHeightScale {
            profile_id: profile.id.as_str().to_string(),
            value,
            min: profile.height_scale_min,
            max: profile.height_scale_max,
        });
    }
    Ok(())
}
