//! CG2 morph resolver tests.

use std::collections::BTreeMap;

use super::definition::{
    AppearanceParameterDefinition, AppearanceProfile, BodyVariantDefinition, MorphMappingSide,
    MorphTargetMapping,
};
use super::id::{AppearanceParamId, AppearanceProfileId, BodyVariantId};
use super::morph::{
    HUMAN_MORPH_TARGET_NAMES, resolve_equipment_morph_weights, resolve_morph_weights,
};
use crate::world::{SpeciesId, UnitRenderKey};

fn human_profile() -> AppearanceProfile {
    AppearanceProfile {
        id: AppearanceProfileId::new("human"),
        species_id: SpeciesId::new("human"),
        schema_version: 1,
        height_scale_min: 0.85,
        height_scale_max: 1.15,
        height_scale_default: 1.0,
        body_variants: vec![
            BodyVariantDefinition {
                id: BodyVariantId::new("human_male"),
                display_name: "Male".into(),
                render_key: UnitRenderKey::reserved("human_male"),
                enabled: true,
            },
            BodyVariantDefinition {
                id: BodyVariantId::new("human_female"),
                display_name: "Female".into(),
                render_key: UnitRenderKey::reserved("human_female"),
                enabled: true,
            },
        ],
        parameters: vec![
            param("build", 0.0, 1.0, 0.5),
            param("fat", 0.0, 1.0, 0.35),
            param("muscle", 0.0, 1.0, 0.45),
            param("head_size", 0.0, 1.0, 0.5),
        ],
        morph_mappings: human_mappings("human_male"),
        enabled: true,
    }
}

fn param(id: &str, min: f32, max: f32, default: f32) -> AppearanceParameterDefinition {
    AppearanceParameterDefinition {
        id: AppearanceParamId::new(id),
        display_name: id.into(),
        category: "Body".into(),
        min,
        max,
        default,
        display_order: 1,
        enabled: true,
    }
}

fn human_mappings(variant: &str) -> Vec<MorphTargetMapping> {
    [
        ("build", "build_broad", MorphMappingSide::AboveDefault),
        ("build", "build_narrow", MorphMappingSide::BelowDefault),
        ("fat", "fat_soft", MorphMappingSide::AboveDefault),
        ("muscle", "muscle_define", MorphMappingSide::AboveDefault),
        ("head_size", "head_large", MorphMappingSide::AboveDefault),
        ("head_size", "head_small", MorphMappingSide::BelowDefault),
    ]
        .map(|(param, target, side)| MorphTargetMapping {
            variant_id: BodyVariantId::new(variant),
            param_id: AppearanceParamId::new(param),
            technical_target: target.into(),
            side,
            multiplier: 1.0,
            enabled: true,
        })
        .to_vec()
}

fn target_names() -> Vec<String> {
    HUMAN_MORPH_TARGET_NAMES
        .iter()
        .map(|name| name.to_string())
        .collect()
}

#[test]
fn defaults_resolve_to_neutral_weights() {
    let profile = human_profile();
    let values = profile
        .parameters
        .iter()
        .map(|p| (p.id.clone(), p.default))
        .collect();
    let weights = resolve_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &target_names(),
    )
    .unwrap();
    assert!(weights.iter().all(|w| *w < 1e-4));
}

#[test]
fn low_build_activates_narrow_target() {
    let profile = human_profile();
    let mut values = BTreeMap::new();
    values.insert(AppearanceParamId::new("build"), 0.0);
    for param in &profile.parameters {
        values.entry(param.id.clone()).or_insert(param.default);
    }
    let weights = resolve_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &target_names(),
    )
    .unwrap();
    assert!(weights[1] > 0.9);
    assert!(weights[0] < 0.1);
}

#[test]
fn high_build_activates_broad_target() {
    let profile = human_profile();
    let mut values = BTreeMap::new();
    values.insert(AppearanceParamId::new("build"), 1.0);
    for param in &profile.parameters {
        values.entry(param.id.clone()).or_insert(param.default);
    }
    let weights = resolve_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &target_names(),
    )
    .unwrap();
    assert!(weights[0] > 0.9);
    assert!(weights[1] < 0.1);
}

#[test]
fn equipment_resolve_filters_consumed_params() {
    let profile = human_profile();
    let mut values = BTreeMap::new();
    values.insert(AppearanceParamId::new("build"), 1.0);
    values.insert(AppearanceParamId::new("head_size"), 1.0);
    let torso_targets = vec![
        "build_broad".to_string(),
        "build_narrow".to_string(),
        "fat_soft".to_string(),
        "muscle_define".to_string(),
    ];
    let consumed = vec![
        AppearanceParamId::new("build"),
        AppearanceParamId::new("fat"),
        AppearanceParamId::new("muscle"),
    ];
    let weights = resolve_equipment_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &torso_targets,
        &consumed,
    )
    .unwrap();
    assert!(weights[0] > 0.9, "build_broad should activate");
    assert!(weights[1] < 0.1, "build_narrow should stay inactive");
}

#[test]
fn equipment_resolve_ignores_unconsumed_head_size_on_torso_mesh() {
    let profile = human_profile();
    let mut values = BTreeMap::new();
    values.insert(AppearanceParamId::new("head_size"), 1.0);
    let torso_targets = vec![
        "build_broad".to_string(),
        "build_narrow".to_string(),
        "fat_soft".to_string(),
        "muscle_define".to_string(),
    ];
    let consumed = vec![
        AppearanceParamId::new("build"),
        AppearanceParamId::new("fat"),
        AppearanceParamId::new("muscle"),
    ];
    let weights = resolve_equipment_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &torso_targets,
        &consumed,
    )
    .unwrap();
    assert!(weights.iter().all(|weight| *weight < 1e-4));
}

#[test]
fn equipment_resolve_uses_armor_target_name_ordering() {
    let profile = human_profile();
    let mut values = BTreeMap::new();
    values.insert(AppearanceParamId::new("build"), 0.0);
    let reordered = vec![
        "fat_soft".to_string(),
        "build_narrow".to_string(),
        "build_broad".to_string(),
    ];
    let consumed = vec![AppearanceParamId::new("build")];
    let weights = resolve_equipment_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &reordered,
        &consumed,
    )
    .unwrap();
    assert!(weights[1] > 0.9, "build_narrow index should follow armor mesh names");
}

#[test]
fn missing_technical_target_fails_loudly() {
    let profile = human_profile();
    let values = BTreeMap::new();
    let unknown = vec!["unknown_target".to_string()];
    let err = resolve_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &unknown,
    )
    .unwrap_err();
    assert!(err.to_string().contains("unknown technical target"));
}
