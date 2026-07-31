//! Building navigation blueprint editor input and picking (NV1.4).

use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::{CameraSettings, RtsCamera, RtsCameraState};
use crate::client::selection::WorldSelectionState;
use crate::debug::{DebugOverlayConfig, InspectorOverlayFocus};
use crate::dev::dev_mode::DevModeInputGate;
use crate::dev::window::DevWindowRegistry;
use crate::dev::{DevModeState, DevPanelHoverState};
use crate::terrain::TerrainRenderAssets;
use crate::units::input::cursor_world_ray;
use crate::world::{
    BuildingCatalog, BuildingCatalogRevision, BuildingCategoryCatalog, BuildingDefinitionId,
    BuildingId, BuildingNavigationBlueprint, BuildingNavigationBlueprintCatalog,
    BuildingNavigationBlueprintCatalogRevision, BuildingVariantCreateInput, InteriorProfileCatalog,
    WorldConfig, WorldData, apply_blueprint_to_asset, building_model_render_transform,
    count_inheriting_instances, create_building_variant, delete_entrance, delete_floor_vertex,
    delete_transition, insert_vertex_on_edge, move_entrance, move_floor_vertex,
    move_transition_from, move_transition_to, replace_building_instance_definition,
    reset_instance_to_asset, save_instance_blueprint, set_entrance_radius, set_transition_radius,
    suggest_variant_definition_id, validate_blueprint_for_inspection,
    validate_building_definition_id,
};

use super::blueprint_inspection::{
    BlueprintEditDrag, BlueprintEditSelection, BlueprintEditTool, BlueprintInspectionState,
    BlueprintPendingConfirmation, BlueprintVariantDraft, BlueprintVariantDraftField,
    capture_edit_blueprint_snapshot, enter_blueprint_inspection, frame_building_for_inspection,
};
use super::state::WorldInspectorState;

const VERTEX_PICK_RADIUS: f32 = 0.45;
const EDGE_PICK_RADIUS: f32 = 0.35;
const ENTRANCE_PICK_RADIUS: f32 = 0.6;
const TRANSITION_PICK_RADIUS: f32 = 0.75;
/// Reject near-parallel plane hits that would produce extreme blueprint coordinates.
const MAX_LOCAL_XZ_ABS: f32 = 50_000.0;

/// Cursor ray intersection with the active floor editing plane (render space).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloorPlaneHit {
    /// Intersection point in render/world space (matches camera and overlay).
    pub world_point: Vec3,
    /// Building-local blueprint XZ stored in the draft.
    pub local_xz: Vec2,
}

#[derive(SystemParam)]
pub struct BlueprintEditInputParams<'w, 's> {
    pub dev_state: Res<'w, DevModeState>,
    pub panel_hovered: Res<'w, DevPanelHoverState>,
    pub gate: ResMut<'w, DevModeInputGate>,
    pub keyboard: Res<'w, ButtonInput<KeyCode>>,
    pub mouse_buttons: Res<'w, ButtonInput<MouseButton>>,
    pub windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    pub camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<RtsCamera>>,
    pub rts_camera: Query<'w, 's, &'static mut RtsCameraState, With<RtsCamera>>,
    pub inspection: ResMut<'w, BlueprintInspectionState>,
    pub world_selection: Res<'w, WorldSelectionState>,
    pub inspector: ResMut<'w, WorldInspectorState>,
    pub overlay_focus: ResMut<'w, InspectorOverlayFocus>,
    pub debug_config: ResMut<'w, DebugOverlayConfig>,
    pub world: ResMut<'w, WorldData>,
    pub config: Res<'w, WorldConfig>,
    pub building_catalog: ResMut<'w, BuildingCatalog>,
    pub category_catalog: Res<'w, BuildingCategoryCatalog>,
    pub building_revision: ResMut<'w, BuildingCatalogRevision>,
    pub interior_catalog: Res<'w, InteriorProfileCatalog>,
    pub nav_catalog: ResMut<'w, BuildingNavigationBlueprintCatalog>,
    pub nav_revision: ResMut<'w, BuildingNavigationBlueprintCatalogRevision>,
    pub camera_settings: Res<'w, CameraSettings>,
    pub render_assets: Option<Res<'w, TerrainRenderAssets>>,
    pub window_registry: ResMut<'w, DevWindowRegistry>,
    pub nav_ui: ResMut<'w, crate::dev::NavigationEditorUiState>,
}

pub fn enter_blueprint_edit(
    building_id: BuildingId,
    inspection: &mut BlueprintInspectionState,
    overlay_focus: &mut InspectorOverlayFocus,
    camera: &mut RtsCameraState,
    snapshot: &super::snapshot::BuildingBlueprintInspectorSnapshot,
    pitch_max: f32,
    distance_min: f32,
    distance_max: f32,
    debug_config: &mut DebugOverlayConfig,
    working: BuildingNavigationBlueprint,
) {
    enter_blueprint_inspection(
        building_id,
        inspection,
        overlay_focus,
        camera,
        snapshot,
        pitch_max,
        distance_min,
        distance_max,
        debug_config,
    );
    inspection.editing = true;
    inspection.dirty = false;
    inspection.working_copy = Some(working);
    inspection.selection = BlueprintEditSelection::None;
    inspection.active_tool = BlueprintEditTool::Select;
    inspection.drag = None;
}

pub fn exit_blueprint_edit_to_inspect(inspection: &mut BlueprintInspectionState) {
    inspection.editing = false;
    inspection.working_copy = None;
    inspection.dirty = false;
    inspection.selection = BlueprintEditSelection::None;
    inspection.active_tool = BlueprintEditTool::Select;
    inspection.drag = None;
}

/// While blueprint editing owns the world pointer, suppress gameplay selection/commands.
pub fn navigation_edit_owns_world_pointer(editing: bool, panel_hovered: bool) -> bool {
    editing && !panel_hovered
}

pub fn handle_blueprint_edit_input(mut params: BlueprintEditInputParams<'_, '_>) {
    if !params.dev_state.enabled {
        return;
    }
    if !params
        .window_registry
        .is_visible(crate::dev::window::DevWindowId::NavigationEditor)
    {
        return;
    }

    let Some(building_id) = params.world_selection.building_id else {
        return;
    };

    if !params.inspection.editing {
        return;
    }

    if params.inspection.pending_confirmation.is_some() {
        return;
    }

    if params.inspection.variant_draft.is_some() {
        return;
    }

    // Own world mouse for the whole edit session so left-release cannot ClearSelection
    // (which would trip the dirty Save/Discard/Cancel guard).
    if navigation_edit_owns_world_pointer(true, params.panel_hovered.hovered) {
        params.gate.block_gameplay_mouse = true;
    }

    let delete_pressed = (params.keyboard.just_pressed(KeyCode::Delete)
        || params.keyboard.just_pressed(KeyCode::Backspace))
        && !params.dev_state.has_text_focus();

    if delete_pressed {
        if delete_selection(&mut params.inspection) {
            params.inspector.last_message = "Deleted selected blueprint element".into();
            refresh_edit_snapshot(
                &params.world,
                &params.building_catalog,
                &params.nav_catalog,
                building_id,
                &params.inspection,
                &mut params.inspector,
            );
        }
        return;
    }

    if params.panel_hovered.hovered {
        return;
    }

    let Some(record) = params.world.get_building(building_id) else {
        return;
    };
    let Some(definition) = params.building_catalog.get(&record.definition_id) else {
        return;
    };
    let layout = params.config.chunk_layout();
    let vertical_scale = params
        .render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    // Camera rays and overlay gizmos use render space; pick/place must match.
    let model =
        building_model_render_transform(definition, &record.placement, layout, vertical_scale);
    let floor_id = params.inspection.selected_floor_id;
    let floor_elevation = params
        .inspection
        .working_copy
        .as_ref()
        .and_then(|blueprint| {
            floor_id.and_then(|id| {
                blueprint
                    .floors
                    .iter()
                    .find(|floor| floor.floor_id == id)
                    .map(|floor| floor.elevation_meters)
            })
        })
        .unwrap_or(0.0);

    let Ok(window) = params.windows.single() else {
        return;
    };
    let Ok((_camera_component, _camera_transform)) = params.camera.single() else {
        return;
    };
    let Some(_cursor) = window.cursor_position() else {
        return;
    };
    let Some(ray) = cursor_world_ray(&params.windows, &params.camera) else {
        return;
    };

    let Some(hit) = cursor_ray_to_floor_blueprint_point(&ray, &model, floor_elevation) else {
        // Invalid / parallel ray: consume the click, place nothing.
        if params.mouse_buttons.just_pressed(MouseButton::Left) {
            params.inspection.last_pick_message =
                Some("Cursor ray does not intersect the active floor plane".into());
        }
        if params.mouse_buttons.just_released(MouseButton::Left) {
            params.inspection.drag = None;
        }
        return;
    };
    let local_xz = hit.local_xz;

    if params.mouse_buttons.just_pressed(MouseButton::Left) {
        handle_edit_click(&mut params.inspection, local_xz);
        refresh_edit_snapshot(
            &params.world,
            &params.building_catalog,
            &params.nav_catalog,
            building_id,
            &params.inspection,
            &mut params.inspector,
        );
    }

    if params.mouse_buttons.pressed(MouseButton::Left) {
        if params.inspection.drag.is_some() {
            if let Some(drag) = params.inspection.drag.clone() {
                apply_drag(&mut params.inspection, drag, local_xz);
                refresh_edit_snapshot(
                    &params.world,
                    &params.building_catalog,
                    &params.nav_catalog,
                    building_id,
                    &params.inspection,
                    &mut params.inspector,
                );
            }
        } else {
            params.inspection.drag = drag_from_selection(params.inspection.selection.clone());
        }
    }

    if params.mouse_buttons.just_released(MouseButton::Left) {
        params.inspection.drag = None;
    }
}

fn handle_variant_draft_input(params: &mut BlueprintEditInputParams<'_, '_>) -> bool {
    let Some(building_id) = params.world_selection.building_id else {
        return false;
    };
    let Some(mut draft) = params.inspection.variant_draft.clone() else {
        return false;
    };

    if params.keyboard.just_pressed(KeyCode::Tab) {
        draft.active_field = match draft.active_field {
            BlueprintVariantDraftField::DisplayName => BlueprintVariantDraftField::AssetId,
            BlueprintVariantDraftField::AssetId => BlueprintVariantDraftField::Description,
            BlueprintVariantDraftField::Description => BlueprintVariantDraftField::DisplayName,
        };
        params.inspection.variant_draft = Some(draft);
        return true;
    }

    if params.keyboard.just_pressed(KeyCode::Enter) {
        let Some(working) = params.inspection.working_copy.clone() else {
            params.inspector.last_message = "No working blueprint to save as variant".into();
            return true;
        };
        if let Err(err) = validate_building_definition_id(&draft.asset_id, &params.building_catalog)
        {
            params.inspector.last_message = format!("Invalid asset id: {err}");
            return true;
        }
        if draft.display_name.trim().is_empty() {
            params.inspector.last_message = "Variant display name must not be empty".into();
            return true;
        }
        let new_definition_id = BuildingDefinitionId::new(draft.asset_id.trim());
        let description = if draft.description.trim().is_empty() {
            None
        } else {
            Some(draft.description.trim().to_string())
        };
        match create_building_variant(
            &mut params.building_catalog,
            &params.category_catalog,
            &mut params.nav_catalog,
            &mut params.nav_revision,
            BuildingVariantCreateInput {
                source_definition_id: draft.source_definition_id.clone(),
                new_definition_id: new_definition_id.clone(),
                display_name: draft.display_name.clone(),
                description,
                blueprint: working,
            },
        ) {
            Ok(outcome) => {
                params.building_revision.0 = params.building_revision.0.saturating_add(1);
                params.inspection.variant_draft = None;
                params.inspection.dirty = false;
                params.inspection.pending_confirmation =
                    Some(BlueprintPendingConfirmation::ReplaceInstanceWithVariant {
                        definition_id: new_definition_id,
                    });
                params.inspector.last_message = format!(
                    "{} — replace this instance with `{}`? [Enter] yes or Cancel in Selected Object",
                    outcome.message,
                    outcome.definition_id.as_str()
                );
            }
            Err(err) => params.inspector.last_message = format!("Save As Variant failed: {err}"),
        }
        return true;
    }

    if params.keyboard.just_pressed(KeyCode::Backspace) {
        match draft.active_field {
            BlueprintVariantDraftField::DisplayName => draft.display_name.pop(),
            BlueprintVariantDraftField::AssetId => draft.asset_id.pop(),
            BlueprintVariantDraftField::Description => draft.description.pop(),
        };
        params.inspection.variant_draft = Some(draft);
        return true;
    }

    let allow_underscore = draft.active_field == BlueprintVariantDraftField::AssetId;
    for key in params.keyboard.get_just_pressed() {
        if let Some(ch) = variant_draft_char(*key, allow_underscore) {
            match draft.active_field {
                BlueprintVariantDraftField::DisplayName => draft.display_name.push(ch),
                BlueprintVariantDraftField::AssetId => draft.asset_id.push(ch),
                BlueprintVariantDraftField::Description => draft.description.push(ch),
            }
        }
    }
    params.inspection.variant_draft = Some(draft);
    let _ = building_id;
    true
}

fn variant_draft_char(key: KeyCode, allow_underscore: bool) -> Option<char> {
    match key {
        KeyCode::Minus if allow_underscore => Some('_'),
        KeyCode::Digit0 => Some('0'),
        KeyCode::Digit1 => Some('1'),
        KeyCode::Digit2 => Some('2'),
        KeyCode::Digit3 => Some('3'),
        KeyCode::Digit4 => Some('4'),
        KeyCode::Digit5 => Some('5'),
        KeyCode::Digit6 => Some('6'),
        KeyCode::Digit7 => Some('7'),
        KeyCode::Digit8 => Some('8'),
        KeyCode::Digit9 => Some('9'),
        KeyCode::KeyA => Some('a'),
        KeyCode::KeyB => Some('b'),
        KeyCode::KeyC => Some('c'),
        KeyCode::KeyD => Some('d'),
        KeyCode::KeyE => Some('e'),
        KeyCode::KeyF => Some('f'),
        KeyCode::KeyG => Some('g'),
        KeyCode::KeyH => Some('h'),
        KeyCode::KeyI => Some('i'),
        KeyCode::KeyJ => Some('j'),
        KeyCode::KeyK => Some('k'),
        KeyCode::KeyL => Some('l'),
        KeyCode::KeyM => Some('m'),
        KeyCode::KeyN => Some('n'),
        KeyCode::KeyO => Some('o'),
        KeyCode::KeyP => Some('p'),
        KeyCode::KeyQ => Some('q'),
        KeyCode::KeyR => Some('r'),
        KeyCode::KeyS => Some('s'),
        KeyCode::KeyT => Some('t'),
        KeyCode::KeyU => Some('u'),
        KeyCode::KeyV => Some('v'),
        KeyCode::KeyW => Some('w'),
        KeyCode::KeyX => Some('x'),
        KeyCode::KeyY => Some('y'),
        KeyCode::KeyZ => Some('z'),
        KeyCode::Space if !allow_underscore => Some(' '),
        _ => None,
    }
}

fn handle_pending_confirmation(params: &mut BlueprintEditInputParams<'_, '_>) -> bool {
    params.inspection.pending_confirmation.is_some()
}

/// Execute the pending blueprint confirmation (Navigation Editor buttons, Slice 7).
pub fn confirm_blueprint_pending_action(params: &mut BlueprintEditInputParams<'_, '_>) -> bool {
    let Some(building_id) = params.world_selection.building_id else {
        return false;
    };
    let Some(pending) = params.inspection.pending_confirmation.take() else {
        return false;
    };

    match pending {
        BlueprintPendingConfirmation::DiscardEdits { .. } => {
            exit_blueprint_edit_to_inspect(&mut params.inspection);
            if let Some(snap) = params.inspector.blueprint_snapshot.as_mut() {
                snap.edit_active = false;
                snap.edit_dirty = false;
            }
            params.inspector.last_message =
                "Exited blueprint edit (unsaved changes discarded)".into();
            refresh_edit_snapshot(
                &params.world,
                &params.building_catalog,
                &params.nav_catalog,
                building_id,
                &params.inspection,
                &mut params.inspector,
            );
        }
        BlueprintPendingConfirmation::ApplyToAsset { .. } => {
            let Some(record) = params.world.get_building(building_id) else {
                return true;
            };
            let definition_id = record.definition_id.clone();
            let Some(working) = params.inspection.working_copy.clone() else {
                params.inspector.last_message = "No working blueprint to apply".into();
                return true;
            };
            match apply_blueprint_to_asset(
                &mut params.world,
                &params.building_catalog,
                &params.interior_catalog,
                &mut params.nav_catalog,
                &mut params.nav_revision,
                &definition_id,
                working,
            ) {
                Ok(outcome) => {
                    params.inspection.dirty = false;
                    params.inspector.last_message = outcome.message;
                    refresh_edit_snapshot(
                        &params.world,
                        &params.building_catalog,
                        &params.nav_catalog,
                        building_id,
                        &params.inspection,
                        &mut params.inspector,
                    );
                }
                Err(err) => params.inspector.last_message = format!("Apply to asset failed: {err}"),
            }
        }
        BlueprintPendingConfirmation::ResetToAsset => {
            match reset_instance_to_asset(
                &mut params.world,
                &params.building_catalog,
                &params.interior_catalog,
                &params.nav_catalog,
                building_id,
            ) {
                Ok(outcome) => {
                    params.inspection.dirty = false;
                    if let Some(snap) = capture_edit_blueprint_snapshot(
                        &params.world,
                        &params.building_catalog,
                        &params.nav_catalog,
                        building_id,
                        params.inspection.selected_floor_id,
                        None,
                    ) {
                        if let Some(working) = snap.resolved_blueprint.clone() {
                            params.inspection.working_copy = Some(working);
                        }
                    }
                    params.inspector.last_message = format!(
                        "{} (authority: {})",
                        outcome.message,
                        outcome.authority.label()
                    );
                    refresh_edit_snapshot(
                        &params.world,
                        &params.building_catalog,
                        &params.nav_catalog,
                        building_id,
                        &params.inspection,
                        &mut params.inspector,
                    );
                }
                Err(err) => params.inspector.last_message = format!("Reset to asset failed: {err}"),
            }
        }
        BlueprintPendingConfirmation::RegenerateFromMesh { .. } => {
            #[cfg(feature = "data-import")]
            {
                match crate::world::regenerate_navigation_blueprint_for_building(
                    building_id,
                    &params.world,
                    &params.building_catalog,
                ) {
                    Ok((report, blueprint)) => {
                        params.inspection.working_copy = Some(blueprint);
                        params.inspection.editing = true;
                        params.inspection.sync_selected_floor_from_working_copy();
                        params.inspection.dirty = true;
                        params.overlay_focus.blueprint_floor_id =
                            params.inspection.selected_floor_id;
                        params.debug_config.nav_blueprint = true;
                        params.nav_ui.regeneration_source_label = report.mesh_source_label.clone();
                        params.nav_ui.generation_diagnostics = Some(
                            crate::dev::navigation_editor::NavigationGenerationDiagnostics {
                                entrances_generated: report
                                    .entrance_diagnostics
                                    .entrances_generated,
                                explicit_markers: report.entrance_diagnostics.explicit_markers,
                                synthesized_entrances: report
                                    .entrance_diagnostics
                                    .synthesized_entrances,
                                deduplicated_candidates: report
                                    .entrance_diagnostics
                                    .deduplicated_candidates,
                                regeneration_source: report
                                    .mesh_source_label
                                    .clone()
                                    .unwrap_or_else(|| "unknown".into()),
                                candidate_details: report.entrance_diagnostics.candidate_details,
                            },
                        );
                        let source = report.mesh_source_label.as_deref().unwrap_or("unknown");
                        params.inspector.last_message = format!(
                            "Regenerated draft {} ({:?}) from {source} - use Save Instance or Apply to Asset to persist",
                            report.blueprint_id, report.status
                        );
                        refresh_edit_snapshot(
                            &params.world,
                            &params.building_catalog,
                            &params.nav_catalog,
                            building_id,
                            &params.inspection,
                            &mut params.inspector,
                        );
                    }
                    Err(err) => {
                        params.inspector.last_message =
                            format!("Blueprint regeneration failed: {err}")
                    }
                }
            }
            #[cfg(not(feature = "data-import"))]
            {
                params.inspector.last_message =
                    "Blueprint regeneration requires data-import feature".into();
            }
        }
        BlueprintPendingConfirmation::ReplaceInstanceWithVariant { definition_id } => {
            match replace_building_instance_definition(
                &mut params.world,
                &params.building_catalog,
                &params.interior_catalog,
                &params.nav_catalog,
                building_id,
                definition_id.clone(),
            ) {
                Ok(()) => {
                    params.inspector.last_message = format!(
                        "Replaced building #{} with variant `{}`",
                        building_id.raw(),
                        definition_id.as_str()
                    );
                    refresh_edit_snapshot(
                        &params.world,
                        &params.building_catalog,
                        &params.nav_catalog,
                        building_id,
                        &params.inspection,
                        &mut params.inspector,
                    );
                }
                Err(err) => {
                    params.inspector.last_message =
                        format!("Variant created but instance replace failed: {err}");
                }
            }
        }
    }
    true
}

/// Returns false when blueprint editing should block selecting another building.
pub fn blueprint_edit_blocks_building_selection(inspection: &BlueprintInspectionState) -> bool {
    inspection.editing && inspection.dirty
}

/// Refresh inspector blueprint snapshot from the current edit session (Slice 7).
pub fn refresh_blueprint_edit_snapshot(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    building_id: BuildingId,
    inspection: &BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
) {
    refresh_edit_snapshot(
        world,
        building_catalog,
        nav_catalog,
        building_id,
        inspection,
        inspector,
    );
}

pub fn editor_save_instance_blueprint(params: &mut BlueprintEditInputParams<'_, '_>) {
    let Some(building_id) = params.world_selection.building_id else {
        return;
    };
    let Some(working) = params.inspection.working_copy.clone() else {
        params.inspector.last_message = "No working blueprint to save".into();
        return;
    };
    match save_instance_blueprint(
        &mut params.world,
        &params.building_catalog,
        &params.interior_catalog,
        &params.nav_catalog,
        building_id,
        working,
    ) {
        Ok(outcome) => {
            params.inspection.dirty = false;
            if let Some(working) = params.inspection.working_copy.as_mut() {
                if let Some(record) = params.world.get_building(building_id) {
                    if let Some(override_data) =
                        record.interior.navigation_blueprint_override.as_ref()
                    {
                        if let Some(inline) = &override_data.inline_blueprint {
                            *working = inline.clone();
                        }
                    }
                }
            }
            params.inspector.last_message = format!(
                "{} (authority: {})",
                outcome.message,
                outcome.authority.label()
            );
            refresh_edit_snapshot(
                &params.world,
                &params.building_catalog,
                &params.nav_catalog,
                building_id,
                &params.inspection,
                &mut params.inspector,
            );
        }
        Err(err) => params.inspector.last_message = format!("Save instance failed: {err}"),
    }
}

pub fn editor_request_apply_to_asset(params: &mut BlueprintEditInputParams<'_, '_>) {
    let Some(building_id) = params.world_selection.building_id else {
        return;
    };
    let Some(record) = params.world.get_building(building_id) else {
        return;
    };
    let inheriting = count_inheriting_instances(&params.world, &record.definition_id);
    params.inspection.pending_confirmation = Some(BlueprintPendingConfirmation::ApplyToAsset {
        inheriting_count: inheriting,
    });
    params.inspector.last_message = format!(
        "Apply blueprint to asset default? {inheriting} instance(s) without overrides may inherit this change."
    );
}

pub fn editor_request_reset_to_asset(params: &mut BlueprintEditInputParams<'_, '_>) {
    params.inspection.pending_confirmation = Some(BlueprintPendingConfirmation::ResetToAsset);
    params.inspector.last_message =
        "Reset instance to asset/generated blueprint? Confirm or cancel.".into();
}

pub fn editor_delete_selection(params: &mut BlueprintEditInputParams<'_, '_>) {
    let Some(building_id) = params.world_selection.building_id else {
        return;
    };
    if delete_selection(&mut params.inspection) {
        params.inspector.last_message = "Deleted selected blueprint element".into();
        refresh_edit_snapshot(
            &params.world,
            &params.building_catalog,
            &params.nav_catalog,
            building_id,
            &params.inspection,
            &mut params.inspector,
        );
    }
}

pub fn editor_adjust_radius(params: &mut BlueprintEditInputParams<'_, '_>, delta: f32) {
    let Some(building_id) = params.world_selection.building_id else {
        return;
    };
    if adjust_selected_radius(&mut params.inspection, delta) {
        refresh_edit_snapshot(
            &params.world,
            &params.building_catalog,
            &params.nav_catalog,
            building_id,
            &params.inspection,
            &mut params.inspector,
        );
    }
}

pub fn editor_submit_variant_draft(
    params: &mut BlueprintEditInputParams<'_, '_>,
    display_name: &str,
    asset_id: &str,
    description: &str,
) {
    let Some(building_id) = params.world_selection.building_id else {
        return;
    };
    let Some(working) = params.inspection.working_copy.clone() else {
        params.inspector.last_message = "No working blueprint to save as variant".into();
        return;
    };
    if let Err(err) = validate_building_definition_id(asset_id, &params.building_catalog) {
        params.inspector.last_message = format!("Invalid asset id: {err}");
        return;
    }
    if display_name.trim().is_empty() {
        params.inspector.last_message = "Variant display name must not be empty".into();
        return;
    }
    let new_definition_id = BuildingDefinitionId::new(asset_id.trim());
    let description = if description.trim().is_empty() {
        None
    } else {
        Some(description.trim().to_string())
    };
    let source_definition_id = params
        .world
        .get_building(building_id)
        .map(|r| r.definition_id.clone())
        .unwrap_or_else(|| BuildingDefinitionId::new("unknown"));
    match create_building_variant(
        &mut params.building_catalog,
        &params.category_catalog,
        &mut params.nav_catalog,
        &mut params.nav_revision,
        BuildingVariantCreateInput {
            source_definition_id,
            new_definition_id: new_definition_id.clone(),
            display_name: display_name.to_string(),
            description,
            blueprint: working,
        },
    ) {
        Ok(outcome) => {
            params.building_revision.0 = params.building_revision.0.saturating_add(1);
            params.inspection.variant_draft = None;
            params.inspection.dirty = false;
            params.inspection.pending_confirmation =
                Some(BlueprintPendingConfirmation::ReplaceInstanceWithVariant {
                    definition_id: new_definition_id,
                });
            params.inspector.last_message = format!(
                "{} — replace this instance with `{}`?",
                outcome.message,
                outcome.definition_id.as_str()
            );
        }
        Err(err) => params.inspector.last_message = format!("Save As Variant failed: {err}"),
    }
}

fn refresh_edit_snapshot(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    building_id: BuildingId,
    inspection: &BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
) {
    if let Some(mut snap) = capture_edit_blueprint_snapshot(
        world,
        building_catalog,
        nav_catalog,
        building_id,
        inspection.selected_floor_id,
        inspection.working_copy.as_ref(),
    ) {
        snap.inspection_active = inspection.active;
        snap.edit_active = inspection.editing;
        snap.edit_dirty = inspection.dirty;
        snap.selected_element = selection_label(&inspection.selection);
        if let Some(draft) = &inspection.variant_draft {
            snap.variant_draft_active = true;
            snap.variant_draft_display_name = Some(draft.display_name.clone());
            snap.variant_draft_asset_id = Some(draft.asset_id.clone());
            snap.variant_draft_description = Some(draft.description.clone());
            snap.variant_draft_active_field = Some(
                match draft.active_field {
                    BlueprintVariantDraftField::DisplayName => "display name",
                    BlueprintVariantDraftField::AssetId => "asset id",
                    BlueprintVariantDraftField::Description => "description",
                }
                .into(),
            );
        } else {
            snap.variant_draft_active = false;
            snap.variant_draft_display_name = None;
            snap.variant_draft_asset_id = None;
            snap.variant_draft_description = None;
            snap.variant_draft_active_field = None;
        }
        inspector.blueprint_snapshot = Some(snap);
        if let Some(validation) = inspection
            .working_copy
            .as_ref()
            .map(validate_blueprint_for_inspection)
        {
            if let Some(snapshot) = inspector.blueprint_snapshot.as_mut() {
                snapshot.validation = validation;
            }
        }
    }
}

fn selection_label(selection: &BlueprintEditSelection) -> Option<String> {
    match selection {
        BlueprintEditSelection::None => None,
        BlueprintEditSelection::Vertex { floor_id, index } => {
            Some(format!("vertex floor {floor_id} #{index}"))
        }
        BlueprintEditSelection::Edge { floor_id, index } => {
            Some(format!("edge floor {floor_id} #{index}"))
        }
        BlueprintEditSelection::Entrance { key } => Some(format!("entrance {key}")),
        BlueprintEditSelection::Transition { key } => Some(format!("transition {key}")),
        BlueprintEditSelection::TransitionTo { key } => Some(format!("transition target {key}")),
    }
}

fn handle_edit_click(inspection: &mut BlueprintInspectionState, local_xz: Vec2) {
    let Some(blueprint) = inspection.working_copy.as_mut() else {
        return;
    };
    let Some(floor_id) = inspection.selected_floor_id else {
        inspection.last_pick_message =
            Some("Select a floor before editing blueprint geometry".into());
        return;
    };
    let floor_key = blueprint
        .floors
        .iter()
        .find(|floor| floor.floor_id == floor_id)
        .map(|floor| floor.key.clone());

    match inspection.active_tool {
        BlueprintEditTool::Select => {
            if let Some((kind, selection)) = pick_blueprint_element(blueprint, floor_id, local_xz) {
                inspection.selection = selection;
                inspection.last_pick_message = Some(kind.to_string());
                return;
            }
            inspection.selection = BlueprintEditSelection::None;
        }
        BlueprintEditTool::AddVertex => {
            let edge_index = pick_edge(blueprint, floor_id, local_xz, EDGE_PICK_RADIUS)
                .or_else(|| pick_nearest_edge(blueprint, floor_id, local_xz));
            if let Some(edge_index) = edge_index {
                let outcome = insert_vertex_on_edge(
                    blueprint,
                    floor_id,
                    edge_index,
                    [local_xz.x, local_xz.y],
                );
                if outcome.applied {
                    inspection.dirty = true;
                    inspection.selection = BlueprintEditSelection::Vertex {
                        floor_id,
                        index: edge_index + 1,
                    };
                    // Add Corner is one-shot: return to Select after a successful place.
                    inspection.active_tool = BlueprintEditTool::Select;
                } else {
                    inspection.last_pick_message = outcome.message;
                }
            } else {
                inspection.last_pick_message = Some(
                    "No floor edge found — frame the building and click near the outline".into(),
                );
            }
        }
        BlueprintEditTool::AddEntrance => {
            if let Some(floor_key) = floor_key {
                let outcome = crate::world::add_entrance_on_floor(
                    blueprint,
                    &floor_key,
                    [local_xz.x, local_xz.y],
                    1.5,
                );
                if outcome.applied {
                    inspection.dirty = true;
                    if let Some(entrance) = blueprint.entrances.last() {
                        inspection.selection = BlueprintEditSelection::Entrance {
                            key: entrance.key.clone(),
                        };
                    }
                } else {
                    inspection.last_pick_message = outcome.message;
                }
            }
        }
    }
}

fn apply_drag(inspection: &mut BlueprintInspectionState, drag: BlueprintEditDrag, local_xz: Vec2) {
    let Some(blueprint) = inspection.working_copy.as_mut() else {
        return;
    };
    let point = [local_xz.x, local_xz.y];
    let outcome = match drag {
        BlueprintEditDrag::Vertex { floor_id, index } => {
            move_floor_vertex(blueprint, floor_id, index, point)
        }
        BlueprintEditDrag::Entrance { key } => move_entrance(blueprint, &key, point),
        BlueprintEditDrag::TransitionFrom { key } => move_transition_from(blueprint, &key, point),
        BlueprintEditDrag::TransitionTo { key } => {
            let Some(transition) = blueprint
                .vertical_transitions
                .iter()
                .find(|transition| transition.key == key)
            else {
                return;
            };
            let mut target = transition.to_local_position;
            target[0] = point[0];
            target[2] = point[1];
            move_transition_to(blueprint, &key, target)
        }
    };
    if outcome.applied {
        inspection.dirty = true;
    } else {
        inspection.last_pick_message = outcome.message;
    }
}

fn drag_from_selection(selection: BlueprintEditSelection) -> Option<BlueprintEditDrag> {
    match selection {
        BlueprintEditSelection::Vertex { floor_id, index } => {
            Some(BlueprintEditDrag::Vertex { floor_id, index })
        }
        BlueprintEditSelection::Entrance { key } => Some(BlueprintEditDrag::Entrance { key }),
        BlueprintEditSelection::Transition { key } => {
            Some(BlueprintEditDrag::TransitionFrom { key })
        }
        BlueprintEditSelection::TransitionTo { key } => {
            Some(BlueprintEditDrag::TransitionTo { key })
        }
        BlueprintEditSelection::None | BlueprintEditSelection::Edge { .. } => None,
    }
}

fn delete_selection(inspection: &mut BlueprintInspectionState) -> bool {
    let Some(blueprint) = inspection.working_copy.as_mut() else {
        return false;
    };
    let outcome = match &inspection.selection {
        BlueprintEditSelection::Vertex { floor_id, index } => {
            delete_floor_vertex(blueprint, *floor_id, *index)
        }
        BlueprintEditSelection::Entrance { key } => delete_entrance(blueprint, key),
        BlueprintEditSelection::Transition { key }
        | BlueprintEditSelection::TransitionTo { key } => delete_transition(blueprint, key),
        BlueprintEditSelection::None | BlueprintEditSelection::Edge { .. } => {
            return false;
        }
    };
    if outcome.applied {
        inspection.dirty = true;
        inspection.selection = BlueprintEditSelection::None;
        true
    } else {
        inspection.last_pick_message = outcome.message;
        false
    }
}

fn adjust_selected_radius(inspection: &mut BlueprintInspectionState, delta: f32) -> bool {
    let Some(blueprint) = inspection.working_copy.as_mut() else {
        return false;
    };
    let outcome = match &inspection.selection {
        BlueprintEditSelection::Entrance { key } => {
            let radius = blueprint
                .entrances
                .iter()
                .find(|entrance| entrance.key == *key)
                .map(|entrance| (entrance.radius_meters + delta).max(0.25))
                .unwrap_or(1.5);
            set_entrance_radius(blueprint, key, radius)
        }
        BlueprintEditSelection::Transition { key } => {
            let radius = blueprint
                .vertical_transitions
                .iter()
                .find(|transition| transition.key == *key)
                .map(|transition| (transition.from_radius_meters + delta).max(0.25))
                .unwrap_or(1.25);
            set_transition_radius(blueprint, key, radius)
        }
        _ => return false,
    };
    if outcome.applied {
        inspection.dirty = true;
        true
    } else {
        inspection.last_pick_message = outcome.message;
        false
    }
}

fn pick_blueprint_element(
    blueprint: &BuildingNavigationBlueprint,
    floor_id: i32,
    local_xz: Vec2,
) -> Option<(&'static str, BlueprintEditSelection)> {
    if let Some(index) = pick_vertex(blueprint, floor_id, local_xz, VERTEX_PICK_RADIUS) {
        return Some(("vertex", BlueprintEditSelection::Vertex { floor_id, index }));
    }
    if let Some(key) = pick_transition_to(blueprint, floor_id, local_xz, TRANSITION_PICK_RADIUS) {
        return Some((
            "transition target",
            BlueprintEditSelection::TransitionTo { key },
        ));
    }
    if let Some(key) = pick_transition_from(blueprint, floor_id, local_xz, TRANSITION_PICK_RADIUS) {
        return Some(("transition", BlueprintEditSelection::Transition { key }));
    }
    if let Some(key) = pick_entrance(blueprint, floor_id, local_xz, ENTRANCE_PICK_RADIUS) {
        return Some(("entrance", BlueprintEditSelection::Entrance { key }));
    }
    if let Some(index) = pick_edge(blueprint, floor_id, local_xz, EDGE_PICK_RADIUS) {
        return Some(("edge", BlueprintEditSelection::Edge { floor_id, index }));
    }
    None
}

fn pick_vertex(
    blueprint: &BuildingNavigationBlueprint,
    floor_id: i32,
    local_xz: Vec2,
    radius: f32,
) -> Option<usize> {
    let floor = blueprint
        .floors
        .iter()
        .find(|floor| floor.floor_id == floor_id)?;
    let mut best: Option<(f32, usize)> = None;
    for (index, &[x, z]) in floor.walkable_outline.vertices_xz.iter().enumerate() {
        let dist = Vec2::new(x, z).distance(local_xz);
        if dist <= radius && best.map(|(best_dist, _)| dist < best_dist).unwrap_or(true) {
            best = Some((dist, index));
        }
    }
    best.map(|(_, index)| index)
}

fn pick_edge(
    blueprint: &BuildingNavigationBlueprint,
    floor_id: i32,
    local_xz: Vec2,
    radius: f32,
) -> Option<usize> {
    pick_nearest_edge_within_radius(blueprint, floor_id, local_xz, Some(radius))
}

fn pick_nearest_edge(
    blueprint: &BuildingNavigationBlueprint,
    floor_id: i32,
    local_xz: Vec2,
) -> Option<usize> {
    pick_nearest_edge_within_radius(blueprint, floor_id, local_xz, None)
}

fn pick_nearest_edge_within_radius(
    blueprint: &BuildingNavigationBlueprint,
    floor_id: i32,
    local_xz: Vec2,
    max_radius: Option<f32>,
) -> Option<usize> {
    let floor = blueprint
        .floors
        .iter()
        .find(|floor| floor.floor_id == floor_id)?;
    let verts = &floor.walkable_outline.vertices_xz;
    if verts.len() < 2 {
        return None;
    }
    let mut best: Option<(f32, usize)> = None;
    for index in 0..verts.len() {
        let [ax, az] = verts[index];
        let [bx, bz] = verts[(index + 1) % verts.len()];
        let dist = point_segment_distance(local_xz, Vec2::new(ax, az), Vec2::new(bx, bz));
        let within_radius = max_radius.is_none_or(|radius| dist <= radius);
        if within_radius && best.map(|(best_dist, _)| dist < best_dist).unwrap_or(true) {
            best = Some((dist, index));
        }
    }
    best.map(|(_, index)| index)
}

fn pick_entrance(
    blueprint: &BuildingNavigationBlueprint,
    floor_id: i32,
    local_xz: Vec2,
    radius: f32,
) -> Option<String> {
    let floor = blueprint
        .floors
        .iter()
        .find(|floor| floor.floor_id == floor_id)?;
    let mut best: Option<(f32, String)> = None;
    for entrance in &blueprint.entrances {
        if entrance.floor_key != floor.key {
            continue;
        }
        let center = Vec2::new(entrance.local_position_xz[0], entrance.local_position_xz[1]);
        let dist = center.distance(local_xz);
        let threshold = entrance.radius_meters.max(radius);
        if dist <= threshold
            && best
                .as_ref()
                .map(|(best_dist, _)| dist < *best_dist)
                .unwrap_or(true)
        {
            best = Some((dist, entrance.key.clone()));
        }
    }
    best.map(|(_, key)| key)
}

fn pick_transition_from(
    blueprint: &BuildingNavigationBlueprint,
    floor_id: i32,
    local_xz: Vec2,
    radius: f32,
) -> Option<String> {
    let mut best: Option<(f32, String)> = None;
    for transition in &blueprint.vertical_transitions {
        let Some(from_floor) = blueprint.floor_by_key(&transition.from_floor_key) else {
            continue;
        };
        if from_floor.floor_id != floor_id {
            continue;
        }
        let center = Vec2::new(
            transition.from_local_position_xz[0],
            transition.from_local_position_xz[1],
        );
        let dist = center.distance(local_xz);
        let threshold = transition.from_radius_meters.max(radius);
        if dist <= threshold
            && best
                .as_ref()
                .map(|(best_dist, _)| dist < *best_dist)
                .unwrap_or(true)
        {
            best = Some((dist, transition.key.clone()));
        }
    }
    best.map(|(_, key)| key)
}

fn pick_transition_to(
    blueprint: &BuildingNavigationBlueprint,
    floor_id: i32,
    local_xz: Vec2,
    radius: f32,
) -> Option<String> {
    let mut best: Option<(f32, String)> = None;
    for transition in &blueprint.vertical_transitions {
        let Some(to_floor) = blueprint.floor_by_key(&transition.to_floor_key) else {
            continue;
        };
        if to_floor.floor_id != floor_id {
            continue;
        }
        let center = Vec2::new(
            transition.to_local_position[0],
            transition.to_local_position[2],
        );
        let dist = center.distance(local_xz);
        if dist <= radius
            && best
                .as_ref()
                .map(|(best_dist, _)| dist < *best_dist)
                .unwrap_or(true)
        {
            best = Some((dist, transition.key.clone()));
        }
    }
    best.map(|(_, key)| key)
}

fn point_segment_distance(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= f32::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    point.distance(a + ab * t)
}

/// Project a cursor ray onto the active floor plane and convert to blueprint local XZ.
///
/// `model_transform` must be the same render-space building transform used by the
/// blueprint overlay (`building_model_render_transform`), not the authoritative
/// world transform alone (terrain vertical scale would otherwise laterally skew hits).
pub fn cursor_ray_to_floor_blueprint_point(
    ray: &Ray3d,
    model_transform: &Transform,
    floor_elevation: f32,
) -> Option<FloorPlaneHit> {
    let plane_point = blueprint_local_to_world(model_transform, Vec2::ZERO, floor_elevation);
    let plane_normal = model_transform.rotation * Vec3::Y;
    let world_point = ray_plane_intersection(ray, plane_point, plane_normal)?;
    if !world_point.is_finite() {
        return None;
    }
    let local_xz = world_point_to_blueprint_local_xz(model_transform, world_point)?;
    Some(FloorPlaneHit {
        world_point,
        local_xz,
    })
}

/// Inverse of [`blueprint_local_to_world`] for XZ (floor elevation is recovered from the plane).
pub fn world_point_to_blueprint_local_xz(
    model_transform: &Transform,
    world_point: Vec3,
) -> Option<Vec2> {
    let world_from_local = Affine3A::from_scale_rotation_translation(
        model_transform.scale,
        model_transform.rotation,
        model_transform.translation,
    );
    let local = world_from_local.inverse().transform_point3(world_point);
    if !local.is_finite() {
        return None;
    }
    let local_xz = Vec2::new(local.x, local.z);
    if !local_xz.is_finite()
        || local_xz.x.abs() > MAX_LOCAL_XZ_ABS
        || local_xz.y.abs() > MAX_LOCAL_XZ_ABS
    {
        return None;
    }
    Some(local_xz)
}

/// Transform a blueprint local XZ + floor elevation into the model transform's space
/// (render space when given `building_model_render_transform`).
pub fn blueprint_local_to_world(
    model_transform: &Transform,
    local_xz: Vec2,
    floor_elevation: f32,
) -> Vec3 {
    model_transform.transform_point(Vec3::new(local_xz.x, floor_elevation, local_xz.y))
}

/// Convenience wrapper used by callers that only need local XZ.
pub fn ray_to_building_floor_local_xz(
    ray: &Ray3d,
    model_transform: &Transform,
    floor_elevation: f32,
) -> Option<Vec2> {
    cursor_ray_to_floor_blueprint_point(ray, model_transform, floor_elevation)
        .map(|hit| hit.local_xz)
}

fn ray_plane_intersection(ray: &Ray3d, plane_point: Vec3, plane_normal: Vec3) -> Option<Vec3> {
    let normal = plane_normal.normalize_or_zero();
    if normal.length_squared() < 1e-8 {
        return None;
    }
    let denom = ray.direction.dot(normal);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (plane_point - ray.origin).dot(normal) / denom;
    if t < 0.0 || !t.is_finite() {
        return None;
    }
    let hit = ray.origin + ray.direction * t;
    if !hit.is_finite() {
        return None;
    }
    Some(hit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingNavigationBlueprint, NavigationFloorDefinition, NavigationPolygon2d,
    };
    use bevy::math::Dir3;

    fn identity_model() -> Transform {
        Transform::IDENTITY
    }

    fn model_trs(translation: Vec3, rotation: Quat, uniform_scale: f32) -> Transform {
        Transform {
            translation,
            rotation,
            scale: Vec3::splat(uniform_scale),
        }
    }

    fn downward_ray_through(xz: Vec2, y: f32) -> Ray3d {
        Ray3d {
            origin: Vec3::new(xz.x, y, xz.y),
            direction: Dir3::new(Vec3::NEG_Y).unwrap(),
        }
    }

    fn sample_blueprint() -> BuildingNavigationBlueprint {
        BuildingNavigationBlueprint::new("test_nav", "Test").with_floors(vec![
            NavigationFloorDefinition {
                floor_id: 0,
                key: "floor_0".into(),
                display_label: "Floor 0".into(),
                elevation_meters: 0.0,
                visibility_group_id: 0,
                room_tag: None,
                walkable_outline: NavigationPolygon2d {
                    vertices_xz: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
                },
            },
        ])
    }

    #[test]
    fn navigation_edit_owns_world_pointer_while_editing_over_world() {
        assert!(navigation_edit_owns_world_pointer(true, false));
        assert!(!navigation_edit_owns_world_pointer(true, true));
        assert!(!navigation_edit_owns_world_pointer(false, false));
    }

    #[test]
    fn origin_identity_placement_matches_cursor_xz() {
        let model = identity_model();
        let expected = Vec2::new(3.5, -2.0);
        let ray = downward_ray_through(expected, 25.0);
        let hit = cursor_ray_to_floor_blueprint_point(&ray, &model, 0.0).unwrap();
        assert!(hit.local_xz.distance(expected) < 1e-4);
        assert!(
            hit.world_point
                .distance(Vec3::new(expected.x, 0.0, expected.y))
                < 1e-4
        );
    }

    #[test]
    fn translated_building_converts_world_cursor_to_local() {
        let model = model_trs(Vec3::new(100.0, 4.0, -50.0), Quat::IDENTITY, 1.0);
        let local = Vec2::new(2.0, 3.0);
        let world = blueprint_local_to_world(&model, local, 0.0);
        let ray = downward_ray_through(Vec2::new(world.x, world.z), world.y + 40.0);
        let hit = cursor_ray_to_floor_blueprint_point(&ray, &model, 0.0).unwrap();
        assert!(hit.local_xz.distance(local) < 1e-4);
    }

    #[test]
    fn rotated_building_preserves_local_round_trip() {
        let yaw = Quat::from_rotation_y(std::f32::consts::FRAC_PI_3);
        let model = model_trs(Vec3::new(10.0, 0.0, 20.0), yaw, 1.0);
        let local = Vec2::new(4.0, -1.5);
        let elev = 2.5;
        let world = blueprint_local_to_world(&model, local, elev);
        let back = world_point_to_blueprint_local_xz(&model, world).unwrap();
        assert!(back.distance(local) < 1e-4);

        // Angled ray toward the floor plane point.
        let offset = Vec3::new(5.0, 12.0, 5.0);
        let ray = Ray3d {
            origin: world + offset,
            direction: Dir3::new(-offset.normalize()).unwrap(),
        };
        let hit = cursor_ray_to_floor_blueprint_point(&ray, &model, elev).unwrap();
        assert!(hit.local_xz.distance(local) < 1e-3);
        assert!(hit.world_point.distance(world) < 1e-3);
    }

    #[test]
    fn uniformly_scaled_building_preserves_local_round_trip() {
        let model = model_trs(Vec3::new(-8.0, 1.0, 4.0), Quat::IDENTITY, 2.5);
        let local = Vec2::new(1.0, 2.0);
        let world = blueprint_local_to_world(&model, local, 0.0);
        let back = world_point_to_blueprint_local_xz(&model, world).unwrap();
        assert!(back.distance(local) < 1e-4);
        let ray = downward_ray_through(Vec2::new(world.x, world.z), world.y + 30.0);
        let hit = cursor_ray_to_floor_blueprint_point(&ray, &model, 0.0).unwrap();
        assert!(hit.local_xz.distance(local) < 1e-4);
    }

    #[test]
    fn translated_rotated_scaled_with_floor_elevation_round_trips() {
        let model = model_trs(
            Vec3::new(30.0, 6.0, -12.0),
            Quat::from_rotation_y(-0.7),
            1.75,
        );
        for elev in [-3.0_f32, 0.0, 4.5] {
            let local = Vec2::new(-2.25, 5.5);
            let world = blueprint_local_to_world(&model, local, elev);
            let back = world_point_to_blueprint_local_xz(&model, world).unwrap();
            assert!(
                back.distance(local) < 1e-4,
                "elev={elev}: {back:?} vs {local:?}"
            );
            let ray = Ray3d {
                origin: world + Vec3::new(0.0, 20.0, 0.0),
                direction: Dir3::new(Vec3::NEG_Y).unwrap(),
            };
            let hit = cursor_ray_to_floor_blueprint_point(&ray, &model, elev).unwrap();
            assert!(hit.local_xz.distance(local) < 1e-4);
            assert!(hit.world_point.distance(world) < 1e-4);
        }
    }

    #[test]
    fn preview_world_matches_committed_local_transformed_back() {
        let model = model_trs(Vec3::new(5.0, 2.0, 9.0), Quat::from_rotation_y(0.4), 1.2);
        let elev = 3.0;
        let ray = Ray3d {
            origin: Vec3::new(8.0, 40.0, 11.0),
            direction: Dir3::new(Vec3::NEG_Y).unwrap(),
        };
        let preview = cursor_ray_to_floor_blueprint_point(&ray, &model, elev).unwrap();
        // Commit stores local_xz only; overlay reconstructs world from the same helper path.
        let committed_world = blueprint_local_to_world(&model, preview.local_xz, elev);
        assert!(preview.world_point.distance(committed_world) < 1e-4);
    }

    #[test]
    fn parallel_ray_rejects_placement() {
        let model = identity_model();
        let ray = Ray3d {
            origin: Vec3::new(0.0, 5.0, 0.0),
            direction: Dir3::new(Vec3::X).unwrap(),
        };
        assert!(cursor_ray_to_floor_blueprint_point(&ray, &model, 0.0).is_none());
        assert!(ray_to_building_floor_local_xz(&ray, &model, 0.0).is_none());
    }

    #[test]
    fn select_vertex_does_not_mark_dirty() {
        let mut inspection = BlueprintInspectionState {
            editing: true,
            dirty: false,
            selected_floor_id: Some(0),
            active_tool: BlueprintEditTool::Select,
            working_copy: Some(sample_blueprint()),
            ..Default::default()
        };
        handle_edit_click(&mut inspection, Vec2::new(0.1, 0.05));
        assert!(!inspection.dirty);
        assert_eq!(
            inspection.selection,
            BlueprintEditSelection::Vertex {
                floor_id: 0,
                index: 0
            }
        );
        assert!(inspection.pending_confirmation.is_none());
    }

    #[test]
    fn select_empty_clears_vertex_not_building_session() {
        let mut inspection = BlueprintInspectionState {
            editing: true,
            dirty: true,
            building_id: Some(BuildingId(7)),
            selected_floor_id: Some(0),
            active_tool: BlueprintEditTool::Select,
            selection: BlueprintEditSelection::Vertex {
                floor_id: 0,
                index: 1,
            },
            working_copy: Some(sample_blueprint()),
            ..Default::default()
        };
        handle_edit_click(&mut inspection, Vec2::new(50.0, 50.0));
        assert_eq!(inspection.selection, BlueprintEditSelection::None);
        assert!(inspection.dirty);
        assert_eq!(inspection.building_id, Some(BuildingId(7)));
        assert!(inspection.editing);
        assert!(inspection.pending_confirmation.is_none());
    }

    #[test]
    fn add_vertex_places_at_local_cursor_and_marks_dirty() {
        let mut inspection = BlueprintInspectionState {
            editing: true,
            dirty: false,
            selected_floor_id: Some(0),
            active_tool: BlueprintEditTool::AddVertex,
            working_copy: Some(sample_blueprint()),
            ..Default::default()
        };
        let cursor = Vec2::new(2.0, 0.05);
        handle_edit_click(&mut inspection, cursor);
        assert!(inspection.dirty);
        assert_eq!(inspection.active_tool, BlueprintEditTool::Select);
        assert_eq!(
            inspection.selection,
            BlueprintEditSelection::Vertex {
                floor_id: 0,
                index: 1
            }
        );
        let blueprint = inspection.working_copy.as_ref().unwrap();
        assert_eq!(blueprint.floors[0].walkable_outline.vertices_xz.len(), 5);
        assert_eq!(
            blueprint.floors[0].walkable_outline.vertices_xz[1],
            [cursor.x, cursor.y]
        );
    }

    #[test]
    fn add_vertex_invalid_click_keeps_add_corner_tool() {
        let mut inspection = BlueprintInspectionState {
            editing: true,
            dirty: false,
            selected_floor_id: Some(0),
            active_tool: BlueprintEditTool::AddVertex,
            working_copy: Some(sample_blueprint()),
            ..Default::default()
        };
        // Degenerate outline so insert_vertex_on_edge rejects (needs >= 3 verts).
        inspection.working_copy.as_mut().unwrap().floors[0]
            .walkable_outline
            .vertices_xz = vec![[0.0, 0.0], [1.0, 0.0]];
        handle_edit_click(&mut inspection, Vec2::new(0.5, 0.0));
        assert!(!inspection.dirty);
        assert_eq!(inspection.active_tool, BlueprintEditTool::AddVertex);
        assert_eq!(
            inspection.working_copy.as_ref().unwrap().floors[0]
                .walkable_outline
                .vertices_xz
                .len(),
            2
        );
    }

    #[test]
    fn add_vertex_one_shot_adds_exactly_one_corner_per_activation() {
        let mut inspection = BlueprintInspectionState {
            editing: true,
            dirty: false,
            selected_floor_id: Some(0),
            active_tool: BlueprintEditTool::AddVertex,
            working_copy: Some(sample_blueprint()),
            ..Default::default()
        };
        handle_edit_click(&mut inspection, Vec2::new(2.0, 0.0));
        assert_eq!(inspection.active_tool, BlueprintEditTool::Select);
        let count_after_first = inspection.working_copy.as_ref().unwrap().floors[0]
            .walkable_outline
            .vertices_xz
            .len();
        // Second click while Select must not insert another corner.
        handle_edit_click(&mut inspection, Vec2::new(3.0, 0.0));
        assert_eq!(
            inspection.working_copy.as_ref().unwrap().floors[0]
                .walkable_outline
                .vertices_xz
                .len(),
            count_after_first
        );
    }

    #[test]
    fn angled_ray_requires_render_space_plane_when_vertical_scale_differs() {
        let world_model = model_trs(Vec3::new(10.0, 8.0, 0.0), Quat::IDENTITY, 1.0);
        let mut render_model = world_model;
        render_model.translation.y = world_model.translation.y * 3.0;
        let elev = 2.0;
        let local = Vec2::new(1.5, -0.5);
        let overlay_point = blueprint_local_to_world(&render_model, local, elev);
        let offset = Vec3::new(10.0, 25.0, 6.0);
        let ray = Ray3d {
            origin: overlay_point + offset,
            direction: Dir3::new(-offset.normalize()).unwrap(),
        };

        let correct = cursor_ray_to_floor_blueprint_point(&ray, &render_model, elev).unwrap();
        assert!(correct.local_xz.distance(local) < 1e-3);

        let wrong = cursor_ray_to_floor_blueprint_point(&ray, &world_model, elev).unwrap();
        assert!(
            wrong.local_xz.distance(local) > 0.5,
            "authoritative-only plane should laterally skew angled hits; got {:?}",
            wrong.local_xz
        );
    }

    #[test]
    fn dirty_guard_still_required_for_building_change() {
        let mut inspection = BlueprintInspectionState {
            editing: true,
            dirty: true,
            ..Default::default()
        };
        assert!(blueprint_edit_blocks_building_selection(&inspection));
        inspection.dirty = false;
        assert!(!blueprint_edit_blocks_building_selection(&inspection));
    }
}
