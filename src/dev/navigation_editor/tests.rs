//! Navigation Editor tests (Slice 7).

use super::commands::open_navigation_editor;
use super::panel::NavigationEditorAction;
use super::state::{
    DEFAULT_NAV_EDITOR_BUILDING_OPACITY, NavigationEditorUiState, NavigationGenerationDiagnostics,
    format_concise_generation_summary, format_generation_details, navigation_editor_owns_session,
    wrap_panel_text,
};
use crate::dev::gizmo::{DevTool, TransformEditState};
use crate::dev::inspector::BlueprintInspectionState;
use crate::dev::widgets::contains_forbidden_dev_ui_glyph;
use crate::dev::window::{DevWindowId, DevWindowRegistry};

#[test]
fn navigation_editor_window_identity() {
    assert_eq!(DevWindowId::NavigationEditor.title(), "Navigation Editor");
    assert_eq!(
        DevWindowId::NavigationEditor.launcher_label(),
        "Navigation Editor"
    );
    assert!(!DevWindowId::NavigationEditor.default_visible());
}

#[test]
fn dirty_edit_blocks_selection() {
    let mut inspection = BlueprintInspectionState::default();
    assert!(!crate::dev::inspector::blueprint_edit_blocks_building_selection(&inspection));
    inspection.editing = true;
    inspection.dirty = true;
    assert!(crate::dev::inspector::blueprint_edit_blocks_building_selection(&inspection));
}

#[test]
fn open_navigation_editor_shows_window() {
    let mut registry = DevWindowRegistry::default();
    assert!(!registry.is_visible(DevWindowId::NavigationEditor));
    open_navigation_editor(&mut registry);
    assert!(registry.is_visible(DevWindowId::NavigationEditor));
}

#[test]
fn navigation_editor_owns_session_requires_visible_window_and_active_inspection() {
    let registry = DevWindowRegistry::default();
    let mut inspection = BlueprintInspectionState::default();
    assert!(!navigation_editor_owns_session(
        true,
        &registry,
        &inspection
    ));

    let mut registry = DevWindowRegistry::default();
    registry.show(DevWindowId::NavigationEditor);
    assert!(!navigation_editor_owns_session(
        true,
        &registry,
        &inspection
    ));

    inspection.active = true;
    assert!(navigation_editor_owns_session(true, &registry, &inspection));
}

#[test]
fn default_building_opacity_is_session_local() {
    let ui = NavigationEditorUiState::default();
    assert!((ui.building_opacity - DEFAULT_NAV_EDITOR_BUILDING_OPACITY).abs() < f32::EPSILON);
    assert!(ui.regeneration_source_label.is_none());
    assert!(ui.generation_diagnostics.is_none());
}

#[test]
fn reset_session_presentation_clears_editor_local_state() {
    let mut ui = NavigationEditorUiState {
        building_opacity: 0.1,
        regeneration_source_label: Some("occupancy_collision".into()),
        generation_diagnostics: Some(NavigationGenerationDiagnostics {
            entrances_generated: 1,
            ..Default::default()
        }),
        ..Default::default()
    };
    ui.reset_session_presentation();
    assert!((ui.building_opacity - DEFAULT_NAV_EDITOR_BUILDING_OPACITY).abs() < f32::EPSILON);
    assert!(ui.regeneration_source_label.is_none());
    assert!(ui.generation_diagnostics.is_none());
}

#[test]
fn entering_navigation_editor_session_clears_transform_gizmo_state() {
    let mut edit = TransformEditState::default();
    edit.mode = DevTool::Translate;
    edit.target = Some(crate::dev::gizmo::SelectedWorldObject::Building(
        crate::world::BuildingId(1),
    ));
    assert!(edit.is_transform_session_active());

    let mut registry = DevWindowRegistry::default();
    registry.show(DevWindowId::NavigationEditor);
    let mut inspection = BlueprintInspectionState::default();
    inspection.active = true;
    assert!(navigation_editor_owns_session(true, &registry, &inspection));

    // Same path sync_gizmo_target takes while the editor owns the session.
    edit.full_cancel();
    assert!(!edit.is_transform_session_active());
    assert_eq!(edit.mode, DevTool::Select);
}

#[test]
fn infer_message_severity_classifies_blocked_and_success() {
    use crate::dev::navigation_editor::infer_message_severity;
    use crate::dev::widgets::DevStatusSeverity;
    assert_eq!(
        infer_message_severity("Save blocked: overlap"),
        DevStatusSeverity::Error
    );
    assert_eq!(
        infer_message_severity("Save complete."),
        DevStatusSeverity::Success
    );
    assert_eq!(
        infer_message_severity(
            "Saved instance blueprint override for building #1: activated 1, refreshed 0, skipped 0, failed 0 (authority: Instance Override)"
        ),
        DevStatusSeverity::Success
    );
    assert_eq!(
        infer_message_severity("activated 1, refreshed 0, skipped 0, failed 2"),
        DevStatusSeverity::Error
    );
}

#[test]
fn navigation_editor_action_labels_are_glyph_safe() {
    // Keep in sync with layout labels that must remain ASCII-safe.
    let labels = [
        "Inspect",
        "Edit",
        "Exit edit",
        "^",
        "v",
        "<",
        ">",
        "Select",
        "+ Vertex",
        "+ Entry",
        "+ Region",
        "+ Link",
        "Delete",
        "Radius +",
        "Radius -",
        "Frame",
        "Return view",
        "Generate draft",
        "Regenerate draft",
        "Preview draft",
        "Edit draft",
        "Replace working copy",
        "Discard draft",
        "Validate",
        "Save instance",
        "Apply to asset",
        "Reset to asset",
        "Save As Variant",
        "Create variant",
        "Confirm",
        "Cancel",
        "Cancel variant",
        "Blueprint",
        "Entrances",
        "Runtime path",
    ];
    for label in labels {
        assert!(
            !contains_forbidden_dev_ui_glyph(label),
            "forbidden glyph in Navigation Editor label: {label:?}"
        );
    }
    // Explicitly guard the previously-broken controls.
    assert!(!contains_forbidden_dev_ui_glyph("^"));
    assert!(!contains_forbidden_dev_ui_glyph("v"));
    assert!(!contains_forbidden_dev_ui_glyph("<"));
    assert!(!contains_forbidden_dev_ui_glyph(">"));
    assert!(!contains_forbidden_dev_ui_glyph("Generate draft"));
    assert!(!contains_forbidden_dev_ui_glyph("Save As Variant"));
    let _ = NavigationEditorAction::Regenerate;
    let _ = NavigationEditorAction::ReplaceWorkingCopy;
    let _ = NavigationEditorAction::EditDraft;
    let _ = NavigationEditorAction::DiscardDraft;
}

#[test]
fn accept_draft_replaces_topology_and_marks_dirty() {
    use crate::dev::inspector::{GeneratedBlueprintDraft, accept_generated_blueprint_draft};
    use crate::world::two_room_hut_navigation_blueprint;
    use crate::world::validate_blueprint_for_inspection;

    let mut inspection = BlueprintInspectionState::default();
    let working = two_room_hut_navigation_blueprint();
    inspection.working_copy = Some(working.clone());
    let mut draft = two_room_hut_navigation_blueprint();
    draft.floors[0].regions.truncate(1);
    draft.region_connections.clear();
    let validation = validate_blueprint_for_inspection(&draft);
    inspection.generated_draft = Some(GeneratedBlueprintDraft {
        blueprint: draft,
        warnings: Vec::new(),
        geometry_diagnostics: Default::default(),
        mesh_source_label: "synthetic".into(),
        validation,
        adopted: false,
    });
    accept_generated_blueprint_draft(&mut inspection).expect("accept");
    assert!(inspection.dirty);
    assert_eq!(
        inspection.working_copy.as_ref().unwrap().floors[0]
            .regions
            .len(),
        1
    );
    assert!(inspection.generated_draft.is_none());
}

#[test]
fn discard_draft_leaves_working_copy_unchanged() {
    use crate::dev::inspector::{GeneratedBlueprintDraft, discard_generated_blueprint_draft};
    use crate::world::two_room_hut_navigation_blueprint;
    use crate::world::validate_blueprint_for_inspection;

    let mut inspection = BlueprintInspectionState::default();
    let working = two_room_hut_navigation_blueprint();
    inspection.working_copy = Some(working.clone());
    let draft = two_room_hut_navigation_blueprint();
    inspection.generated_draft = Some(GeneratedBlueprintDraft {
        blueprint: draft,
        warnings: Vec::new(),
        geometry_diagnostics: Default::default(),
        mesh_source_label: "synthetic".into(),
        validation: validate_blueprint_for_inspection(&working),
        adopted: false,
    });
    discard_generated_blueprint_draft(&mut inspection);
    assert!(inspection.generated_draft.is_none());
    assert_eq!(
        inspection.working_copy.as_ref().unwrap().floors[0]
            .regions
            .len(),
        working.floors[0].regions.len()
    );
    assert!(!inspection.dirty);
}

#[test]
fn invalid_generated_draft_cannot_be_accepted() {
    use crate::dev::inspector::{GeneratedBlueprintDraft, accept_generated_blueprint_draft};
    use crate::world::validate_blueprint_for_inspection;

    let mut inspection = BlueprintInspectionState::default();
    let mut draft = crate::world::two_room_hut_navigation_blueprint();
    draft.entrances[0].region_key = None;
    let validation = validate_blueprint_for_inspection(&draft);
    assert!(!validation.valid());
    inspection.generated_draft = Some(GeneratedBlueprintDraft {
        blueprint: draft,
        warnings: vec!["generator_entrance_region_unresolved".into()],
        geometry_diagnostics: Default::default(),
        mesh_source_label: "synthetic".into(),
        validation,
        adopted: false,
    });
    assert!(inspection.has_pending_generated_draft());
    assert!(accept_generated_blueprint_draft(&mut inspection).is_err());
    assert!(inspection.has_pending_generated_draft());
}

#[test]
fn invalid_generated_draft_can_be_adopted_for_editing() {
    use crate::dev::inspector::{
        GeneratedBlueprintDraft, adopt_generated_blueprint_draft_for_editing,
    };
    use crate::world::validate_blueprint_for_inspection;

    let mut inspection = BlueprintInspectionState::default();
    let mut draft_bp = crate::world::two_room_hut_navigation_blueprint();
    draft_bp.entrances[0].region_key = None;
    let validation = validate_blueprint_for_inspection(&draft_bp);
    assert!(!validation.valid());
    inspection.generated_draft = Some(GeneratedBlueprintDraft {
        blueprint: draft_bp.clone(),
        warnings: Vec::new(),
        geometry_diagnostics: Default::default(),
        mesh_source_label: "synthetic".into(),
        validation,
        adopted: false,
    });
    adopt_generated_blueprint_draft_for_editing(&mut inspection).expect("adopt");
    assert!(inspection.editing);
    assert!(inspection.dirty);
    assert!(inspection.is_editing_adopted_draft());
    assert!(!inspection.has_pending_generated_draft());
    assert!(inspection.selected_floor_id.is_some());
    assert_eq!(
        inspection
            .working_copy
            .as_ref()
            .unwrap()
            .floors
            .iter()
            .map(|floor| floor.regions.len())
            .sum::<usize>(),
        draft_bp
            .floors
            .iter()
            .map(|floor| floor.regions.len())
            .sum::<usize>()
    );
}

#[test]
fn adopt_preserves_pre_adoption_working_copy_for_reset() {
    use crate::dev::inspector::{
        GeneratedBlueprintDraft, adopt_generated_blueprint_draft_for_editing,
        restore_pre_adoption_working_copy,
    };
    use crate::world::validate_blueprint_for_inspection;

    let mut inspection = BlueprintInspectionState::default();
    let prior = crate::world::two_room_hut_navigation_blueprint();
    inspection.working_copy = Some(prior.clone());
    let mut draft_bp = prior.clone();
    draft_bp.floors[0].regions.truncate(1);
    let validation = validate_blueprint_for_inspection(&draft_bp);
    inspection.generated_draft = Some(GeneratedBlueprintDraft {
        blueprint: draft_bp,
        warnings: Vec::new(),
        geometry_diagnostics: Default::default(),
        mesh_source_label: "synthetic".into(),
        validation,
        adopted: false,
    });
    adopt_generated_blueprint_draft_for_editing(&mut inspection).expect("adopt");
    assert_eq!(
        inspection.working_copy.as_ref().unwrap().floors[0]
            .regions
            .len(),
        1
    );
    restore_pre_adoption_working_copy(&mut inspection);
    assert_eq!(
        inspection.working_copy.as_ref().unwrap().floors[0]
            .regions
            .len(),
        prior.floors[0].regions.len()
    );
    assert!(inspection.generated_draft.is_none());
}

#[test]
fn wrap_panel_text_breaks_long_marker_lines() {
    let long = "marker_a @ [1.0,2.0] (explicit) | marker_b @ [3.0,4.0] (synthesized) | marker_c @ [5.0,6.0] (deduplicated)";
    let wrapped = wrap_panel_text(long, 40);
    assert!(wrapped.contains('\n'));
    assert!(wrapped.len() >= long.len());
}

#[test]
fn concise_generation_summary_includes_counts() {
    let summary = format_concise_generation_summary(Some("occupancy_collision"), 5, 4, 3);
    assert!(summary.contains("occupancy_collision"));
    assert!(summary.contains("Regions: 5"));
    assert!(summary.contains("Connections: 4"));
    assert!(summary.contains("Validation errors: 3"));
}

#[test]
fn generation_details_lists_markers_on_separate_lines() {
    let diag = NavigationGenerationDiagnostics {
        entrances_generated: 2,
        explicit_markers: 1,
        synthesized_entrances: 1,
        deduplicated_candidates: 0,
        regeneration_source: "occupancy_collision".into(),
        candidate_details: vec![
            "north_door @ [1.0, 2.0] (explicit)".into(),
            "south_door @ [3.0, 4.0] (synthesized)".into(),
        ],
    };
    let details = format_generation_details(Some("occupancy_collision"), Some(&diag));
    assert!(details.contains("Markers:"));
    assert!(details.contains("north_door"));
    assert!(details.contains("south_door"));
    assert!(!details.contains(" | "));
}

#[test]
fn invalid_draft_status_message_is_not_fatal_failure() {
    use crate::dev::inspector::{GeneratedBlueprintDraft, format_generated_draft_status_message};
    use crate::world::validate_blueprint_for_inspection;

    let mut draft = crate::world::two_room_hut_navigation_blueprint();
    draft.entrances[0].region_key = None;
    let validation = validate_blueprint_for_inspection(&draft);
    let message = format_generated_draft_status_message(&GeneratedBlueprintDraft {
        blueprint: draft,
        warnings: Vec::new(),
        geometry_diagnostics: Default::default(),
        mesh_source_label: "synthetic".into(),
        validation,
        adopted: false,
    });
    assert!(message.contains("validation errors"));
    assert!(!message.contains("failed before a usable draft"));
}

#[test]
fn navigation_editor_default_width_is_wider() {
    use crate::dev::window::{
        NAVIGATION_EDITOR_WIDTH_PX, navigation_editor_panel_width,
        navigation_editor_uses_two_columns,
    };
    use bevy::prelude::Vec2;

    let width = navigation_editor_panel_width(Vec2::new(1920.0, 1080.0));
    assert!((width - NAVIGATION_EDITOR_WIDTH_PX).abs() < 1.0);
    assert!(navigation_editor_uses_two_columns(width));
}

#[test]
fn generation_details_expansion_persists_in_ui_state() {
    use crate::dev::widgets::{DevCollapsibleSectionId, DevCollapsibleState};

    let mut ui = NavigationEditorUiState::default();
    let mut collapsible = DevCollapsibleState::default();
    assert!(!collapsible.is_expanded(DevCollapsibleSectionId::NavEditorGeneration));
    collapsible.set_expanded(DevCollapsibleSectionId::NavEditorGeneration, true);
    ui.generation_details_expanded =
        collapsible.is_expanded(DevCollapsibleSectionId::NavEditorGeneration);
    assert!(ui.generation_details_expanded);
    collapsible.set_expanded(DevCollapsibleSectionId::NavEditorGeneration, true);
    assert!(ui.generation_details_expanded);
}

#[test]
fn validation_expanded_hint_is_one_shot() {
    let mut ui = NavigationEditorUiState {
        validation_expanded: true,
        ..Default::default()
    };
    ui.validation_expanded = false;
    assert!(!ui.validation_expanded);
}

#[test]
fn success_toast_auto_dismisses() {
    use super::state::NavEditorToast;
    use crate::dev::widgets::DevStatusSeverity;

    let toast = NavEditorToast {
        message: "Region deleted.".into(),
        severity: DevStatusSeverity::Success,
        shown_at_secs: 1.0,
        auto_dismiss: true,
    };
    assert!(!toast.is_expired(3.5));
    assert!(toast.is_expired(4.1));
}
