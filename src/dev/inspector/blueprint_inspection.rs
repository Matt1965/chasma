//! Building navigation blueprint read-only inspection (NV1.2.5).

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::camera::{RtsCamera, RtsCameraState};
use crate::debug::{DebugOverlayConfig, InspectorOverlayFocus};
use crate::terrain::TerrainRenderAssets;
use crate::world::{
    BuildingCatalog, BuildingId, BuildingNavigationBlueprintCatalog,
    BuildingNavigationBlueprintCatalogRevision, WorldData,
};

use super::capture::capture_building_blueprint_inspection_snapshot;
use super::snapshot::BuildingBlueprintInspectorSnapshot;
use super::state::WorldInspectorState;

#[derive(Debug, Clone, PartialEq)]
pub enum BlueprintPendingConfirmation {
    ApplyToAsset { inheriting_count: usize },
    ResetToAsset,
    RegenerateFromMesh { current_source: String },
    DiscardEdits { action: String },
    ReplaceInstanceWithVariant {
        definition_id: crate::world::BuildingDefinitionId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlueprintVariantDraftField {
    DisplayName,
    AssetId,
    Description,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintVariantDraft {
    pub source_definition_id: crate::world::BuildingDefinitionId,
    pub display_name: String,
    pub asset_id: String,
    pub description: String,
    pub active_field: BlueprintVariantDraftField,
}

/// Session state for blueprint inspection mode (camera save/restore, floor selection).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct BlueprintInspectionState {
    pub active: bool,
    pub editing: bool,
    pub dirty: bool,
    pub building_id: Option<BuildingId>,
    pub selected_floor_id: Option<i32>,
    pub focused_diagnostic_index: Option<usize>,
    pub saved_camera: Option<RtsCameraState>,
    pub working_copy: Option<crate::world::BuildingNavigationBlueprint>,
    pub selection: BlueprintEditSelection,
    pub active_tool: BlueprintEditTool,
    pub drag: Option<BlueprintEditDrag>,
    pub last_pick_message: Option<String>,
    pub pending_confirmation: Option<BlueprintPendingConfirmation>,
    pub variant_draft: Option<BlueprintVariantDraft>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BlueprintEditTool {
    #[default]
    Select,
    AddVertex,
    AddEntrance,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BlueprintEditSelection {
    #[default]
    None,
    Vertex { floor_id: i32, index: usize },
    Edge { floor_id: i32, index: usize },
    Entrance { key: String },
    Transition { key: String },
    TransitionTo { key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlueprintEditDrag {
    Vertex { floor_id: i32, index: usize },
    Entrance { key: String },
    TransitionFrom { key: String },
    TransitionTo { key: String },
}

impl BlueprintInspectionState {
    pub fn exit(&mut self) {
        self.active = false;
        self.editing = false;
        self.dirty = false;
        self.building_id = None;
        self.selected_floor_id = None;
        self.focused_diagnostic_index = None;
        self.saved_camera = None;
        self.working_copy = None;
        self.selection = BlueprintEditSelection::None;
        self.active_tool = BlueprintEditTool::Select;
        self.drag = None;
        self.last_pick_message = None;
        self.pending_confirmation = None;
        self.variant_draft = None;
    }
}

/// Reusable bird's-eye framing for a building anchor and blueprint bounds (NV1.2.5).
pub fn frame_building_for_inspection(
    camera: &mut RtsCameraState,
    building_center: Vec3,
    bounds_half_extent: f32,
    pitch_max: f32,
    distance_min: f32,
    distance_max: f32,
) {
    let padding = 1.35;
    let extent = bounds_half_extent.max(4.0) * padding;
    let distance = (extent * 2.2).clamp(distance_min, distance_max);
    camera.target_focus = building_center;
    camera.target_yaw = 0.0;
    camera.target_pitch = pitch_max * 0.98;
    camera.target_distance = distance;
    camera.focus = building_center;
    camera.yaw = 0.0;
    camera.pitch = pitch_max * 0.98;
    camera.distance = distance;
}

fn blueprint_bounds_half_extent(
    snapshot: &BuildingBlueprintInspectorSnapshot,
    building_center: Vec3,
) -> f32 {
    if snapshot.world_bounds_radius > 0.0 {
        return snapshot.world_bounds_radius;
    }
    8.0 + building_center.xz().length() * 0.0
}

pub fn enter_blueprint_inspection(
    building_id: BuildingId,
    inspection: &mut BlueprintInspectionState,
    overlay_focus: &mut InspectorOverlayFocus,
    camera: &mut RtsCameraState,
    snapshot: &BuildingBlueprintInspectorSnapshot,
    pitch_max: f32,
    distance_min: f32,
    distance_max: f32,
    debug_config: &mut DebugOverlayConfig,
) {
    if inspection.saved_camera.is_none() {
        inspection.saved_camera = Some(*camera);
    }
    inspection.active = true;
    inspection.building_id = Some(building_id);
    inspection.selected_floor_id = snapshot
        .floor_ids
        .first()
        .copied()
        .or(snapshot.selected_floor_id);
    inspection.focused_diagnostic_index = None;

    overlay_focus.blueprint_building_id = Some(building_id);
    overlay_focus.blueprint_floor_id = inspection.selected_floor_id;
    overlay_focus.blueprint_diagnostic = None;

    frame_building_for_inspection(
        camera,
        snapshot.building_center,
        blueprint_bounds_half_extent(snapshot, snapshot.building_center),
        pitch_max,
        distance_min,
        distance_max,
    );

    debug_config.nav_blueprint = true;
}

pub fn exit_blueprint_inspection(
    inspection: &mut BlueprintInspectionState,
    overlay_focus: &mut InspectorOverlayFocus,
    camera: &mut RtsCameraState,
) {
    if let Some(saved) = inspection.saved_camera.take() {
        *camera = saved;
    }
    inspection.exit();
    overlay_focus.clear_blueprint();
}

pub fn handle_blueprint_inspection_input(
    dev_state: Res<crate::dev::DevModeState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut inspection: ResMut<BlueprintInspectionState>,
    mut inspector: ResMut<WorldInspectorState>,
    mut overlay_focus: ResMut<InspectorOverlayFocus>,
    mut debug_config: ResMut<DebugOverlayConfig>,
    world: Res<WorldData>,
    building_catalog: Res<BuildingCatalog>,
    mut nav_catalog: ResMut<BuildingNavigationBlueprintCatalog>,
    mut nav_revision: ResMut<BuildingNavigationBlueprintCatalogRevision>,
    camera_settings: Res<crate::camera::CameraSettings>,
    mut camera: Query<&mut RtsCameraState, With<RtsCamera>>,
    _render_assets: Option<Res<TerrainRenderAssets>>,
) {
    if !dev_state.enabled {
        return;
    }

    let Some(building_id) = inspector.selected_building else {
        if inspection.active {
            if let Ok(mut cam) = camera.single_mut() {
                exit_blueprint_inspection(&mut inspection, &mut overlay_focus, &mut cam);
            } else {
                inspection.exit();
                overlay_focus.clear_blueprint();
            }
        }
        return;
    };

    let refresh_snapshot = || {
        capture_building_blueprint_inspection_snapshot(
            &world,
            &building_catalog,
            &nav_catalog,
            building_id,
            inspection.selected_floor_id,
        )
    };

    if keyboard.just_pressed(KeyCode::KeyN) && !inspection.active {
        let Some(mut snapshot) = refresh_snapshot() else {
            inspector.last_message = "No navigation blueprint available for this building".into();
            return;
        };
        snapshot.inspection_active = true;
        if let Ok(mut cam) = camera.single_mut() {
            enter_blueprint_inspection(
                building_id,
                &mut inspection,
                &mut overlay_focus,
                &mut cam,
                &snapshot,
                camera_settings.pitch_max,
                camera_settings.distance_min,
                camera_settings.distance_max,
                &mut debug_config,
            );
            inspector.blueprint_snapshot = Some(snapshot);
            inspector.last_message =
                format!("Blueprint inspection: building #{}", building_id.raw());
        }
        return;
    }

    if inspection.active && keyboard.just_pressed(KeyCode::Escape) && !inspection.editing {
        if let Ok(mut cam) = camera.single_mut() {
            exit_blueprint_inspection(&mut inspection, &mut overlay_focus, &mut cam);
        } else {
            inspection.exit();
            overlay_focus.clear_blueprint();
        }
        inspector.last_message = "Exited blueprint inspection".into();
        if let Some(snap) = inspector.blueprint_snapshot.as_mut() {
            snap.inspection_active = false;
        }
        return;
    }

    if !inspection.active || inspection.editing {
        return;
    }

    if inspection.building_id != Some(building_id) {
        inspection.building_id = Some(building_id);
    }

    let mut snapshot_dirty = false;

    if keyboard.just_pressed(KeyCode::BracketLeft) || keyboard.just_pressed(KeyCode::BracketRight) {
        if let Some(mut snap) = inspector.blueprint_snapshot.clone() {
            if !snap.floor_ids.is_empty() {
                let current = inspection
                    .selected_floor_id
                    .and_then(|id| snap.floor_ids.iter().position(|&f| f == id))
                    .unwrap_or(0);
                let next = if keyboard.just_pressed(KeyCode::BracketRight) {
                    (current + 1) % snap.floor_ids.len()
                } else {
                    (current + snap.floor_ids.len() - 1) % snap.floor_ids.len()
                };
                let floor_id = snap.floor_ids[next];
                inspection.selected_floor_id = Some(floor_id);
                snap.selected_floor_id = Some(floor_id);
                snap = enrich_floor_details(snap, floor_id);
                inspector.blueprint_snapshot = Some(snap);
                overlay_focus.blueprint_floor_id = Some(floor_id);
                snapshot_dirty = true;
            }
        }
    }

    if keyboard.just_pressed(KeyCode::KeyR)
        && (keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight))
    {
        if let (Some(record), Some(definition)) = (
            world.get_building(building_id),
            world
                .get_building(building_id)
                .and_then(|record| building_catalog.get(&record.definition_id)),
        ) {
            let authority = crate::world::classify_blueprint_authority(
                definition,
                &nav_catalog,
                record.interior.navigation_blueprint_override.as_ref(),
            );
            if authority != crate::world::BlueprintAuthoritySource::None {
                inspection.pending_confirmation =
                    Some(BlueprintPendingConfirmation::RegenerateFromMesh {
                        current_source: authority.label().to_string(),
                    });
                inspector.last_message = format!(
                    "Regenerate from mesh will replace the {} catalog blueprint for this asset (instance overrides are preserved) — [Enter] confirm, [Esc] cancel",
                    authority.label()
                );
            } else {
                #[cfg(feature = "data-import")]
                {
                    match crate::world::regenerate_navigation_blueprint_for_building(
                        building_id,
                        &world,
                        &building_catalog,
                        &mut nav_catalog,
                        &mut nav_revision,
                    ) {
                        Ok(report) => {
                            inspector.last_message = format!(
                                "Regenerated blueprint {} ({:?})",
                                report.blueprint_id, report.status
                            );
                            snapshot_dirty = true;
                        }
                        Err(err) => {
                            inspector.last_message = format!("Blueprint regeneration failed: {err}");
                        }
                    }
                }
                #[cfg(not(feature = "data-import"))]
                {
                    inspector.last_message =
                        "Blueprint regeneration requires data-import feature".into();
                }
            }
        }
    }

    if let Some(pending) = inspection.pending_confirmation.clone() {
        if keyboard.just_pressed(KeyCode::Escape) {
            inspection.pending_confirmation = None;
            inspector.last_message = "Cancelled pending blueprint action".into();
            return;
        }
        if keyboard.just_pressed(KeyCode::Enter) {
            inspection.pending_confirmation = None;
            if let BlueprintPendingConfirmation::RegenerateFromMesh { .. } = pending {
                #[cfg(feature = "data-import")]
                {
                    match crate::world::regenerate_navigation_blueprint_for_building(
                        building_id,
                        &world,
                        &building_catalog,
                        &mut nav_catalog,
                        &mut nav_revision,
                    ) {
                        Ok(report) => {
                            inspector.last_message = format!(
                                "Regenerated blueprint {} ({:?})",
                                report.blueprint_id, report.status
                            );
                            snapshot_dirty = true;
                        }
                        Err(err) => {
                            inspector.last_message = format!("Blueprint regeneration failed: {err}");
                        }
                    }
                }
            }
        }
        return;
    }

    for (index, key) in [
        (0, KeyCode::Digit1),
        (1, KeyCode::Digit2),
        (2, KeyCode::Digit3),
        (3, KeyCode::Digit4),
        (4, KeyCode::Digit5),
        (5, KeyCode::Digit6),
        (6, KeyCode::Digit7),
        (7, KeyCode::Digit8),
        (8, KeyCode::Digit9),
    ] {
        if keyboard.just_pressed(key) {
            if let Some(snap) = inspector.blueprint_snapshot.as_ref() {
                if index < snap.validation.diagnostics.len() {
                    inspection.focused_diagnostic_index = Some(index);
                    overlay_focus.blueprint_diagnostic = snap.validation.diagnostics[index]
                        .focus
                        .clone();
                }
            }
        }
    }

    if snapshot_dirty || inspector.blueprint_snapshot.is_none() {
        if let Some(mut snap) = capture_building_blueprint_inspection_snapshot(
            &world,
            &building_catalog,
            &nav_catalog,
            building_id,
            inspection.selected_floor_id,
        ) {
            snap.inspection_active = true;
            if let Some(floor_id) = inspection.selected_floor_id {
                snap = enrich_floor_details(snap, floor_id);
            }
            inspector.blueprint_snapshot = Some(snap);
        }
    }

    if let Ok(mut cam) = camera.single_mut() {
        if let Some(snap) = inspector.blueprint_snapshot.as_ref() {
            let center = snap.building_center;
            let half = blueprint_bounds_half_extent(snap, center);
            let target_pitch = camera_settings.pitch_max * 0.98;
            if (cam.target_focus - center).length() > 0.5
                || (cam.target_pitch - target_pitch).abs() > 0.05
            {
                frame_building_for_inspection(
                    &mut cam,
                    center,
                    half,
                    camera_settings.pitch_max,
                    camera_settings.distance_min,
                    camera_settings.distance_max,
                );
            }
        }
    }
}

/// Capture inspector snapshot using an in-progress editor working copy when provided.
pub fn capture_edit_blueprint_snapshot(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    building_id: BuildingId,
    selected_floor_id: Option<i32>,
    working_override: Option<&crate::world::BuildingNavigationBlueprint>,
) -> Option<BuildingBlueprintInspectorSnapshot> {
    let mut snapshot = capture_building_blueprint_inspection_snapshot(
        world,
        building_catalog,
        nav_catalog,
        building_id,
        selected_floor_id,
    )?;
    if let Some(working) = working_override {
        snapshot.resolved_blueprint = Some(working.clone());
        snapshot.blueprint_id = Some(working.id.as_str().to_string());
        snapshot.validation = crate::world::validate_blueprint_for_inspection(working);
        snapshot.entrance_count = working.entrances.len();
        snapshot.transition_count = working.vertical_transitions.len();
        snapshot.floor_ids = working.floors.iter().map(|floor| floor.floor_id).collect();
        if let Some(floor_id) = selected_floor_id {
            snapshot = enrich_floor_details(snapshot, floor_id);
        }
    }
    Some(snapshot)
}

fn enrich_floor_details(
    mut snap: BuildingBlueprintInspectorSnapshot,
    floor_id: i32,
) -> BuildingBlueprintInspectorSnapshot {
    snap.selected_floor_id = Some(floor_id);
    if let Some(blueprint) = snap.resolved_blueprint.as_ref() {
        if let Some(floor) = blueprint.floors.iter().find(|f| f.floor_id == floor_id) {
            snap.selected_floor_vertex_count = floor.walkable_outline.vertices_xz.len();
            snap.selected_floor_elevation = Some(floor.elevation_meters);
            snap.selected_floor_entrances = blueprint
                .entrances
                .iter()
                .filter(|e| e.floor_key == floor.key)
                .map(|e| format!("{} @ [{:.1},{:.1}] r={:.1}m", e.key, e.local_position_xz[0], e.local_position_xz[1], e.radius_meters))
                .collect();
            snap.selected_floor_transitions = blueprint
                .vertical_transitions
                .iter()
                .filter(|t| {
                    blueprint
                        .floors
                        .iter()
                        .find(|f| f.key == t.from_floor_key)
                        .map(|f| f.floor_id == floor_id)
                        .unwrap_or(false)
                })
                .map(|t| {
                    format!(
                        "{} {:?} {} → {}",
                        t.key, t.kind, t.from_floor_key, t.to_floor_key
                    )
                })
                .collect();
        }
    }
    snap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bird_eye_frame_sets_overhead_pitch() {
        let mut cam = RtsCameraState::new(Vec3::ZERO, 1.0, 0.5, 100.0);
        frame_building_for_inspection(&mut cam, Vec3::new(10.0, 0.0, 20.0), 12.0, 1.35, 40.0, 5000.0);
        assert!((cam.target_pitch - 1.35 * 0.98).abs() < 0.01);
        assert!((cam.target_focus.x - 10.0).abs() < 0.01);
        assert!(cam.target_distance >= 40.0);
    }
}
