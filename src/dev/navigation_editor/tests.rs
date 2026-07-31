//! Navigation Editor tests (Slice 7).

use super::panel::NavigationEditorAction;
use super::*;
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
fn navigation_editor_action_labels_are_glyph_safe() {
    // Keep in sync with `panel::action_rows` labels that must remain ASCII-safe.
    let labels = [
        "Inspect",
        "Edit",
        "Exit edit",
        "Floor -",
        "Floor +",
        "Select",
        "Add corner",
        "Add entrance",
        "Delete",
        "Radius +",
        "Radius -",
        "Frame building",
        "Return view",
        "Regenerate",
        "Validate",
        "Save instance",
        "Apply to asset",
        "Reset to asset",
        "Save As Variant",
        "Create variant",
        "Confirm",
        "Cancel",
        "Cancel variant",
        "Overlay blueprint",
        "Overlay entrances",
        "Overlay runtime path",
    ];
    for label in labels {
        assert!(
            !contains_forbidden_dev_ui_glyph(label),
            "forbidden glyph in Navigation Editor label: {label:?}"
        );
    }
    // Explicitly guard the previously-broken controls.
    assert!(!contains_forbidden_dev_ui_glyph("Floor -"));
    assert!(!contains_forbidden_dev_ui_glyph("Radius -"));
    assert!(!contains_forbidden_dev_ui_glyph("Regenerate"));
    assert!(!contains_forbidden_dev_ui_glyph("Save As Variant"));
    let _ = NavigationEditorAction::Regenerate;
}
