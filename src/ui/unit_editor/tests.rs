//! Unit Editor focused tests (CG3).

use std::collections::BTreeMap;

use super::controls::appearance_control_specs;
use super::draft::UnitAppearanceDraft;
use super::preview_spawn::appearance_requires_preview_respawn;
use super::session::{UnitEditorMode, UnitEditorSession};
use crate::menu::{AppScreen, MenuNavigation, menu_blocks_input};
use crate::world::{
    AppearanceParamId, AppearanceParameterDefinition, AppearanceProfile,
    AppearanceProfileId, BodyVariantDefinition, BodyVariantId, SpeciesId, UnitAppearance,
    UnitDefinitionId, UnitId, UnitRenderKey,
};

fn test_profile() -> AppearanceProfile {
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
        parameters: vec![
            AppearanceParameterDefinition {
                id: AppearanceParamId::new("build"),
                display_name: "Build".into(),
                category: "Body".into(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
                display_order: 2,
                enabled: true,
            },
            AppearanceParameterDefinition {
                id: AppearanceParamId::new("fat"),
                display_name: "Fat".into(),
                category: "Body".into(),
                min: 0.0,
                max: 1.0,
                default: 0.35,
                display_order: 1,
                enabled: true,
            },
        ],
        morph_mappings: Vec::new(),
        enabled: true,
    }
}

fn test_draft() -> UnitAppearanceDraft {
    let profile = test_profile();
    UnitAppearanceDraft::from_live(
        UnitDefinitionId::new("human"),
        Some("Human".into()),
        UnitAppearance {
            profile_id: profile.id.clone(),
            body_variant_id: profile.body_variants[0].id.clone(),
            height_scale: profile.height_scale_default,
            morphs: profile
                .enabled_parameters()
                .iter()
                .map(|parameter| (parameter.id.clone(), parameter.default))
                .collect::<BTreeMap<_, _>>(),
            generation_seed: None,
        },
    )
}

#[test]
fn draft_copy_is_independent_of_live_unit() {
    let mut draft = test_draft();
    draft.appearance.height_scale = 1.2;
    let initial = draft.clone();
    draft.appearance.height_scale = 1.3;
    assert_ne!(draft.appearance.height_scale, initial.appearance.height_scale);
}

#[test]
fn session_tracks_dirty_state() {
    let draft = test_draft();
    let mut session = UnitEditorSession::new(UnitEditorMode::LiveUnit(UnitId::new(1)), draft);
    assert!(!session.dirty);
    session.draft.appearance.height_scale = 1.3;
    session.recompute_dirty();
    assert!(session.dirty);
}

#[test]
fn appearance_controls_follow_profile_order() {
    let specs = appearance_control_specs(&test_profile());
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0].param_id.as_str(), "fat");
    assert_eq!(specs[1].param_id.as_str(), "build");
}

#[test]
fn menu_blocks_while_unit_editor_open() {
    let nav = MenuNavigation::default();
    assert!(menu_blocks_input(&AppScreen::UnitEditor, &nav));
}

#[test]
fn draft_definition_id_is_preserved() {
    let draft = test_draft();
    assert_eq!(draft.definition_id.as_str(), "human");
}

#[test]
fn height_and_morph_changes_do_not_require_preview_respawn() {
    let mut current = test_draft().appearance;
    let mut next = current.clone();
    next.height_scale = 1.1;
    next.morphs
        .insert(AppearanceParamId::new("build"), 0.9);
    assert!(!appearance_requires_preview_respawn(&current, &next));
}

#[test]
fn body_variant_change_requires_preview_respawn() {
    let current = test_draft().appearance;
    let mut next = current.clone();
    next.body_variant_id = BodyVariantId::new("human_female");
    assert!(appearance_requires_preview_respawn(&current, &next));
}
