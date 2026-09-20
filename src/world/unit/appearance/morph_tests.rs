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
    human_mappings_for_params(
        variant,
        &[
            ("build", "build_broad", MorphMappingSide::AboveDefault),
            ("build", "build_narrow", MorphMappingSide::BelowDefault),
            ("fat", "fat_soft", MorphMappingSide::AboveDefault),
            ("muscle", "muscle_define", MorphMappingSide::AboveDefault),
            ("head_size", "head_large", MorphMappingSide::AboveDefault),
            ("head_size", "head_small", MorphMappingSide::BelowDefault),
        ],
    )
}

fn human_mappings_for_params(
    variant: &str,
    rows: &[(&str, &str, MorphMappingSide)],
) -> Vec<MorphTargetMapping> {
    rows.iter()
        .map(|(param, target, side)| MorphTargetMapping {
            variant_id: BodyVariantId::new(variant),
            param_id: AppearanceParamId::new(*param),
            technical_target: (*target).into(),
            side: *side,
            multiplier: 1.0,
            enabled: true,
        })
        .collect()
}

fn full_human_profile() -> AppearanceProfile {
    let regional = [
        ("shoulders", 0.5),
        ("torso", 0.5),
        ("arms", 0.5),
        ("hips", 0.5),
        ("legs", 0.5),
    ];
    let mut parameters = vec![
        param("build", 0.0, 1.0, 0.5),
        param("fat", 0.0, 1.0, 0.35),
        param("muscle", 0.0, 1.0, 0.45),
        param("head_size", 0.0, 1.0, 0.5),
    ];
    for (id, default) in regional {
        parameters.push(param(id, 0.0, 1.0, default));
    }
    let mut morph_mappings = human_mappings("human_male");
    morph_mappings.extend(human_mappings_for_params(
        "human_male",
        &[
            ("shoulders", "shoulders_broad", MorphMappingSide::AboveDefault),
            ("shoulders", "shoulders_narrow", MorphMappingSide::BelowDefault),
            ("torso", "torso_broad", MorphMappingSide::AboveDefault),
            ("torso", "torso_narrow", MorphMappingSide::BelowDefault),
            ("arms", "arms_thick", MorphMappingSide::AboveDefault),
            ("arms", "arms_thin", MorphMappingSide::BelowDefault),
            ("hips", "hips_broad", MorphMappingSide::AboveDefault),
            ("hips", "hips_narrow", MorphMappingSide::BelowDefault),
            ("legs", "legs_thick", MorphMappingSide::AboveDefault),
            ("legs", "legs_thin", MorphMappingSide::BelowDefault),
        ],
    ));
    AppearanceProfile {
        id: AppearanceProfileId::new("human"),
        species_id: SpeciesId::new("human"),
        schema_version: 1,
        height_scale_min: 0.85,
        height_scale_max: 1.15,
        height_scale_default: 1.0,
        body_variants: vec![BodyVariantDefinition {
            id: BodyVariantId::new("human_male"),
            display_name: "Male".into(),
            render_key: UnitRenderKey::reserved("human_male"),
            enabled: true,
        }],
        parameters,
        morph_mappings,
        enabled: true,
    }
}

fn default_semantic_values(profile: &AppearanceProfile) -> BTreeMap<AppearanceParamId, f32> {
    profile
        .parameters
        .iter()
        .map(|parameter| (parameter.id.clone(), parameter.default))
        .collect()
}

fn target_index(name: &str) -> usize {
    HUMAN_MORPH_TARGET_NAMES
        .iter()
        .position(|value| *value == name)
        .expect("target name")
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
fn regional_neutral_values_produce_zero_contribution() {
    let profile = full_human_profile();
    let values = default_semantic_values(&profile);
    let weights = resolve_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &target_names(),
    )
    .unwrap();
    for index in [
        target_index("shoulders_broad"),
        target_index("shoulders_narrow"),
        target_index("torso_broad"),
        target_index("torso_narrow"),
        target_index("arms_thick"),
        target_index("arms_thin"),
        target_index("hips_broad"),
        target_index("hips_narrow"),
        target_index("legs_thick"),
        target_index("legs_thin"),
    ] {
        assert!(weights[index] < 1e-4, "target index {index}");
    }
}

#[test]
fn broad_shoulders_activate_positive_target_only() {
    let profile = full_human_profile();
    let mut values = default_semantic_values(&profile);
    values.insert(AppearanceParamId::new("shoulders"), 1.0);
    let weights = resolve_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &target_names(),
    )
    .unwrap();
    assert!(weights[target_index("shoulders_broad")] > 0.9);
    assert!(weights[target_index("shoulders_narrow")] < 0.1);
    assert!(weights[target_index("hips_broad")] < 0.1);
    assert!(weights[target_index("legs_thick")] < 0.1);
}

#[test]
fn thin_arms_activate_negative_target_only() {
    let profile = full_human_profile();
    let mut values = default_semantic_values(&profile);
    values.insert(AppearanceParamId::new("arms"), 0.0);
    let weights = resolve_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &target_names(),
    )
    .unwrap();
    assert!(weights[target_index("arms_thin")] > 0.9);
    assert!(weights[target_index("arms_thick")] < 0.1);
    assert!(weights[target_index("torso_broad")] < 0.1);
}

#[test]
fn shoulders_and_muscle_compose_independently() {
    let profile = full_human_profile();
    let mut values = default_semantic_values(&profile);
    values.insert(AppearanceParamId::new("shoulders"), 1.0);
    values.insert(AppearanceParamId::new("muscle"), 1.0);
    let weights = resolve_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &target_names(),
    )
    .unwrap();
    assert!(weights[target_index("shoulders_broad")] > 0.9);
    assert!(weights[target_index("muscle_define")] > 0.9);
}

#[test]
fn torso_and_fat_compose_independently() {
    let profile = full_human_profile();
    let mut values = default_semantic_values(&profile);
    values.insert(AppearanceParamId::new("torso"), 1.0);
    values.insert(AppearanceParamId::new("fat"), 1.0);
    let weights = resolve_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &target_names(),
    )
    .unwrap();
    assert!(weights[target_index("torso_broad")] > 0.9);
    assert!(weights[target_index("fat_soft")] > 0.9);
}

#[test]
fn regional_values_resolve_independently_per_actor() {
    let profile = full_human_profile();
    let mut broad_shoulders = default_semantic_values(&profile);
    broad_shoulders.insert(AppearanceParamId::new("shoulders"), 1.0);
    let mut narrow_shoulders = default_semantic_values(&profile);
    narrow_shoulders.insert(AppearanceParamId::new("shoulders"), 0.0);
    let broad = resolve_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &broad_shoulders,
        &target_names(),
    )
    .unwrap();
    let narrow = resolve_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &narrow_shoulders,
        &target_names(),
    )
    .unwrap();
    assert!(broad[target_index("shoulders_broad")] > 0.9);
    assert!(narrow[target_index("shoulders_narrow")] > 0.9);
    assert_ne!(
        broad[target_index("shoulders_broad")],
        narrow[target_index("shoulders_broad")]
    );
}

#[test]
fn equipment_body_resolve_activates_regional_shoulders_only() {
    let profile = full_human_profile();
    let mut values = default_semantic_values(&profile);
    values.insert(AppearanceParamId::new("shoulders"), 1.0);
    let body_targets = vec![
        "build_broad".to_string(),
        "build_narrow".to_string(),
        "fat_soft".to_string(),
        "muscle_define".to_string(),
        "shoulders_broad".to_string(),
        "shoulders_narrow".to_string(),
        "torso_broad".to_string(),
        "torso_narrow".to_string(),
        "hips_broad".to_string(),
        "hips_narrow".to_string(),
    ];
    let consumed = vec![
        AppearanceParamId::new("build"),
        AppearanceParamId::new("fat"),
        AppearanceParamId::new("muscle"),
        AppearanceParamId::new("shoulders"),
        AppearanceParamId::new("torso"),
        AppearanceParamId::new("hips"),
    ];
    let weights = resolve_equipment_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &body_targets,
        &consumed,
    )
    .unwrap();
    assert!(weights[4] > 0.9);
    assert!(weights[5] < 0.1);
    assert!(weights[6] < 0.1);
    assert!(weights[8] < 0.1);
}

#[test]
fn equipment_arms_resolve_ignores_torso_regional_params() {
    let profile = full_human_profile();
    let mut values = default_semantic_values(&profile);
    values.insert(AppearanceParamId::new("torso"), 1.0);
    values.insert(AppearanceParamId::new("arms"), 1.0);
    let arm_targets = vec![
        "build_broad".to_string(),
        "build_narrow".to_string(),
        "fat_soft".to_string(),
        "muscle_define".to_string(),
        "arms_thick".to_string(),
        "arms_thin".to_string(),
    ];
    let consumed = vec![
        AppearanceParamId::new("build"),
        AppearanceParamId::new("fat"),
        AppearanceParamId::new("muscle"),
        AppearanceParamId::new("arms"),
    ];
    let weights = resolve_equipment_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &arm_targets,
        &consumed,
    )
    .unwrap();
    assert!(weights[4] > 0.9);
    assert!(weights[5] < 0.1);
}

#[test]
fn equipment_shoulders_and_muscle_compose_on_body_armor() {
    let profile = full_human_profile();
    let mut values = default_semantic_values(&profile);
    values.insert(AppearanceParamId::new("shoulders"), 1.0);
    values.insert(AppearanceParamId::new("muscle"), 1.0);
    let body_targets = vec![
        "build_broad".to_string(),
        "build_narrow".to_string(),
        "fat_soft".to_string(),
        "muscle_define".to_string(),
        "shoulders_broad".to_string(),
        "shoulders_narrow".to_string(),
        "torso_broad".to_string(),
        "torso_narrow".to_string(),
        "hips_broad".to_string(),
        "hips_narrow".to_string(),
    ];
    let consumed = vec![
        AppearanceParamId::new("build"),
        AppearanceParamId::new("fat"),
        AppearanceParamId::new("muscle"),
        AppearanceParamId::new("shoulders"),
        AppearanceParamId::new("torso"),
        AppearanceParamId::new("hips"),
    ];
    let weights = resolve_equipment_morph_weights(
        &profile,
        &BodyVariantId::new("human_male"),
        &values,
        &body_targets,
        &consumed,
    )
    .unwrap();
    assert!(weights[4] > 0.9);
    assert!(weights[3] > 0.9);
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
