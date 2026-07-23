//! Building navigation blueprint editor input and picking (NV1.4).

use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::{CameraSettings, RtsCamera, RtsCameraState};
use crate::debug::{DebugOverlayConfig, InspectorOverlayFocus};
use bevy::math::Affine3A;
use crate::dev::{DevModeState, DevPanelHoverState};
use crate::terrain::TerrainRenderAssets;
use crate::units::input::cursor_world_ray;
use crate::world::{
    apply_blueprint_to_asset, count_inheriting_instances, create_building_variant,
    replace_building_instance_definition, reset_instance_to_asset, save_instance_blueprint,
    suggest_variant_definition_id, validate_building_definition_id, BuildingCatalog,
    BuildingCatalogRevision, BuildingCategoryCatalog, BuildingDefinitionId, BuildingId,
    BuildingNavigationBlueprint, BuildingNavigationBlueprintCatalog,
    BuildingNavigationBlueprintCatalogRevision, BuildingVariantCreateInput, InteriorProfileCatalog,
    WorldConfig, WorldData, building_model_world_transform, delete_entrance, delete_floor_vertex,
    delete_transition, insert_vertex_on_edge, move_entrance, move_floor_vertex, move_transition_from,
    move_transition_to, set_entrance_radius, set_transition_radius, validate_blueprint_for_inspection,
};

use super::blueprint_inspection::{
    capture_edit_blueprint_snapshot, enter_blueprint_inspection, frame_building_for_inspection,
    BlueprintEditDrag, BlueprintEditSelection, BlueprintEditTool, BlueprintInspectionState,
    BlueprintPendingConfirmation, BlueprintVariantDraft, BlueprintVariantDraftField,
};
use super::state::WorldInspectorState;

const VERTEX_PICK_RADIUS: f32 = 0.45;
const EDGE_PICK_RADIUS: f32 = 0.35;
const ENTRANCE_PICK_RADIUS: f32 = 0.6;
const TRANSITION_PICK_RADIUS: f32 = 0.75;

#[derive(SystemParam)]
pub struct BlueprintEditInputParams<'w, 's> {
    pub dev_state: Res<'w, DevModeState>,
    pub panel_hovered: Res<'w, DevPanelHoverState>,
    pub keyboard: Res<'w, ButtonInput<KeyCode>>,
    pub mouse_buttons: Res<'w, ButtonInput<MouseButton>>,
    pub windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    pub camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<RtsCamera>>,
    pub rts_camera: Query<'w, 's, &'static mut RtsCameraState, With<RtsCamera>>,
    pub inspection: ResMut<'w, BlueprintInspectionState>,
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

pub fn handle_blueprint_edit_input(mut params: BlueprintEditInputParams<'_, '_>) {
    if !params.dev_state.enabled {
        return;
    }

    let Some(building_id) = params.inspector.selected_building else {
        return;
    };

    if params.keyboard.just_pressed(KeyCode::KeyE) && !params.inspection.editing {
        let Some(snapshot) = capture_edit_blueprint_snapshot(
            &params.world,
            &params.building_catalog,
            &params.nav_catalog,
            building_id,
            params.inspection.selected_floor_id,
            params.inspection.working_copy.as_ref(),
        ) else {
            params.inspector.last_message = "No navigation blueprint available to edit".into();
            return;
        };
        let Some(working) = snapshot.resolved_blueprint.clone() else {
            params.inspector.last_message = "No navigation blueprint available to edit".into();
            return;
        };
        if let Ok(mut cam) = params.rts_camera.single_mut() {
            enter_blueprint_edit(
                building_id,
                &mut params.inspection,
                &mut params.overlay_focus,
                &mut cam,
                &snapshot,
                params.camera_settings.pitch_max,
                params.camera_settings.distance_min,
                params.camera_settings.distance_max,
                &mut params.debug_config,
                working,
            );
            params.inspector.blueprint_snapshot = Some(snapshot);
            params.inspector.last_message =
                format!(
                    "Blueprint edit: building #{} — [Ctrl+S] instance [Ctrl+Shift+S] asset [Ctrl+Shift+V] variant",
                    building_id.raw()
                );
            if let Some(snap) = params.inspector.blueprint_snapshot.as_mut() {
                snap.edit_active = true;
            }
        }
        return;
    }

    if !params.inspection.editing {
        return;
    }

    if params.keyboard.just_pressed(KeyCode::BracketLeft)
        || params.keyboard.just_pressed(KeyCode::BracketRight)
    {
        if let Some(snap) = params.inspector.blueprint_snapshot.clone() {
            if !snap.floor_ids.is_empty() {
                let current = params
                    .inspection
                    .selected_floor_id
                    .and_then(|id| snap.floor_ids.iter().position(|&f| f == id))
                    .unwrap_or(0);
                let next = if params.keyboard.just_pressed(KeyCode::BracketRight) {
                    (current + 1) % snap.floor_ids.len()
                } else {
                    (current + snap.floor_ids.len() - 1) % snap.floor_ids.len()
                };
                let floor_id = snap.floor_ids[next];
                params.inspection.selected_floor_id = Some(floor_id);
                params.overlay_focus.blueprint_floor_id = Some(floor_id);
                params.inspection.selection = BlueprintEditSelection::None;
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
        return;
    }

    if handle_variant_draft_input(&mut params) {
        return;
    }

    if handle_pending_confirmation(&mut params) {
        return;
    }

    if params.keyboard.just_pressed(KeyCode::Escape) {
        if params.inspection.dirty {
            params.inspection.pending_confirmation = Some(BlueprintPendingConfirmation::DiscardEdits {
                action: "exit edit".into(),
            });
            params.inspector.last_message =
                "Unsaved blueprint edits — [Enter] discard, [Esc] cancel".into();
            return;
        }
        exit_blueprint_edit_to_inspect(&mut params.inspection);
        if let Some(snap) = params.inspector.blueprint_snapshot.as_mut() {
            snap.edit_active = false;
            snap.edit_dirty = false;
        }
        params.inspector.last_message = "Exited blueprint edit".into();
        refresh_edit_snapshot(
            &params.world,
            &params.building_catalog,
            &params.nav_catalog,
            building_id,
            &params.inspection,
            &mut params.inspector,
        );
        return;
    }

    let ctrl = params.keyboard.pressed(KeyCode::ControlLeft)
        || params.keyboard.pressed(KeyCode::ControlRight);
    let shift = params.keyboard.pressed(KeyCode::ShiftLeft)
        || params.keyboard.pressed(KeyCode::ShiftRight);
    let alt = params.keyboard.pressed(KeyCode::AltLeft)
        || params.keyboard.pressed(KeyCode::AltRight);

    if ctrl && shift && params.keyboard.just_pressed(KeyCode::KeyV) {
        let Some(record) = params.world.get_building(building_id) else {
            return;
        };
        let Some(definition) = params.building_catalog.get(&record.definition_id) else {
            return;
        };
        let display_name = format!("{} Variant", definition.display_name);
        let asset_id = suggest_variant_definition_id(
            record.definition_id.as_str(),
            &display_name,
        );
        params.inspection.variant_draft = Some(BlueprintVariantDraft {
            source_definition_id: record.definition_id.clone(),
            display_name,
            asset_id,
            description: String::new(),
            active_field: BlueprintVariantDraftField::DisplayName,
        });
        params.inspector.last_message =
            "Save As Variant — [Tab] next field, type name/id/description, [Enter] create, [Esc] cancel"
                .into();
        return;
    }

    if ctrl && shift && params.keyboard.just_pressed(KeyCode::KeyS) {
        let Some(record) = params.world.get_building(building_id) else {
            return;
        };
        let inheriting = count_inheriting_instances(&params.world, &record.definition_id);
        params.inspection.pending_confirmation =
            Some(BlueprintPendingConfirmation::ApplyToAsset { inheriting_count: inheriting });
        params.inspector.last_message = format!(
            "Apply blueprint to asset default? {inheriting} loaded instance(s) without overrides inherit this change — [Enter] confirm, [Esc] cancel"
        );
        return;
    }

    if ctrl && alt && params.keyboard.just_pressed(KeyCode::KeyR) {
        params.inspection.pending_confirmation = Some(BlueprintPendingConfirmation::ResetToAsset);
        params.inspector.last_message =
            "Reset instance to asset/generated blueprint? [Enter] confirm, [Esc] cancel".into();
        return;
    }

    if ctrl && !shift && params.keyboard.just_pressed(KeyCode::KeyS) {
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
                        if let Some(override_data) = record.interior.navigation_blueprint_override.as_ref() {
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
        return;
    }

    if params.keyboard.just_pressed(KeyCode::Digit1) {
        params.inspection.active_tool = BlueprintEditTool::Select;
        params.inspector.last_message = "Blueprint tool: select".into();
    }
    if params.keyboard.just_pressed(KeyCode::Digit2) {
        params.inspection.active_tool = BlueprintEditTool::AddVertex;
        params.inspector.last_message = "Blueprint tool: add vertex (click edge)".into();
    }
    if params.keyboard.just_pressed(KeyCode::Digit3) {
        params.inspection.active_tool = BlueprintEditTool::AddEntrance;
        params.inspector.last_message = "Blueprint tool: add entrance (click floor)".into();
    }

    if params.keyboard.just_pressed(KeyCode::Delete) || params.keyboard.just_pressed(KeyCode::Backspace) {
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

    if params.keyboard.just_pressed(KeyCode::Equal) || params.keyboard.just_pressed(KeyCode::NumpadAdd) {
        if adjust_selected_radius(&mut params.inspection, 0.1) {
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
    if params.keyboard.just_pressed(KeyCode::Minus) || params.keyboard.just_pressed(KeyCode::NumpadSubtract) {
        if adjust_selected_radius(&mut params.inspection, -0.1) {
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

    let Some(record) = params.world.get_building(building_id) else {
        return;
    };
    let Some(definition) = params.building_catalog.get(&record.definition_id) else {
        return;
    };
    let layout = params.config.chunk_layout();
    let model = building_model_world_transform(definition, &record.placement, layout);
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

    if params.panel_hovered.hovered {
        return;
    }

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

    let local_xz = match ray_to_building_floor_local_xz(&ray, &model, floor_elevation) {
        Some(point) => point,
        None => return,
    };

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
    let Some(building_id) = params.inspector.selected_building else {
        return false;
    };
    let Some(mut draft) = params.inspection.variant_draft.clone() else {
        return false;
    };

    if params.keyboard.just_pressed(KeyCode::Escape) {
        params.inspection.variant_draft = None;
        params.inspector.last_message = "Cancelled Save As Variant".into();
        return true;
    }

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
        if let Err(err) = validate_building_definition_id(&draft.asset_id, &params.building_catalog) {
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
                    "{} — replace this instance with `{}`? [Enter] yes, [Esc] keep current asset",
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
    let Some(building_id) = params.inspector.selected_building else {
        return false;
    };
    let Some(pending) = params.inspection.pending_confirmation.clone() else {
        return false;
    };

    if params.keyboard.just_pressed(KeyCode::Escape) {
        params.inspection.pending_confirmation = None;
        params.inspector.last_message = match &pending {
            BlueprintPendingConfirmation::ReplaceInstanceWithVariant { definition_id } => {
                format!(
                    "Variant `{}` created — kept current instance asset",
                    definition_id.as_str()
                )
            }
            _ => "Cancelled pending blueprint action".into(),
        };
        return true;
    }

    if !params.keyboard.just_pressed(KeyCode::Enter) {
        return true;
    }

    params.inspection.pending_confirmation = None;
    match pending {
        BlueprintPendingConfirmation::DiscardEdits { .. } => {
            exit_blueprint_edit_to_inspect(&mut params.inspection);
            if let Some(snap) = params.inspector.blueprint_snapshot.as_mut() {
                snap.edit_active = false;
                snap.edit_dirty = false;
            }
            params.inspector.last_message = "Exited blueprint edit (unsaved changes discarded)".into();
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
                    &mut params.nav_catalog,
                    &mut params.nav_revision,
                ) {
                    Ok(report) => {
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
                            "Regenerated blueprint {} ({:?})",
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
                        params.inspector.last_message = format!("Blueprint regeneration failed: {err}")
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
            snap.variant_draft_active_field = Some(match draft.active_field {
                BlueprintVariantDraftField::DisplayName => "display name",
                BlueprintVariantDraftField::AssetId => "asset id",
                BlueprintVariantDraftField::Description => "description",
            }.into());
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
            if let Some(edge_index) = pick_edge(blueprint, floor_id, local_xz, EDGE_PICK_RADIUS) {
                let outcome = insert_vertex_on_edge(
                    blueprint,
                    floor_id,
                    edge_index,
                    [local_xz.x, local_xz.y],
                );
                if outcome.applied {
                    inspection.dirty = true;
                    inspection.selection =
                        BlueprintEditSelection::Vertex { floor_id, index: edge_index + 1 };
                } else {
                    inspection.last_pick_message = outcome.message;
                }
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
                        inspection.selection =
                            BlueprintEditSelection::Entrance { key: entrance.key.clone() };
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
        BlueprintEditSelection::TransitionTo { key } => Some(BlueprintEditDrag::TransitionTo { key }),
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
        BlueprintEditSelection::Transition { key } | BlueprintEditSelection::TransitionTo { key } => {
            delete_transition(blueprint, key)
        }
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
        return Some((
            "vertex",
            BlueprintEditSelection::Vertex { floor_id, index },
        ));
    }
    if let Some(key) = pick_transition_to(blueprint, floor_id, local_xz, TRANSITION_PICK_RADIUS) {
        return Some((
            "transition target",
            BlueprintEditSelection::TransitionTo { key },
        ));
    }
    if let Some(key) = pick_transition_from(blueprint, floor_id, local_xz, TRANSITION_PICK_RADIUS) {
        return Some((
            "transition",
            BlueprintEditSelection::Transition { key },
        ));
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
    let floor = blueprint.floors.iter().find(|floor| floor.floor_id == floor_id)?;
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
    let floor = blueprint.floors.iter().find(|floor| floor.floor_id == floor_id)?;
    let verts = &floor.walkable_outline.vertices_xz;
    if verts.len() < 2 {
        return None;
    }
    let mut best: Option<(f32, usize)> = None;
    for index in 0..verts.len() {
        let [ax, az] = verts[index];
        let [bx, bz] = verts[(index + 1) % verts.len()];
        let dist = point_segment_distance(local_xz, Vec2::new(ax, az), Vec2::new(bx, bz));
        if dist <= radius && best.map(|(best_dist, _)| dist < best_dist).unwrap_or(true) {
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
    let floor = blueprint.floors.iter().find(|floor| floor.floor_id == floor_id)?;
    let mut best: Option<(f32, String)> = None;
    for entrance in &blueprint.entrances {
        if entrance.floor_key != floor.key {
            continue;
        }
        let center = Vec2::new(entrance.local_position_xz[0], entrance.local_position_xz[1]);
        let dist = center.distance(local_xz);
        let threshold = entrance.radius_meters.max(radius);
        if dist <= threshold && best.as_ref().map(|(best_dist, _)| dist < *best_dist).unwrap_or(true) {
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
        if dist <= threshold && best.as_ref().map(|(best_dist, _)| dist < *best_dist).unwrap_or(true) {
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
        let center = Vec2::new(transition.to_local_position[0], transition.to_local_position[2]);
        let dist = center.distance(local_xz);
        if dist <= radius && best.as_ref().map(|(best_dist, _)| dist < *best_dist).unwrap_or(true) {
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

pub fn ray_to_building_floor_local_xz(
    ray: &Ray3d,
    model_transform: &Transform,
    floor_elevation: f32,
) -> Option<Vec2> {
    let plane_point = model_transform.transform_point(Vec3::new(0.0, floor_elevation, 0.0));
    let plane_normal = model_transform.rotation * Vec3::Y;
    let hit = ray_plane_intersection(ray, plane_point, plane_normal)?;
    let world_from_local = Affine3A::from_scale_rotation_translation(
        model_transform.scale,
        model_transform.rotation,
        model_transform.translation,
    );
    let local = world_from_local.inverse().transform_point3(hit);
    Some(Vec2::new(local.x, local.z))
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
    if t < 0.0 {
        return None;
    }
    Some(ray.origin + ray.direction * t)
}
