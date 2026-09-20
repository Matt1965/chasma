//! Semantic → technical morph weight resolution (CG2).

use std::collections::{BTreeMap, HashMap, HashSet};

use super::definition::{AppearanceProfile, MorphMappingSide, MorphTargetMapping};
use super::id::{AppearanceParamId, BodyVariantId};

/// Semantic parameters driven by morph targets on the human body (not height).
pub const HUMAN_MORPH_SEMANTIC_PARAMS: &[&str] = &[
    "build",
    "fat",
    "muscle",
    "head_size",
    "shoulders",
    "torso",
    "arms",
    "hips",
    "legs",
];

/// Deprecated alias — use [`HUMAN_MORPH_SEMANTIC_PARAMS`].
pub const CG2_MORPH_SEMANTIC_PARAMS: &[&str] = HUMAN_MORPH_SEMANTIC_PARAMS;

#[derive(Debug, Clone, PartialEq)]
pub enum MorphResolveError {
    UnknownVariant { profile: String, variant: String },
    UnknownParameter { profile: String, param: String },
    NonFiniteValue { param: String, value: f32 },
    MissingMapping {
        profile: String,
        variant: String,
        param: String,
    },
    UnknownTechnicalTarget {
        profile: String,
        variant: String,
        target: String,
    },
    DuplicateTechnicalTarget {
        profile: String,
        variant: String,
        target: String,
    },
}

impl std::fmt::Display for MorphResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownVariant { profile, variant } => {
                write!(f, "unknown body variant `{variant}` for profile `{profile}`")
            }
            Self::UnknownParameter { profile, param } => {
                write!(f, "unknown semantic parameter `{param}` for profile `{profile}`")
            }
            Self::NonFiniteValue { param, value } => {
                write!(f, "semantic parameter `{param}` value {value} is not finite")
            }
            Self::MissingMapping {
                profile,
                variant,
                param,
            } => write!(
                f,
                "missing morph mapping for profile `{profile}` variant `{variant}` parameter `{param}`"
            ),
            Self::UnknownTechnicalTarget {
                profile,
                variant,
                target,
            } => write!(
                f,
                "workbook mapping references unknown technical target `{target}` (profile `{profile}` variant `{variant}`)"
            ),
            Self::DuplicateTechnicalTarget {
                profile,
                variant,
                target,
            } => write!(
                f,
                "duplicate morph mapping for target `{target}` (profile `{profile}` variant `{variant}`)"
            ),
        }
    }
}

impl std::error::Error for MorphResolveError {}

/// Resolve semantic appearance values into a weight vector aligned with `target_names` order.
pub fn resolve_morph_weights(
    profile: &AppearanceProfile,
    variant_id: &BodyVariantId,
    semantic_values: &BTreeMap<AppearanceParamId, f32>,
    target_names: &[String],
) -> Result<Vec<f32>, MorphResolveError> {
    let profile_id = profile.id.as_str();
    let variant = variant_id.as_str();
    if profile.body_variant(variant_id).is_none() {
        return Err(MorphResolveError::UnknownVariant {
            profile: profile_id.to_string(),
            variant: variant.to_string(),
        });
    }

    let mappings = mappings_for_variant(profile, variant_id);
    validate_human_morph_mappings(profile, variant_id, &mappings)?;

    let mut weights = vec![0.0f32; target_names.len()];
    let name_to_index: HashMap<&str, usize> = target_names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), index))
        .collect();

    for mapping in &mappings {
        if !mapping.enabled {
            continue;
        }
        let Some(index) = name_to_index.get(mapping.technical_target.as_str()) else {
            return Err(MorphResolveError::UnknownTechnicalTarget {
                profile: profile_id.to_string(),
                variant: variant.to_string(),
                target: mapping.technical_target.clone(),
            });
        };
        let parameter = profile.parameter(&mapping.param_id).ok_or_else(|| {
            MorphResolveError::UnknownParameter {
                profile: profile_id.to_string(),
                param: mapping.param_id.as_str().to_string(),
            }
        })?;
        let value = semantic_values
            .get(&mapping.param_id)
            .copied()
            .unwrap_or(parameter.default);
        if !value.is_finite() {
            return Err(MorphResolveError::NonFiniteValue {
                param: mapping.param_id.as_str().to_string(),
                value,
            });
        }
        let contribution = semantic_deviation(parameter, value, mapping.side) * mapping.multiplier;
        weights[*index] = (weights[*index] + contribution).clamp(0.0, 1.0);
    }

    Ok(weights)
}

/// Resolve semantic appearance into armor mesh morph weights, applying only consumed params.
///
/// Unlike body morph resolution, unknown technical targets on the armor mesh are skipped
/// (armor may expose only a subset of body morph vocabulary).
pub fn resolve_equipment_morph_weights(
    profile: &AppearanceProfile,
    variant_id: &BodyVariantId,
    semantic_values: &BTreeMap<AppearanceParamId, f32>,
    target_names: &[String],
    consumed_params: &[AppearanceParamId],
) -> Result<Vec<f32>, MorphResolveError> {
    if consumed_params.is_empty() {
        return Ok(vec![0.0; target_names.len()]);
    }

    let profile_id = profile.id.as_str();
    let variant = variant_id.as_str();
    if profile.body_variant(variant_id).is_none() {
        return Err(MorphResolveError::UnknownVariant {
            profile: profile_id.to_string(),
            variant: variant.to_string(),
        });
    }

    let mappings = mappings_for_variant(profile, variant_id);
    for param in consumed_params {
        if profile.parameter(param).is_none() {
            return Err(MorphResolveError::UnknownParameter {
                profile: profile_id.to_string(),
                param: param.as_str().to_string(),
            });
        }
        let has_mapping = mappings.iter().any(|mapping| mapping.param_id == *param);
        if !has_mapping {
            return Err(MorphResolveError::MissingMapping {
                profile: profile_id.to_string(),
                variant: variant.to_string(),
                param: param.as_str().to_string(),
            });
        }
    }

    let mut weights = vec![0.0f32; target_names.len()];
    if target_names.is_empty() {
        return Ok(weights);
    }
    let name_to_index: HashMap<&str, usize> = target_names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), index))
        .collect();
    let consumed: HashSet<AppearanceParamId> = consumed_params.iter().cloned().collect();

    for mapping in &mappings {
        if !consumed.contains(&mapping.param_id) {
            continue;
        }
        let Some(index) = name_to_index.get(mapping.technical_target.as_str()) else {
            continue;
        };
        let parameter = profile.parameter(&mapping.param_id).ok_or_else(|| {
            MorphResolveError::UnknownParameter {
                profile: profile_id.to_string(),
                param: mapping.param_id.as_str().to_string(),
            }
        })?;
        let value = semantic_values
            .get(&mapping.param_id)
            .copied()
            .unwrap_or(parameter.default);
        if !value.is_finite() {
            return Err(MorphResolveError::NonFiniteValue {
                param: mapping.param_id.as_str().to_string(),
                value,
            });
        }
        let contribution = semantic_deviation(parameter, value, mapping.side) * mapping.multiplier;
        weights[*index] = (weights[*index] + contribution).clamp(0.0, 1.0);
    }

    Ok(weights)
}

fn semantic_deviation(
    parameter: &super::definition::AppearanceParameterDefinition,
    value: f32,
    side: MorphMappingSide,
) -> f32 {
    let default = parameter.default;
    match side {
        MorphMappingSide::AboveDefault => {
            if value <= default || parameter.max <= default {
                0.0
            } else {
                (value - default) / (parameter.max - default)
            }
        }
        MorphMappingSide::BelowDefault => {
            if value >= default || default <= parameter.min {
                0.0
            } else {
                (default - value) / (default - parameter.min)
            }
        }
    }
}

fn mappings_for_variant(
    profile: &AppearanceProfile,
    variant_id: &BodyVariantId,
) -> Vec<MorphTargetMapping> {
    profile
        .morph_mappings
        .iter()
        .filter(|mapping| mapping.variant_id == *variant_id && mapping.enabled)
        .cloned()
        .collect()
}

fn validate_human_morph_mappings(
    profile: &AppearanceProfile,
    variant_id: &BodyVariantId,
    mappings: &[MorphTargetMapping],
) -> Result<(), MorphResolveError> {
    let profile_id = profile.id.as_str();
    let variant = variant_id.as_str();
    for param_id in HUMAN_MORPH_SEMANTIC_PARAMS {
        let id = AppearanceParamId::new(*param_id);
        if profile.parameter(&id).is_none() {
            continue;
        }
        let has_mapping = mappings.iter().any(|mapping| mapping.param_id == id);
        if !has_mapping {
            return Err(MorphResolveError::MissingMapping {
                profile: profile_id.to_string(),
                variant: variant.to_string(),
                param: param_id.to_string(),
            });
        }
    }
    Ok(())
}

/// Validate workbook mappings against profile structure (import time).
pub fn validate_profile_morph_mappings(profile: &AppearanceProfile) -> Result<(), String> {
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for mapping in &profile.morph_mappings {
        if !mapping.enabled {
            continue;
        }
        if profile.body_variant(&mapping.variant_id).is_none() {
            return Err(format!(
                "morph mapping references unknown variant `{}` in profile `{}`",
                mapping.variant_id.as_str(),
                profile.id.as_str()
            ));
        }
        if profile.parameter(&mapping.param_id).is_none() {
            return Err(format!(
                "morph mapping references unknown parameter `{}` in profile `{}`",
                mapping.param_id.as_str(),
                profile.id.as_str()
            ));
        }
        if !mapping.multiplier.is_finite() || mapping.multiplier < 0.0 {
            return Err(format!(
                "morph mapping multiplier must be finite and non-negative (profile `{}` target `{}`)",
                profile.id.as_str(),
                mapping.technical_target
            ));
        }
        if mapping.technical_target.trim().is_empty() {
            return Err(format!(
                "morph mapping target name must be non-empty (profile `{}`)",
                profile.id.as_str()
            ));
        }
        let key = (
            mapping.variant_id.as_str().to_string(),
            mapping.technical_target.clone(),
        );
        if !seen.insert(key) {
            return Err(format!(
                "duplicate morph mapping for target `{}` on variant `{}` in profile `{}`",
                mapping.technical_target,
                mapping.variant_id.as_str(),
                profile.id.as_str()
            ));
        }
    }

    for variant in profile
        .body_variants
        .iter()
        .filter(|variant| variant.enabled)
    {
        let mappings = mappings_for_variant(profile, &variant.id);
        if let Err(error) = validate_human_morph_mappings(profile, &variant.id, &mappings) {
            return Err(error.to_string());
        }
    }
    Ok(())
}

/// Ordered technical target names expected for human unit GLBs.
pub const HUMAN_MORPH_TARGET_NAMES: &[&str] = &[
    "build_broad",
    "build_narrow",
    "fat_soft",
    "muscle_define",
    "head_large",
    "head_small",
    "shoulders_broad",
    "shoulders_narrow",
    "torso_broad",
    "torso_narrow",
    "arms_thick",
    "arms_thin",
    "hips_broad",
    "hips_narrow",
    "legs_thick",
    "legs_thin",
];
