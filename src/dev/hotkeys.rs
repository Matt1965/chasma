//! Dev-mode hotkey inventory and shared input helpers (Slice 6, Phase 8A).
//!
//! The registry documents bindings; handlers remain in domain modules.
//! Authoritative behavior is always the handler code — not this table.

use crate::dev::dev_mode::DevModeState;
use crate::dev::inspector::{
    BlueprintInspectionState, BlueprintPendingConfirmation, WorldInspectorState,
    exit_blueprint_edit_to_inspect, exit_blueprint_inspection,
};
use crate::dev::selected_object::SelectedObjectUiState;
use crate::world::{BuildingTransformEditOptions, DoodadTransformEditOptions};

use super::gizmo::{
    GizmoCoordinateSpace, dev_gizmo_building_commit_options, dev_gizmo_doodad_commit_options,
};

/// Gizmo axes are permanently world-aligned (Slice 6).
pub const DEV_GIZMO_COORDINATE_SPACE: GizmoCoordinateSpace = GizmoCoordinateSpace::World;

/// Whether global dev shortcuts must not run this frame.
#[derive(Debug, Clone, Copy)]
pub struct DevShortcutSuppressionCtx<'a> {
    pub dev_state: &'a DevModeState,
    pub selected_object_ui: &'a SelectedObjectUiState,
    pub blueprint_inspection: &'a BlueprintInspectionState,
}

impl<'a> DevShortcutSuppressionCtx<'a> {
    pub fn new(
        dev_state: &'a DevModeState,
        selected_object_ui: &'a SelectedObjectUiState,
        blueprint_inspection: &'a BlueprintInspectionState,
    ) -> Self {
        Self {
            dev_state,
            selected_object_ui,
            blueprint_inspection,
        }
    }
}

pub fn dev_shortcuts_suppressed(ctx: DevShortcutSuppressionCtx<'_>) -> bool {
    if ctx.dev_state.has_text_focus() {
        return true;
    }
    if ctx.selected_object_ui.pending_delete.is_some() {
        return true;
    }
    if ctx.blueprint_inspection.pending_confirmation.is_some() {
        return true;
    }
    if ctx.blueprint_inspection.variant_draft.is_some() {
        return true;
    }
    false
}

/// Dev transform commit options — overlap always allowed; no follow-ground on moved objects.
pub fn dev_doodad_transform_edit_options() -> DoodadTransformEditOptions {
    dev_gizmo_doodad_commit_options()
}

pub fn dev_building_transform_edit_options() -> BuildingTransformEditOptions {
    dev_gizmo_building_commit_options()
}

/// Lifecycle of a documented dev shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevShortcutLifecycle {
    /// Stable global or context-local binding.
    Retained,
    /// Removed — listed for audit only.
    Removed,
}

/// Static metadata for discoverability and tests.
#[derive(Debug, Clone, Copy)]
pub struct DevHotkeyEntry {
    pub key_label: &'static str,
    pub action: &'static str,
    pub context: &'static str,
    pub lifecycle: DevShortcutLifecycle,
    pub migration_slice: Option<&'static str>,
    pub ui_replacement: Option<&'static str>,
    pub source_module: &'static str,
}

/// Code-level dev hotkey inventory (not a runtime input router).
pub const DEV_HOTKEY_REGISTRY: &[DevHotkeyEntry] = &[
    DevHotkeyEntry {
        key_label: "F12",
        action: "Toggle dev mode",
        context: "Global",
        lifecycle: DevShortcutLifecycle::Retained,
        migration_slice: None,
        ui_replacement: None,
        source_module: "dev/input.rs",
    },
    DevHotkeyEntry {
        key_label: "Ctrl+F",
        action: "Focus catalog search or scene name",
        context: "Dev mode enabled",
        lifecycle: DevShortcutLifecycle::Retained,
        migration_slice: None,
        ui_replacement: Some("Click search field"),
        source_module: "dev/input.rs",
    },
    DevHotkeyEntry {
        key_label: "Tab",
        action: "Cycle catalog tabs",
        context: "Dev mode, no text focus",
        lifecycle: DevShortcutLifecycle::Retained,
        migration_slice: None,
        ui_replacement: Some("Tab buttons"),
        source_module: "dev/input.rs",
    },
    DevHotkeyEntry {
        key_label: ",",
        action: "Translate gizmo",
        context: "Doodad/building selected, shortcuts not suppressed",
        lifecycle: DevShortcutLifecycle::Retained,
        migration_slice: None,
        ui_replacement: Some("Selected Object Move button"),
        source_module: "dev/gizmo/input.rs",
    },
    DevHotkeyEntry {
        key_label: ".",
        action: "Rotate gizmo",
        context: "Doodad/building selected, shortcuts not suppressed",
        lifecycle: DevShortcutLifecycle::Retained,
        migration_slice: None,
        ui_replacement: Some("Selected Object Rotate button"),
        source_module: "dev/gizmo/input.rs",
    },
    DevHotkeyEntry {
        key_label: "/",
        action: "Scale gizmo only",
        context: "Doodad/building selected, shortcuts not suppressed",
        lifecycle: DevShortcutLifecycle::Retained,
        migration_slice: None,
        ui_replacement: Some("Selected Object Scale button"),
        source_module: "dev/gizmo/input.rs",
    },
    DevHotkeyEntry {
        key_label: "Esc",
        action: "(removed from dev — reserved for pause menu)",
        context: "N/A",
        lifecycle: DevShortcutLifecycle::Removed,
        migration_slice: None,
        ui_replacement: Some("Visible cancel buttons; right-click policy"),
        source_module: "dev/hotkeys.rs",
    },
    DevHotkeyEntry {
        key_label: "L",
        action: "(removed) gizmo world/local toggle",
        context: "N/A",
        lifecycle: DevShortcutLifecycle::Removed,
        migration_slice: None,
        ui_replacement: Some("Gizmos permanently world-aligned"),
        source_module: "dev/hotkeys.rs",
    },
    DevHotkeyEntry {
        key_label: "O (hold)",
        action: "(removed) dev overlap modifier",
        context: "N/A",
        lifecycle: DevShortcutLifecycle::Removed,
        migration_slice: None,
        ui_replacement: Some("Dev overlap always allowed"),
        source_module: "dev/hotkeys.rs",
    },
    DevHotkeyEntry {
        key_label: "G (hold)",
        action: "(removed) transform follow-ground modifier",
        context: "N/A",
        lifecycle: DevShortcutLifecycle::Removed,
        migration_slice: None,
        ui_replacement: Some("Initial placement terrain snap only"),
        source_module: "dev/hotkeys.rs",
    },
    DevHotkeyEntry {
        key_label: "Right-click",
        action: "Dev cancellation precedence (placement, transform, blueprint pending, selection clear)",
        context: "Dev mode, world pointer",
        lifecycle: DevShortcutLifecycle::Retained,
        migration_slice: None,
        ui_replacement: Some("Cancel Placement button; transform tool buttons"),
        source_module: "dev/hotkeys.rs",
    },
    DevHotkeyEntry {
        key_label: "N",
        action: "(removed) enter blueprint inspection",
        context: "N/A",
        lifecycle: DevShortcutLifecycle::Removed,
        migration_slice: Some("Slice 7"),
        ui_replacement: Some("Navigation Editor → Inspect / Open Navigation Editor"),
        source_module: "dev/navigation_editor/",
    },
    DevHotkeyEntry {
        key_label: "E",
        action: "Toggle catalog enabled-only filter (when no transform target)",
        context: "Dev mode, shortcuts not suppressed",
        lifecycle: DevShortcutLifecycle::Retained,
        migration_slice: None,
        ui_replacement: Some("Catalog filter; Navigation Editor → Edit"),
        source_module: "dev/input.rs",
    },
    DevHotkeyEntry {
        key_label: "[ / ]",
        action: "(removed) blueprint floor navigation",
        context: "N/A",
        lifecycle: DevShortcutLifecycle::Removed,
        migration_slice: Some("Slice 7"),
        ui_replacement: Some("Navigation Editor floor buttons"),
        source_module: "dev/navigation_editor/",
    },
    DevHotkeyEntry {
        key_label: "1–3",
        action: "(removed) blueprint edit tools",
        context: "N/A",
        lifecycle: DevShortcutLifecycle::Removed,
        migration_slice: Some("Slice 7"),
        ui_replacement: Some("Navigation Editor tool buttons"),
        source_module: "dev/navigation_editor/",
    },
    DevHotkeyEntry {
        key_label: "Ctrl+S / Ctrl+Shift+S / Ctrl+Alt+R / Ctrl+Shift+V",
        action: "(removed) blueprint persistence shortcuts",
        context: "N/A",
        lifecycle: DevShortcutLifecycle::Removed,
        migration_slice: Some("Slice 7"),
        ui_replacement: Some("Navigation Editor persistence buttons"),
        source_module: "dev/navigation_editor/",
    },
    DevHotkeyEntry {
        key_label: "1–9",
        action: "Assign favorite slot (Shift) or quick-select definition",
        context: "Dev mode, no text focus, Catalog active",
        lifecycle: DevShortcutLifecycle::Retained,
        migration_slice: None,
        ui_replacement: Some("Catalog favorite star and slot controls"),
        source_module: "dev/input.rs",
    },
    DevHotkeyEntry {
        key_label: "X / Y / Z",
        action: "Lock gizmo axis during active transform drag",
        context: "Transform gizmo engaged, no text focus",
        lifecycle: DevShortcutLifecycle::Retained,
        migration_slice: None,
        ui_replacement: Some("Gizmo axis handles"),
        source_module: "dev/gizmo/input.rs",
    },
    DevHotkeyEntry {
        key_label: "Delete",
        action: "Delete selected blueprint element (Navigation Editor edit mode only)",
        context: "Navigation Editor visible, editing, no text focus",
        lifecycle: DevShortcutLifecycle::Retained,
        migration_slice: Some("Slice 7"),
        ui_replacement: Some("Navigation Editor Delete button"),
        source_module: "dev/inspector/blueprint_edit.rs",
    },
];

/// Cancel blueprint modal / draft state (visible buttons and right-click).
pub fn cancel_blueprint_pending_confirmation(
    inspection: &mut BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
) {
    if inspection.pending_confirmation.take().is_some() {
        inspector.last_message = "Cancelled pending blueprint action".into();
    }
}

pub fn cancel_blueprint_variant_draft(
    inspection: &mut BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
) {
    if inspection.variant_draft.take().is_some() {
        inspector.last_message = "Cancelled Save As Variant".into();
    }
}

pub fn cancel_blueprint_edit_drag(inspection: &mut BlueprintInspectionState) {
    inspection.drag = None;
}

/// Request exit from blueprint edit; prompts discard when dirty.
pub fn request_exit_blueprint_edit(
    inspection: &mut BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
) {
    if inspection.dirty {
        inspection.pending_confirmation = Some(BlueprintPendingConfirmation::DiscardEdits {
            action: "exit edit".into(),
        });
        inspector.last_message =
            "Unsaved blueprint edits — confirm discard with the Exit Edit button or Enter".into();
        return;
    }
    exit_blueprint_edit_to_inspect(inspection);
    if let Some(snap) = inspector.blueprint_snapshot.as_mut() {
        snap.edit_active = false;
        snap.edit_dirty = false;
    }
    inspector.last_message = "Exited blueprint edit".into();
}

/// Exit blueprint inspection (visible Selected Object control).
pub fn exit_blueprint_inspection_from_ui(
    inspection: &mut BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
    overlay_focus: &mut crate::debug::InspectorOverlayFocus,
    camera: &mut crate::camera::RtsCameraState,
) {
    exit_blueprint_inspection(inspection, overlay_focus, camera);
    inspector.last_message = "Exited blueprint inspection".into();
    if let Some(snap) = inspector.blueprint_snapshot.as_mut() {
        snap.inspection_active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::dev_mode::DevTextFieldFocus;
    use crate::dev::gizmo::DevTool;

    fn suppression_ctx<'a>(
        dev_state: &'a DevModeState,
        ui: &'a SelectedObjectUiState,
        blueprint: &'a BlueprintInspectionState,
    ) -> DevShortcutSuppressionCtx<'a> {
        DevShortcutSuppressionCtx::new(dev_state, ui, blueprint)
    }

    #[test]
    fn catalog_search_focus_suppresses_shortcuts() {
        let mut dev_state = DevModeState::default();
        dev_state.focus_catalog_search();
        let ui = SelectedObjectUiState::default();
        let blueprint = BlueprintInspectionState::default();
        assert!(dev_shortcuts_suppressed(suppression_ctx(
            &dev_state, &ui, &blueprint
        )));
    }

    #[test]
    fn scene_name_focus_suppresses_shortcuts() {
        let mut dev_state = DevModeState::default();
        dev_state.focus_scene_name();
        let ui = SelectedObjectUiState::default();
        let blueprint = BlueprintInspectionState::default();
        assert!(dev_shortcuts_suppressed(suppression_ctx(
            &dev_state, &ui, &blueprint
        )));
    }

    #[test]
    fn item_quantity_focus_suppresses_shortcuts() {
        let mut dev_state = DevModeState::default();
        dev_state.text_focus = DevTextFieldFocus::ItemQuantity;
        let ui = SelectedObjectUiState::default();
        let blueprint = BlueprintInspectionState::default();
        assert!(dev_shortcuts_suppressed(suppression_ctx(
            &dev_state, &ui, &blueprint
        )));
    }

    #[test]
    fn pending_delete_suppresses_shortcuts() {
        let dev_state = DevModeState::default();
        let mut ui = SelectedObjectUiState::default();
        ui.request_delete(crate::dev::gizmo::SelectedWorldObject::Doodad(
            crate::world::DoodadId::new(1),
        ));
        let blueprint = BlueprintInspectionState::default();
        assert!(dev_shortcuts_suppressed(suppression_ctx(
            &dev_state, &ui, &blueprint
        )));
    }

    #[test]
    fn blueprint_pending_confirmation_suppresses_shortcuts() {
        let dev_state = DevModeState::default();
        let ui = SelectedObjectUiState::default();
        let mut blueprint = BlueprintInspectionState::default();
        blueprint.pending_confirmation = Some(BlueprintPendingConfirmation::ResetToAsset);
        assert!(dev_shortcuts_suppressed(suppression_ctx(
            &dev_state, &ui, &blueprint
        )));
    }

    #[test]
    fn clearing_focus_restores_shortcuts() {
        let mut dev_state = DevModeState::default();
        dev_state.focus_catalog_search();
        dev_state.clear_text_focus();
        let ui = SelectedObjectUiState::default();
        let blueprint = BlueprintInspectionState::default();
        assert!(!dev_shortcuts_suppressed(suppression_ctx(
            &dev_state, &ui, &blueprint
        )));
    }

    #[test]
    fn dev_transform_options_allow_overlap_no_follow_ground() {
        let doodad = dev_doodad_transform_edit_options();
        assert!(doodad.allow_overlap);
        assert!(!doodad.follow_ground);
        let building = dev_building_transform_edit_options();
        assert!(building.allow_overlap);
        assert!(!building.follow_ground);
    }

    #[test]
    fn registry_lists_global_transform_keys() {
        let labels: Vec<_> = DEV_HOTKEY_REGISTRY.iter().map(|e| e.key_label).collect();
        assert!(labels.contains(&","));
        assert!(labels.contains(&"/"));
    }

    #[test]
    fn removed_bindings_marked_removed_in_registry() {
        for entry in DEV_HOTKEY_REGISTRY {
            if entry.key_label == "Esc" || entry.key_label == "L" {
                assert_eq!(entry.lifecycle, DevShortcutLifecycle::Removed);
            }
        }
    }

    #[test]
    fn slash_registry_entry_is_scale_only() {
        let slash = DEV_HOTKEY_REGISTRY
            .iter()
            .find(|e| e.key_label == "/")
            .expect("slash entry");
        assert!(slash.action.contains("Scale"));
        assert_eq!(slash.lifecycle, DevShortcutLifecycle::Retained);
    }

    #[test]
    fn transform_tool_activation_path_shared() {
        // Documented invariant: buttons call activate_dev_transform_tool; keys call enter_transform_tool.
        assert!(DevTool::Translate.is_transform());
        assert!(DevTool::Scale.is_transform());
    }
}
