//! Inspector input and snapshot refresh (ADR-048).

use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;

use crate::buildings::picking::pick_building_along_ray;
use crate::dev::gizmo::TransformEditState;
use crate::dev::{
    DevModeInputGate, DevModeState, DevPanelHoverState, DevPlacementPreview, cancel_dev_placement,
};
use crate::doodads::picking::pick_doodad_along_ray;
use crate::terrain::TerrainRenderAssets;
use crate::ui::gameplay::GameplayBuildingSelection;
use crate::units::input::{
    BoxSelectDrag, cursor_world_ray, pick_unit_along_ray, terrain_click_to_world_position,
};

use super::capture::{
    capture_building_asset_presentation, capture_building_inspector_snapshot,
    capture_interaction_inspector_snapshot, capture_unit_inspector_snapshot,
    probe_building_operation,
};
use super::params::{
    BuildingInspectorPresentationParams, InspectorCaptureParams, InspectorPickParams,
};
use super::snapshot::capture_doodad_inspector_snapshot;
use crate::world::InventoryCatalogCtx;
use super::state::{InspectorCacheKey, WorldInspectorState};
use crate::debug::InspectorOverlayFocus;

/// Refresh cached inspector snapshots when selection changes or simulation pauses.
pub fn refresh_inspector_snapshot(
    capture: InspectorCaptureParams,
    mut inspector: ResMut<WorldInspectorState>,
    mut overlay_focus: ResMut<InspectorOverlayFocus>,
) {
    if let Some(unit_id) = inspector.selected_unit {
        refresh_unit_snapshot(&capture, &mut inspector, &mut overlay_focus, unit_id);
        return;
    }

    if let Some(doodad_id) = inspector.selected_doodad {
        refresh_doodad_snapshot(&capture, &mut inspector, doodad_id);
        overlay_focus.set_unit(None);
        return;
    }

    inspector.unit_snapshot = None;
    inspector.doodad_snapshot = None;
    overlay_focus.set_unit(None);
}

fn refresh_unit_snapshot(
    capture: &InspectorCaptureParams,
    inspector: &mut WorldInspectorState,
    overlay_focus: &mut InspectorOverlayFocus,
    unit_id: crate::world::UnitId,
) {
    let paused = capture.simulation.paused;
    let key = InspectorCacheKey {
        unit_id: Some(unit_id),
        building_id: None,
        doodad_id: None,
        simulation_tick: capture.simulation.current_tick,
        paused,
    };
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
    inspector.cache_key = key;
    overlay_focus.set_unit(Some(unit_id));
}

fn refresh_doodad_snapshot(
    capture: &InspectorCaptureParams,
    inspector: &mut WorldInspectorState,
    doodad_id: crate::world::DoodadId,
) {
    let paused = capture.simulation.paused;
    let key = InspectorCacheKey {
        unit_id: None,
        building_id: None,
        doodad_id: Some(doodad_id),
        simulation_tick: capture.simulation.current_tick,
        paused,
    };
    if !inspector.needs_refresh(key) {
        return;
    }

    let Some(snapshot) = capture_doodad_inspector_snapshot(
        &capture.world,
        &capture.doodad_catalog,
        &capture.footprint_catalog,
        doodad_id,
    ) else {
        // Keep selection even when the catalog row is missing — gizmos still arm from WorldData.
        return;
    };

    inspector.doodad_snapshot = Some(snapshot);
    inspector.cache_key = key;
}

/// Pick units / probe terrain for inspector (dev mode or Alt modifier).
pub fn handle_inspector_input(
    mut dev_state: ResMut<DevModeState>,
    mut placement_preview: ResMut<DevPlacementPreview>,
    panel_hovered: Res<DevPanelHoverState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut gate: ResMut<DevModeInputGate>,
    box_drag: Res<BoxSelectDrag>,
    gizmo_edit: Res<TransformEditState>,
    pick: InspectorPickParams,
    presentation: BuildingInspectorPresentationParams,
    pile_settings: Res<crate::world::ItemPileSettings>,
    mut capture: InspectorCaptureParams,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut inspector: ResMut<WorldInspectorState>,
    mut overlay_focus: ResMut<InspectorOverlayFocus>,
    mut building_selection: ResMut<GameplayBuildingSelection>,
) {
    let alt = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
    if !dev_state.enabled && !alt {
        return;
    }

    // The gizmo runs first: if it grabbed a handle this frame (or is mid-drag) it sets
    // `spawn_handled_this_frame`, so clicks that miss the gizmo still fall through here
    // and can re-select or deselect a world object.
    if panel_hovered.hovered || gate.spawn_handled_this_frame || gizmo_edit.dragging {
        return;
    }

    if !mouse_buttons.just_pressed(MouseButton::Left) || box_drag.is_box_drag() {
        return;
    }

    let Some(ray) = cursor_world_ray(&pick.windows, &pick.camera) else {
        return;
    };

    if let Some(unit_id) = pick_unit_along_ray(
        &ray,
        &capture.world,
        &capture.unit_catalog,
        &pick.units,
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
                doodad_id: None,
                simulation_tick: capture.simulation.current_tick,
                paused: capture.simulation.paused,
            };
        }
        overlay_focus.set_unit(Some(unit_id));
        return;
    }

    if dev_state.enabled {
        if let Some(doodad_id) = pick_doodad_along_ray(
            &ray,
            &capture.world,
            &capture.doodad_catalog,
            &capture.config,
            &render_assets,
            &pick.doodads,
        ) {
            gate.block_gameplay_mouse = true;
            cancel_dev_placement(&mut dev_state, &mut placement_preview);
            inspector.select_doodad(doodad_id);
            building_selection.set(None);
            inspector.last_message = format!("Inspecting doodad #{}", doodad_id.raw());
            inspector.doodad_snapshot = capture_doodad_inspector_snapshot(
                &capture.world,
                &capture.doodad_catalog,
                &capture.footprint_catalog,
                doodad_id,
            );
            inspector.cache_key = InspectorCacheKey {
                unit_id: None,
                building_id: None,
                doodad_id: Some(doodad_id),
                simulation_tick: capture.simulation.current_tick,
                paused: capture.simulation.paused,
            };
            overlay_focus.set_unit(None);
            return;
        }
    }

    if let Some(building_id) = pick_building_along_ray(
        &ray,
        &capture.world,
        &capture.building_catalog,
        &pick.buildings,
    ) {
        gate.block_gameplay_mouse = true;
        cancel_dev_placement(&mut dev_state, &mut placement_preview);
        inspector.select_building(building_id);
        building_selection.set(Some(building_id));
        inspector.last_message = format!("Inspecting building #{}", building_id.raw());
        let presentation_info = capture_building_asset_presentation(
            building_id,
            &capture.world,
            &capture.building_catalog,
            &presentation.asset_server,
            &presentation.scene_assets,
            &presentation.render_index,
            &presentation.render_entities,
        );
        let inventory_ctx = InventoryCatalogCtx::new(
            &capture.items,
            &capture.item_categories,
            &capture.inventory_profiles,
        );
        let mut operation = crate::world::BuildingOperationParams {
            field_catalog: &capture.field_catalog,
            requirement_catalog: &capture.requirements,
            profile_catalog: &capture.profile_catalog,
            footprint_catalog: &capture.footprint_catalog,
            operation_catalog: &capture.operation_catalog,
            inventory_ctx: &inventory_ctx,
            requirement_revision: capture.requirement_revision.0,
            profile_revision: capture.profile_revision.0,
            assessment_store: &mut capture.assessments,
        };
        let operation_probe = probe_building_operation(
            &capture.world,
            &capture.building_catalog,
            &mut operation,
            building_id,
        );
        inspector.building_snapshot = capture_building_inspector_snapshot(
            &capture.world,
            &capture.building_catalog,
            &crate::world::BuildingInteractionProfileCatalog::default(),
            building_id,
            Some(presentation_info),
            Some(operation_probe),
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
        if let Some(pile_id) = crate::dev::inventory_tools::nearest_pile_at_position(
            &capture.world,
            click.world_position,
            &pile_settings,
        ) {
            inspector.select_pile(pile_id);
            inspector.last_message = format!("Inspecting ground pile #{pile_id:?}");
            return;
        }
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
