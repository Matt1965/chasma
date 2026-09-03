//! Transform gizmo keyboard and mouse input (ADR-099).

use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::client::selection::{
    ApplyWorldSelectionParams, WorldSelectionChange, WorldSelectionRevision, WorldSelectionState,
    apply_world_selection,
};
use crate::dev::hotkeys::DEV_GIZMO_COORDINATE_SPACE;
use crate::dev::{DevModeInputGate, DevModeState, DevPanelHoverState, cancel_dev_placement};
use crate::doodads::DoodadRenderIndex;
use crate::terrain::world_position_to_render_global;
use crate::ui::gameplay::GameplayBuildingSelection;
use crate::units::input::cursor_world_ray;
use crate::world::authoring_transform::{
    AUTHORING_INSTANCE_SCALE_MAX, AUTHORING_INSTANCE_SCALE_MIN,
};
use crate::world::{
    BuildingTransformSafetyClass, DoodadCatalog, FootprintCatalog, InteriorProfileCatalog,
    UnitCatalog, WorldConfig, WorldData,
};

use super::commit::{
    dev_gizmo_building_commit_options, dev_gizmo_doodad_commit_options,
    preview_differs_from_authoritative, try_commit_edit,
};
use super::drag::apply_drag;
use super::handles::policy_for_target;
use super::math::apparent_gizmo_scale;
use super::pick::{gizmo_has_priority, pick_gizmo_handle};
use super::state::{
    DoodadPreviewPlacement, GizmoAxisConstraint, TransformEditState,
    building_preview_from_placement, building_uniform_scale_from_preview,
};
use super::tool::{DevTool, DevToolState, SelectedWorldObject};
use crate::dev::hotkeys::{DevShortcutSuppressionCtx, dev_shortcuts_suppressed};
use crate::dev::navigation_editor::navigation_editor_owns_session;
use crate::dev::selected_object::SelectedObjectUiState;

const GIZMO_CAMERA_FOV_Y: f32 = std::f32::consts::FRAC_PI_4;

#[derive(SystemParam)]
pub struct GizmoInputParams<'w, 's> {
    pub dev_state: ResMut<'w, DevModeState>,
    pub tool_state: ResMut<'w, DevToolState>,
    pub edit: ResMut<'w, TransformEditState>,
    pub world_selection: ResMut<'w, WorldSelectionState>,
    pub panel_hovered: Res<'w, DevPanelHoverState>,
    pub gate: ResMut<'w, DevModeInputGate>,
    pub keyboard: Res<'w, ButtonInput<KeyCode>>,
    pub mouse_buttons: Res<'w, ButtonInput<MouseButton>>,
    pub windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    pub camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<RtsCamera>>,
    pub world: ResMut<'w, WorldData>,
    pub config: Res<'w, WorldConfig>,
    pub doodad_catalog: Res<'w, DoodadCatalog>,
    pub building_catalog: Res<'w, crate::world::BuildingCatalog>,
    pub footprint_catalog: Res<'w, FootprintCatalog>,
    pub interior_catalog: Res<'w, InteriorProfileCatalog>,
    pub unit_catalog: Res<'w, UnitCatalog>,
    pub nav_catalog: Res<'w, crate::world::BuildingNavigationBlueprintCatalog>,
    pub render_index: Res<'w, DoodadRenderIndex>,
    pub render_assets: Option<Res<'w, crate::terrain::TerrainRenderAssets>>,
    pub preview: ResMut<'w, crate::dev::tools::DevPlacementPreview>,
    pub building_selection: ResMut<'w, GameplayBuildingSelection>,
    pub selected_units: ResMut<'w, crate::units::input::SelectedUnits>,
    pub selection_revision: ResMut<'w, WorldSelectionRevision>,
    pub inspector: ResMut<'w, crate::dev::inspector::WorldInspectorState>,
    pub assessment_store: ResMut<'w, crate::world::BuildingTerrainAssessmentStore>,
    pub blueprint_inspection: ResMut<'w, crate::dev::BlueprintInspectionState>,
    pub selected_object_ui: ResMut<'w, SelectedObjectUiState>,
    pub window_registry: Res<'w, crate::dev::window::DevWindowRegistry>,
}

pub fn selected_object(world_selection: &WorldSelectionState) -> Option<SelectedWorldObject> {
    world_selection
        .transform_doodad()
        .map(SelectedWorldObject::Doodad)
        .or(world_selection
            .transform_building()
            .map(SelectedWorldObject::Building))
}

/// Tear down transform gizmo session when dev mode is off.
///
/// Does not mutate [`WorldSelectionState`]: gameplay building/doodad selection is independent
/// of dev transform tooling and must survive across frames while dev mode stays disabled.
pub(crate) fn reset_gizmo_when_dev_disabled(
    edit: &mut TransformEditState,
    tool_state: &mut DevToolState,
) {
    edit.full_cancel();
    tool_state.active_tool = DevTool::Select;
}

pub fn sync_gizmo_target(mut params: GizmoInputParams) {
    let prev_target = params.edit.target;
    let target = selected_object(&params.world_selection);

    if let Some(prev) = prev_target {
        if Some(prev) != target && !params.edit.dragging {
            if let Some(preview) = params.edit.preview_placement {
                if preview_differs_from_authoritative(&params.world, prev, preview) {
                    let doodad_options = dev_gizmo_doodad_commit_options();
                    let building_options = dev_gizmo_building_commit_options();
                    let committed = try_commit_edit(
                        &mut params.edit,
                        &mut params.world,
                        &params.doodad_catalog,
                        &params.building_catalog,
                        &params.footprint_catalog,
                        &params.interior_catalog,
                        &params.unit_catalog,
                        Some(&params.nav_catalog),
                        doodad_options,
                        building_options,
                        Some(&mut params.assessment_store),
                    );
                    if committed {
                        params.inspector.last_message = format!("Gizmo commit: {:?}", prev);
                    } else if !params.edit.last_error.is_empty() {
                        params.inspector.last_message = params.edit.last_error.clone();
                    }
                }
            }
        }
    }

    if !params.dev_state.enabled {
        reset_gizmo_when_dev_disabled(&mut params.edit, &mut params.tool_state);
        return;
    }

    if params.dev_state.clear_world_selection_for_place {
        params.dev_state.clear_world_selection_for_place = false;
        let mut apply_params = ApplyWorldSelectionParams {
            world_selection: &mut params.world_selection,
            selected_units: &mut params.selected_units,
            hud: None,
            revision: Some(&mut params.selection_revision),
        };
        apply_world_selection(WorldSelectionChange::ClearWorldObject, &mut apply_params);
        params.inspector.invalidate_for_selection_change();
        params.edit.clear();
    }

    if params.dev_state.placement_tool_active() {
        // Placement is armed: suppress transform gizmos only. Do not wipe world
        // selection every frame — that fought click-to-inspect (select → sync cleared it).
        params.tool_state.active_tool = DevTool::Place;
        if !params.edit.dragging {
            params.edit.clear();
        }
        return;
    }

    if navigation_editor_owns_session(
        params.dev_state.enabled,
        &params.window_registry,
        &params.blueprint_inspection,
    ) {
        if params.edit.dragging {
            params.edit.cancel_drag();
        }
        params.edit.full_cancel();
        params.tool_state.active_tool = DevTool::Select;
        return;
    }

    let prev_target = params.edit.target;
    let target = selected_object(&params.world_selection);
    let authoritative = target.and_then(|t| match t {
        SelectedWorldObject::Doodad(id) => params
            .world
            .get_doodad(id)
            .map(|r| DoodadPreviewPlacement::from_placement(r.placement)),
        SelectedWorldObject::Building(id) => params
            .world
            .get_building(id)
            .map(|r| building_preview_from_placement(r.placement)),
        _ => None,
    });

    let tool = if params.tool_state.active_tool == DevTool::Place {
        DevTool::Select
    } else {
        params.tool_state.active_tool
    };

    params
        .edit
        .sync_target_from_selection(target, tool, authoritative);

    if let Some(selected) = target {
        let policy = policy_for_target(selected, &params.building_catalog, &params.world);
        if policy.capabilities != crate::world::TransformCapabilities::NONE
            && !params.edit.dragging
            && (target != prev_target || !params.edit.mode.is_transform())
        {
            params.tool_state.active_tool = DevTool::Translate;
            params.edit.mode = DevTool::Translate;
            params.edit.target = Some(selected);
            if let Some(placement) = authoritative {
                params.edit.preview_placement = Some(placement);
            }
        }
    }

    if target.is_none() && !params.edit.dragging {
        params.edit.clear();
        if params.tool_state.active_tool.is_transform() {
            params.tool_state.active_tool = DevTool::Select;
        }
    }
}

pub fn handle_gizmo_keyboard(
    mut params: GizmoInputParams,
    menu_block: Option<Res<crate::menu::MenuInputBlock>>,
) {
    if menu_block.is_some_and(|block| block.blocks()) {
        return;
    }
    if !params.dev_state.enabled {
        return;
    }
    let suppression = DevShortcutSuppressionCtx::new(
        &params.dev_state,
        &params.selected_object_ui,
        &params.blueprint_inspection,
    );
    if dev_shortcuts_suppressed(suppression) {
        return;
    }
    if navigation_editor_owns_session(
        params.dev_state.enabled,
        &params.window_registry,
        &params.blueprint_inspection,
    ) {
        return;
    }

    let transform_context = selected_object(&params.world_selection).is_some();

    if transform_context {
        if params.keyboard.just_pressed(KeyCode::Comma) {
            enter_transform_tool(&mut params, DevTool::Translate);
            return;
        }
        if params.keyboard.just_pressed(KeyCode::Period) {
            enter_transform_tool(&mut params, DevTool::Rotate);
            return;
        }
        if params.keyboard.just_pressed(KeyCode::Slash) {
            enter_transform_tool(&mut params, DevTool::Scale);
            return;
        }
    }

    if !params.edit.dragging {
        return;
    }

    if params.keyboard.just_pressed(KeyCode::KeyX) {
        params.edit.axis_constraint = Some(GizmoAxisConstraint::X);
    }
    if params.keyboard.just_pressed(KeyCode::KeyY) {
        params.edit.axis_constraint = Some(GizmoAxisConstraint::Y);
    }
    if params.keyboard.just_pressed(KeyCode::KeyZ) {
        params.edit.axis_constraint = Some(GizmoAxisConstraint::Z);
    }
}

/// Activate a transform tool for the current world selection (buttons and hotkeys share this path).
pub fn activate_dev_transform_tool(params: &mut GizmoInputParams, tool: DevTool) {
    enter_transform_tool(params, tool);
}

fn enter_transform_tool(params: &mut GizmoInputParams, tool: DevTool) {
    if navigation_editor_owns_session(
        params.dev_state.enabled,
        &params.window_registry,
        &params.blueprint_inspection,
    ) {
        return;
    }
    let Some(target) = selected_object(&params.world_selection) else {
        return;
    };
    let policy = policy_for_target(target, &params.building_catalog, &params.world);
    if policy.capabilities == crate::world::TransformCapabilities::NONE {
        return;
    }
    cancel_dev_placement(&mut params.dev_state, &mut params.preview);
    params.tool_state.active_tool = tool;
    params.edit.mode = tool;
    params.edit.target = Some(target);
    if let SelectedWorldObject::Doodad(id) = target {
        if let Some(record) = params.world.get_doodad(id) {
            params.edit.preview_placement =
                Some(DoodadPreviewPlacement::from_placement(record.placement));
        }
    }
    if let SelectedWorldObject::Building(id) = target {
        if let Some(record) = params.world.get_building(id) {
            params.edit.preview_placement = Some(building_preview_from_placement(record.placement));
        }
    }
}

pub fn handle_gizmo_mouse(
    mut params: GizmoInputParams,
    menu_block: Option<Res<crate::menu::MenuInputBlock>>,
) {
    if menu_block.is_some_and(|block| block.blocks()) {
        return;
    }
    if !params.dev_state.enabled || params.panel_hovered.hovered {
        return;
    }
    if navigation_editor_owns_session(
        params.dev_state.enabled,
        &params.window_registry,
        &params.blueprint_inspection,
    ) {
        return;
    }
    if params.dev_state.text_focus != crate::dev::DevTextFieldFocus::None {
        return;
    }

    let Some(target) = params
        .edit
        .target
        .or_else(|| selected_object(&params.world_selection))
    else {
        return;
    };
    if !params.edit.mode.is_transform() {
        return;
    }

    let policy = policy_for_target(target, &params.building_catalog, &params.world);
    if policy.capabilities == crate::world::TransformCapabilities::NONE {
        return;
    }

    let Some(ray) = cursor_world_ray(&params.windows, &params.camera) else {
        return;
    };

    let (anchor, rotation, min_scale, max_scale) = match target {
        SelectedWorldObject::Doodad(id) => doodad_drag_context(
            &params.world,
            &params.config,
            &params.doodad_catalog,
            &params.edit,
            id,
            &params.render_assets,
        ),
        SelectedWorldObject::Building(id) => building_drag_context(
            &params.world,
            &params.config,
            &params.building_catalog,
            &params.edit,
            id,
            &params.render_assets,
        ),
        SelectedWorldObject::ItemPile(_) => return,
    };
    let Some(anchor) = anchor else {
        if params.edit.dragging {
            params.edit.cancel_drag();
        }
        return;
    };

    let Ok(window) = params.windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = params.camera.single() else {
        return;
    };
    let Some(cursor) = crate::units::input::cursor_screen_position(&params.windows) else {
        return;
    };
    let gizmo_scale = apparent_gizmo_scale(
        camera_transform.translation(),
        anchor,
        GIZMO_CAMERA_FOV_Y,
        window.resolution.height(),
    );

    let finer =
        params.keyboard.pressed(KeyCode::ShiftLeft) || params.keyboard.pressed(KeyCode::ShiftRight);
    let layout = params.config.chunk_layout();
    let vertical_scale = params
        .render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    let yaw_snap_degrees = match target {
        SelectedWorldObject::Building(id) => params
            .world
            .get_building(id)
            .and_then(|record| params.building_catalog.get(&record.definition_id))
            .filter(|definition| {
                definition.transform_safety_class == BuildingTransformSafetyClass::Navigable
            })
            .map(|_| 90.0),
        _ => None,
    };
    let scale_view_dir = params.edit.drag_scale_view_dir.or_else(|| {
        let dir = (camera_transform.translation() - anchor).normalize_or_zero();
        (dir.length_squared() > 1e-6).then_some(dir)
    });

    if params.edit.dragging {
        params.gate.block_gameplay_mouse = true;
        params.gate.block_camera_input = true;
        params.gate.spawn_handled_this_frame = true;

        if params.mouse_buttons.pressed(MouseButton::Left) {
            let Some(handle) = params.edit.active_handle else {
                return;
            };
            let Some(start_ray) = params.edit.drag_start_ray else {
                return;
            };
            let Some(start) = params.edit.drag_start_placement else {
                return;
            };
            if let Some(preview) = apply_drag(
                handle,
                &start_ray,
                &ray,
                start,
                anchor,
                layout,
                vertical_scale,
                rotation,
                DEV_GIZMO_COORDINATE_SPACE,
                params.edit.snap,
                finer,
                params.edit.axis_constraint,
                min_scale,
                max_scale,
                yaw_snap_degrees,
                scale_view_dir,
            ) {
                params.edit.preview_placement = Some(preview);
                params.edit.preview_valid = true;
            }
        }

        if params.mouse_buttons.just_released(MouseButton::Left) {
            let doodad_options = dev_gizmo_doodad_commit_options();
            let building_options = dev_gizmo_building_commit_options();
            let committed = try_commit_edit(
                &mut params.edit,
                &mut params.world,
                &params.doodad_catalog,
                &params.building_catalog,
                &params.footprint_catalog,
                &params.interior_catalog,
                &params.unit_catalog,
                Some(&params.nav_catalog),
                doodad_options,
                building_options,
                Some(&mut params.assessment_store),
            );
            params.edit.end_drag();
            if committed {
                match target {
                    SelectedWorldObject::Doodad(id) => {
                        if let Some(record) = params.world.get_doodad(id) {
                            params.edit.preview_placement =
                                Some(DoodadPreviewPlacement::from_placement(record.placement));
                        }
                        params.inspector.last_message =
                            format!("Gizmo commit: doodad #{}", id.raw());
                    }
                    SelectedWorldObject::Building(id) => {
                        if let Some(record) = params.world.get_building(id) {
                            params.edit.preview_placement =
                                Some(building_preview_from_placement(record.placement));
                        }
                        params.inspector.last_message =
                            format!("Gizmo commit: building #{}", id.raw());
                    }
                    SelectedWorldObject::ItemPile(_) => {}
                }
            } else if !params.edit.last_error.is_empty() {
                params.inspector.last_message = params.edit.last_error.clone();
                if let Some(start) = params.edit.drag_start_placement {
                    params.edit.preview_placement = Some(start);
                }
            }
        }

        return;
    }

    if !gizmo_has_priority(&params.edit, true) {
        return;
    }

    params.edit.hovered_handle = pick_gizmo_handle(
        camera,
        camera_transform,
        cursor,
        anchor,
        rotation,
        gizmo_scale,
        params.edit.mode,
        policy.capabilities,
        DEV_GIZMO_COORDINATE_SPACE,
    );

    if params.edit.hovered_handle.is_some() {
        params.gate.block_gameplay_mouse = true;
    }

    if params.mouse_buttons.just_pressed(MouseButton::Left) {
        let Some(handle) = params.edit.hovered_handle else {
            return;
        };
        let Some(start) = params.edit.preview_placement else {
            return;
        };
        params.gate.block_gameplay_mouse = true;
        params.gate.spawn_handled_this_frame = true;
        let scale_view_dir = (camera_transform.translation() - anchor).normalize_or_zero();
        params.edit.begin_drag(handle, ray, start, scale_view_dir);
    }
}

/// Placement used to anchor the gizmo drag math.
///
/// While dragging, the anchor must be the fixed grab placement (`drag_start_placement`).
/// Using the live `preview_placement` (which is rewritten every frame from the previous
/// drag result) feeds the moving anchor back into the delta computation, so the object
/// accelerates away from the cursor. When not dragging we track the live preview so the
/// gizmo follows the object.
fn drag_anchor_placement(edit: &TransformEditState) -> Option<DoodadPreviewPlacement> {
    if edit.dragging {
        edit.drag_start_placement.or(edit.preview_placement)
    } else {
        edit.preview_placement
    }
}

fn doodad_drag_context(
    world: &WorldData,
    config: &WorldConfig,
    _catalog: &DoodadCatalog,
    edit: &TransformEditState,
    id: crate::world::DoodadId,
    render_assets: &Option<Res<crate::terrain::TerrainRenderAssets>>,
) -> (Option<Vec3>, Quat, f32, f32) {
    let Some(record) = world.get_doodad(id) else {
        return (None, Quat::IDENTITY, 0.05, 20.0);
    };
    let min_scale = AUTHORING_INSTANCE_SCALE_MIN;
    let max_scale = AUTHORING_INSTANCE_SCALE_MAX;
    let placement = drag_anchor_placement(edit)
        .unwrap_or_else(|| DoodadPreviewPlacement::from_placement(record.placement));
    let vertical_scale = render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    let anchor =
        world_position_to_render_global(placement.position, config.chunk_layout(), vertical_scale);
    (
        Some(anchor),
        placement.rotation_quat(),
        min_scale,
        max_scale,
    )
}

fn building_drag_context(
    world: &WorldData,
    config: &WorldConfig,
    _catalog: &crate::world::BuildingCatalog,
    edit: &TransformEditState,
    id: crate::world::BuildingId,
    render_assets: &Option<Res<crate::terrain::TerrainRenderAssets>>,
) -> (Option<Vec3>, Quat, f32, f32) {
    let Some(record) = world.get_building(id) else {
        return (None, Quat::IDENTITY, 0.05, 20.0);
    };
    let min_scale = AUTHORING_INSTANCE_SCALE_MIN;
    let max_scale = AUTHORING_INSTANCE_SCALE_MAX;
    let placement = drag_anchor_placement(edit)
        .map(|preview| {
            (
                preview.position,
                preview.orientation.to_quat(),
                building_uniform_scale_from_preview(preview).to_f32(),
            )
        })
        .unwrap_or_else(|| {
            (
                record.placement.position,
                record.placement.rotation,
                record.placement.uniform_scale_f32(),
            )
        });
    let vertical_scale = render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    let anchor =
        world_position_to_render_global(placement.0, config.chunk_layout(), vertical_scale);
    (Some(anchor), placement.1, min_scale, max_scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::selection::{
        ApplyWorldSelectionParams, WorldSelectionCategory, WorldSelectionChange,
        WorldSelectionRevision, WorldSelectionState, apply_world_selection, prune_world_selection,
    };
    use crate::units::input::SelectedUnits;
    use crate::world::{
        BuildingDefinitionId, BuildingId, BuildingOwnership, BuildingPlacement, BuildingRecord,
        BuildingSource, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition,
        WorldData, WorldPosition,
    };
    use bevy::prelude::{Quat, Vec3};

    #[test]
    fn selected_object_returns_doodad_when_category_doodad() {
        let world_selection = WorldSelectionState {
            category: crate::client::selection::WorldSelectionCategory::Doodad,
            doodad_id: Some(crate::world::DoodadId::new(1)),
            ..Default::default()
        };
        assert!(matches!(
            selected_object(&world_selection),
            Some(SelectedWorldObject::Doodad(_))
        ));
    }

    #[test]
    fn dev_disabled_gizmo_reset_preserves_gameplay_building_selection() {
        let mut world_selection = WorldSelectionState {
            category: WorldSelectionCategory::Building,
            building_id: Some(BuildingId::new(1)),
            ..Default::default()
        };
        let mut edit = TransformEditState::default();
        let mut tool_state = DevToolState::default();

        reset_gizmo_when_dev_disabled(&mut edit, &mut tool_state);

        assert_eq!(world_selection.category, WorldSelectionCategory::Building);
        assert_eq!(world_selection.building_id, Some(BuildingId::new(1)));
        assert_eq!(tool_state.active_tool, DevTool::Select);
        assert!(!edit.is_transform_session_active());
    }

    fn flat_world_with_farm(building_id: BuildingId) -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        let record = BuildingRecord::new(
            building_id,
            BuildingDefinitionId::new("prispod_farm"),
            BuildingPlacement::new(
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(10.0, 0.0, 10.0)),
                ),
                Quat::IDENTITY,
            ),
            BuildingOwnership::default(),
            300,
            BuildingSource::Authored,
        );
        let chunk = ChunkId::new(record.placement.position.chunk);
        world.insert_building(chunk, record).unwrap();
        world
    }

    fn apply_params<'a>(
        world_selection: &'a mut WorldSelectionState,
        selected_units: &'a mut SelectedUnits,
        revision: &'a mut WorldSelectionRevision,
    ) -> ApplyWorldSelectionParams<'a> {
        ApplyWorldSelectionParams {
            world_selection,
            selected_units,
            hud: None,
            revision: Some(revision),
        }
    }

    /// Regression: gameplay SelectBuilding must survive the next-frame dev gizmo sync and prune.
    #[test]
    fn gameplay_building_selection_survives_post_dispatch_dev_frame() {
        let building_id = BuildingId::new(7);
        let world = flat_world_with_farm(building_id);
        let mut world_selection = WorldSelectionState::default();
        let mut selected_units = SelectedUnits::default();
        let mut revision = WorldSelectionRevision::default();

        apply_world_selection(
            WorldSelectionChange::SelectBuilding { building_id },
            &mut apply_params(&mut world_selection, &mut selected_units, &mut revision),
        );
        assert_eq!(world_selection.category, WorldSelectionCategory::Building);

        let mut edit = TransformEditState::default();
        let mut tool_state = DevToolState::default();
        reset_gizmo_when_dev_disabled(&mut edit, &mut tool_state);

        prune_world_selection(
            &world,
            &mut apply_params(&mut world_selection, &mut selected_units, &mut revision),
        );

        assert_eq!(world_selection.category, WorldSelectionCategory::Building);
        assert_eq!(world_selection.building_id, Some(building_id));
    }
}
