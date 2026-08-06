//! Navigation Editor command helpers — orchestrates blueprint domain logic (Slice 7).

use bevy::prelude::*;

use crate::camera::{CameraSettings, RtsCameraState};
use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::debug::{DebugOverlayConfig, InspectorOverlayFocus};
use crate::dev::inspector::{
    BlueprintEditTool, BlueprintInspectionState, BlueprintPendingConfirmation,
    BlueprintVariantDraft, BlueprintVariantDraftField, WorldInspectorState,
    capture_building_blueprint_inspection_snapshot, capture_edit_blueprint_snapshot,
    enter_blueprint_edit, enter_blueprint_inspection, exit_blueprint_edit_to_inspect,
    exit_blueprint_inspection, frame_building_for_inspection,
};
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::world::{
    BuildingCatalog, BuildingId, BuildingNavigationBlueprintCatalog,
    BuildingNavigationBlueprintCatalogRevision, WorldData,
};

use super::state::NavigationEditorUiState;

pub fn navigation_editor_visible(registry: &DevWindowRegistry) -> bool {
    registry.is_visible(DevWindowId::NavigationEditor)
}

pub fn open_navigation_editor(registry: &mut DevWindowRegistry) {
    registry.show(DevWindowId::NavigationEditor);
}

pub fn request_close_navigation_editor(
    registry: &mut DevWindowRegistry,
    inspection: &mut BlueprintInspectionState,
    ui: &mut NavigationEditorUiState,
) -> bool {
    if inspection.editing && inspection.dirty {
        ui.pending_blocked_action = Some(super::state::NavigationEditorBlockedAction::CloseWindow);
        inspection.pending_confirmation = Some(BlueprintPendingConfirmation::DiscardEdits {
            action: "close Navigation Editor".into(),
        });
        return false;
    }
    registry.hide(DevWindowId::NavigationEditor);
    ui.reset_session_presentation();
    true
}

pub fn begin_inspection_for_building(
    building_id: BuildingId,
    inspection: &mut BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
    overlay_focus: &mut InspectorOverlayFocus,
    camera: &mut RtsCameraState,
    debug_config: &mut DebugOverlayConfig,
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    camera_settings: &CameraSettings,
    frame_camera: bool,
) -> bool {
    let Some(mut snapshot) = capture_building_blueprint_inspection_snapshot(
        world,
        building_catalog,
        nav_catalog,
        building_id,
        inspection.selected_floor_id,
    ) else {
        inspector.last_message = "No navigation blueprint available for this building".into();
        return false;
    };
    snapshot.inspection_active = true;
    enter_blueprint_inspection(
        building_id,
        inspection,
        overlay_focus,
        camera,
        &snapshot,
        camera_settings.pitch_max,
        camera_settings.distance_min,
        camera_settings.distance_max,
        debug_config,
    );
    if !frame_camera {
        if let Some(saved) = inspection.saved_camera.take() {
            *camera = saved;
            inspection.saved_camera = Some(*camera);
        }
    }
    inspector.blueprint_snapshot = Some(snapshot);
    inspector.last_message = format!("Blueprint inspection: building #{}", building_id.raw());
    true
}

pub fn begin_edit_for_building(
    building_id: BuildingId,
    inspection: &mut BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
    overlay_focus: &mut InspectorOverlayFocus,
    camera: &mut RtsCameraState,
    debug_config: &mut DebugOverlayConfig,
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    camera_settings: &CameraSettings,
) -> bool {
    let Some(snapshot) = capture_edit_blueprint_snapshot(
        world,
        building_catalog,
        nav_catalog,
        building_id,
        inspection.selected_floor_id,
        inspection.working_copy.as_ref(),
    ) else {
        inspector.last_message = "No navigation blueprint available to edit".into();
        return false;
    };
    let Some(working) = snapshot.resolved_blueprint.clone() else {
        inspector.last_message = "No navigation blueprint available to edit".into();
        return false;
    };
    enter_blueprint_edit(
        building_id,
        inspection,
        overlay_focus,
        camera,
        &snapshot,
        camera_settings.pitch_max,
        camera_settings.distance_min,
        camera_settings.distance_max,
        debug_config,
        working,
    );
    inspection.sync_selected_floor_from_working_copy();
    overlay_focus.blueprint_floor_id = inspection.selected_floor_id;
    inspector.blueprint_snapshot = Some(snapshot);
    if let Some(snap) = inspector.blueprint_snapshot.as_mut() {
        snap.edit_active = true;
    }
    inspector.last_message = format!("Blueprint edit: building #{}", building_id.raw());
    true
}

pub fn frame_selected_building(
    inspection: &BlueprintInspectionState,
    inspector: &WorldInspectorState,
    camera: &mut RtsCameraState,
    camera_settings: &CameraSettings,
) {
    let Some(snap) = inspector.blueprint_snapshot.as_ref() else {
        return;
    };
    let half = if snap.world_bounds_radius > 0.0 {
        snap.world_bounds_radius
    } else {
        8.0
    };
    frame_building_for_inspection(
        camera,
        snap.building_center,
        half,
        camera_settings.pitch_max,
        camera_settings.distance_min,
        camera_settings.distance_max,
    );
}

pub fn change_floor(
    inspection: &mut BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
    overlay_focus: &mut InspectorOverlayFocus,
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    building_id: BuildingId,
    direction: FloorStep,
    wrap: bool,
) {
    let Some(snap) = inspector.blueprint_snapshot.clone() else {
        return;
    };
    if snap.floor_ids.is_empty() {
        return;
    }
    let current = inspection
        .selected_floor_id
        .and_then(|id| snap.floor_ids.iter().position(|&f| f == id))
        .unwrap_or(0);
    let next = if wrap {
        match direction {
            FloorStep::Previous => (current + snap.floor_ids.len() - 1) % snap.floor_ids.len(),
            FloorStep::Next => (current + 1) % snap.floor_ids.len(),
        }
    } else {
        match direction {
            FloorStep::Previous => {
                if current == 0 {
                    return;
                }
                current - 1
            }
            FloorStep::Next => {
                if current + 1 >= snap.floor_ids.len() {
                    return;
                }
                current + 1
            }
        }
    };
    let floor_id = snap.floor_ids[next];
    inspection.selected_floor_id = Some(floor_id);
    overlay_focus.blueprint_floor_id = Some(floor_id);
    inspection.selection = crate::dev::inspector::BlueprintEditSelection::None;
    if inspection.has_pending_generated_draft() {
        inspection.sync_selected_region_from_draft();
    } else {
        inspection.sync_selected_region_from_working_copy();
    }
    inspection.clear_selection_if_stale();
    inspection.pending_connection_regions = None;
    refresh_editor_snapshot(
        world,
        building_catalog,
        nav_catalog,
        building_id,
        inspection,
        inspector,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloorStep {
    Previous,
    Next,
}

pub fn set_edit_tool(inspection: &mut BlueprintInspectionState, tool: BlueprintEditTool) {
    inspection.active_tool = tool;
}

pub fn refresh_editor_snapshot(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    building_id: BuildingId,
    inspection: &BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
) {
    crate::dev::inspector::refresh_blueprint_edit_snapshot(
        world,
        building_catalog,
        nav_catalog,
        building_id,
        inspection,
        inspector,
    );
}

pub fn sync_session_with_selection(
    world_selection: &WorldSelectionState,
    world: &WorldData,
    inspection: &mut BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
    overlay_focus: &mut InspectorOverlayFocus,
    camera: &mut RtsCameraState,
) {
    let building_id = match world_selection.category {
        WorldSelectionCategory::Building => world_selection.building_id,
        _ => None,
    };

    if building_id != inspection.building_id {
        if inspection.active {
            exit_blueprint_inspection(inspection, overlay_focus, camera);
            inspector.blueprint_snapshot = None;
        }
    }

    if let Some(id) = building_id {
        if world.get_building(id).is_none() {
            exit_blueprint_inspection(inspection, overlay_focus, camera);
            inspector.last_message = format!("Building #{} no longer exists", id.raw());
            inspector.blueprint_snapshot = None;
        }
    }
}

pub fn start_variant_draft(
    inspection: &mut BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    building_id: BuildingId,
    ui: &mut NavigationEditorUiState,
) {
    let Some(record) = world.get_building(building_id) else {
        return;
    };
    let Some(definition) = building_catalog.get(&record.definition_id) else {
        return;
    };
    let display_name = format!("{} Variant", definition.display_name);
    let asset_id =
        crate::world::suggest_variant_definition_id(record.definition_id.as_str(), &display_name);
    ui.variant_display_name = display_name.clone();
    ui.variant_asset_id = asset_id.clone();
    ui.variant_description.clear();
    inspection.variant_draft = Some(BlueprintVariantDraft {
        source_definition_id: record.definition_id.clone(),
        display_name,
        asset_id,
        description: String::new(),
        active_field: BlueprintVariantDraftField::DisplayName,
    });
    inspector.last_message = "Save As Variant - fill fields and confirm".into();
}

pub fn exit_inspection_session(
    inspection: &mut BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
    overlay_focus: &mut InspectorOverlayFocus,
    camera: &mut RtsCameraState,
) {
    exit_blueprint_inspection(inspection, overlay_focus, camera);
    if let Some(snap) = inspector.blueprint_snapshot.as_mut() {
        snap.inspection_active = false;
        snap.edit_active = false;
    }
    inspector.last_message = "Exited blueprint inspection".into();
}

pub fn exit_edit_session(
    inspection: &mut BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
) {
    if inspection.dirty {
        inspection.pending_confirmation = Some(BlueprintPendingConfirmation::DiscardEdits {
            action: "exit edit".into(),
        });
        inspector.last_message = "Unsaved blueprint edits - confirm discard or cancel".into();
        return;
    }
    exit_blueprint_edit_to_inspect(inspection);
    if let Some(snap) = inspector.blueprint_snapshot.as_mut() {
        snap.edit_active = false;
        snap.edit_dirty = false;
    }
    inspector.last_message = "Exited blueprint edit".into();
}

pub fn authority_tooltip(source: &str) -> &'static str {
    match source {
        s if s.contains("Instance") => {
            "Instance Override: edits affect only this placed building until you Apply to Asset."
        }
        s if s.contains("Asset") => {
            "Asset Default: shared blueprint for all instances without an override."
        }
        s if s.contains("Generated") => {
            "Generated fallback: procedural starting blueprint; save to persist corrections."
        }
        _ => "No resolved blueprint - generate or assign a catalog blueprint first.",
    }
}
