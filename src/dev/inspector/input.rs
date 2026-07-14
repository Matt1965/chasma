//! Inspector input and snapshot refresh (ADR-048).

use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::buildings::components::BuildingRenderEntity;
use crate::buildings::picking::pick_building_along_ray;
use crate::camera::RtsCamera;
use crate::dev::{DevModeInputGate, DevModeState, DevPanelHoverState};
use crate::terrain::TerrainRenderAssets;
use crate::ui::gameplay::GameplayBuildingSelection;
use crate::units::UnitRenderEntity;
use crate::units::input::{
    BoxSelectDrag, cursor_world_ray, pick_unit_along_ray, terrain_click_to_world_position,
};

use super::capture::{
    capture_building_inspector_snapshot, capture_interaction_inspector_snapshot,
    capture_unit_inspector_snapshot,
};
use super::params::InspectorCaptureParams;
use super::state::{InspectorCacheKey, WorldInspectorState};
use crate::debug::InspectorOverlayFocus;

/// Refresh cached inspector snapshots when selection changes or simulation pauses.
pub fn refresh_inspector_snapshot(
    capture: InspectorCaptureParams,
    mut inspector: ResMut<WorldInspectorState>,
    mut overlay_focus: ResMut<InspectorOverlayFocus>,
) {
    let Some(unit_id) = inspector.selected_unit else {
        inspector.unit_snapshot = None;
        overlay_focus.set_unit(None);
        return;
    };

    let paused = capture.simulation.paused;
    let selection_changed = inspector.cache_key.unit_id != Some(unit_id);
    let pause_edge = paused && !inspector.cache_key.paused;

    if !selection_changed && !pause_edge && inspector.unit_snapshot.is_some() {
        return;
    }

    let Some(snapshot) = capture_unit_inspector_snapshot(
        &capture.world,
        &capture.unit_catalog,
        &capture.weapon_catalog,
        &capture.doodad_catalog,
        &capture.building_catalog,
        &capture.footprint_catalog,
        unit_id,
        capture.simulation.current_tick,
        capture.movement_blocks.last_for_unit(unit_id),
    ) else {
        inspector.clear();
        overlay_focus.set_unit(None);
        return;
    };

    overlay_focus.path_waypoint_index = Some(snapshot.path.waypoint_index);
    inspector.unit_snapshot = Some(snapshot);
    inspector.cache_key = InspectorCacheKey {
        unit_id: Some(unit_id),
        building_id: None,
        simulation_tick: capture.simulation.current_tick,
        paused,
    };
    overlay_focus.set_unit(Some(unit_id));
}

/// Pick units / probe terrain for inspector (dev mode or Alt modifier).
pub fn handle_inspector_input(
    dev_state: Res<DevModeState>,
    panel_hovered: Res<DevPanelHoverState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut gate: ResMut<DevModeInputGate>,
    box_drag: Res<BoxSelectDrag>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    units: Query<(&UnitRenderEntity, &GlobalTransform)>,
    buildings: Query<(&BuildingRenderEntity, &GlobalTransform)>,
    capture: InspectorCaptureParams,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut inspector: ResMut<WorldInspectorState>,
    mut overlay_focus: ResMut<InspectorOverlayFocus>,
    mut building_selection: ResMut<GameplayBuildingSelection>,
) {
    let alt = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
    if !dev_state.enabled && !alt {
        return;
    }

    if panel_hovered.hovered || gate.spawn_handled_this_frame {
        return;
    }

    if !mouse_buttons.just_pressed(MouseButton::Left) || box_drag.is_box_drag() {
        return;
    }

    let Some(ray) = cursor_world_ray(&windows, &camera) else {
        return;
    };

    if let Some(unit_id) = pick_unit_along_ray(
        &ray,
        &capture.world,
        &capture.unit_catalog,
        &units,
        crate::world::SelectionControllabilityPolicy::dev_inspect(),
    ) {
        gate.block_gameplay_mouse = true;
        inspector.select_unit(unit_id);
        building_selection.set(None);
        inspector.last_message = format!("Inspecting unit #{}", unit_id.raw());
        if let Some(snapshot) = capture_unit_inspector_snapshot(
            &capture.world,
            &capture.unit_catalog,
            &capture.weapon_catalog,
            &capture.doodad_catalog,
            &capture.building_catalog,
            &capture.footprint_catalog,
            unit_id,
            capture.simulation.current_tick,
            capture.movement_blocks.last_for_unit(unit_id),
        ) {
            overlay_focus.path_waypoint_index = Some(snapshot.path.waypoint_index);
            inspector.unit_snapshot = Some(snapshot);
            inspector.cache_key = InspectorCacheKey {
                unit_id: Some(unit_id),
                building_id: None,
                simulation_tick: capture.simulation.current_tick,
                paused: capture.simulation.paused,
            };
        }
        overlay_focus.set_unit(Some(unit_id));
        return;
    }

    if let Some(building_id) =
        pick_building_along_ray(&ray, &capture.world, &capture.building_catalog, &buildings)
    {
        gate.block_gameplay_mouse = true;
        inspector.select_building(building_id);
        building_selection.set(Some(building_id));
        inspector.last_message = format!("Inspecting building #{}", building_id.raw());
        inspector.building_snapshot = capture_building_inspector_snapshot(
            &capture.world,
            &capture.building_catalog,
            &crate::world::BuildingInteractionProfileCatalog::default(),
            building_id,
        );
        overlay_focus.set_unit(None);
        return;
    }

    if !dev_state.enabled {
        return;
    }

    let layout = capture.config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    if let Some(click) =
        terrain_click_to_world_position(&ray, &capture.world, layout, vertical_scale)
    {
        gate.block_gameplay_mouse = true;
        inspector.interaction_snapshot = capture_interaction_inspector_snapshot(
            &capture.world,
            &capture.unit_catalog,
            &capture.doodad_catalog,
            &capture.building_catalog,
            &capture.footprint_catalog,
            &capture.weapon_catalog,
            click.world_position,
        );
        inspector.last_message = "Interaction probe at terrain click".into();
    }
}

/// Marker for inspector UI nodes.
#[derive(Component, Debug)]
pub struct DevInspectorUi;
