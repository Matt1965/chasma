//! Inspector input and snapshot refresh (ADR-048).

use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;

use crate::buildings::picking::pick_building_along_ray;
use crate::client::selection::{
    WorldSelectionCategory, WorldSelectionChange, WorldSelectionRevision, WorldSelectionState,
    WorldSelectionWriteParams, apply_world_selection,
};
use crate::dev::gizmo::TransformEditState;
use crate::dev::{
    DevModeInputGate, DevModeState, DevPanelHoverState, DevPlacementPreview, cancel_dev_placement,
};
use crate::doodads::picking::pick_doodad_along_ray;
use crate::terrain::TerrainRenderAssets;
use crate::units::input::{
    BoxSelectDrag, SelectedUnits, cursor_world_ray, pick_unit_along_ray,
    terrain_click_to_world_position,
};

use super::BlueprintInspectionState;
use super::capture::{
    capture_building_asset_presentation, capture_building_blueprint_inspection_snapshot,
    capture_building_inspector_snapshot, capture_interaction_inspector_snapshot,
    capture_unit_inspector_snapshot, probe_building_operation,
};
use super::params::{
    BuildingInspectorPresentationParams, InspectorCaptureParams, InspectorPickParams,
};
use super::snapshot::capture_doodad_inspector_snapshot;
use super::state::{InspectorCacheKey, WorldInspectorState};
use crate::debug::InspectorOverlayFocus;
use crate::world::InventoryCatalogCtx;

/// Invalidate inspector snapshots when shared selection revision changes.
pub fn sync_inspector_on_selection_revision(
    revision: Res<WorldSelectionRevision>,
    mut tracked: Local<u64>,
    mut inspector: ResMut<WorldInspectorState>,
) {
    if *tracked == revision.0 {
        return;
    }
    *tracked = revision.0;
    inspector.invalidate_for_selection_change();
}

/// Refresh cached inspector snapshots when selection changes or simulation pauses.
pub fn refresh_inspector_snapshot(
    world_selection: Res<WorldSelectionState>,
    selected_units: Res<SelectedUnits>,
    blueprint_inspection: Res<BlueprintInspectionState>,
    mut capture: InspectorCaptureParams,
    presentation: BuildingInspectorPresentationParams,
    mut inspector: ResMut<WorldInspectorState>,
    mut overlay_focus: ResMut<InspectorOverlayFocus>,
) {
    match world_selection.category {
        WorldSelectionCategory::Units => {
            if let Some(unit_id) = world_selection.primary_unit(&selected_units) {
                refresh_unit_snapshot(&capture, &mut inspector, &mut overlay_focus, unit_id);
            } else {
                inspector.unit_snapshot = None;
                overlay_focus.set_unit(None);
            }
        }
        WorldSelectionCategory::Building => {
            if let Some(building_id) = world_selection.building_id {
                refresh_building_snapshot(
                    &mut capture,
                    &presentation,
                    &blueprint_inspection,
                    &mut inspector,
                    &mut overlay_focus,
                    building_id,
                );
            }
        }
        WorldSelectionCategory::Doodad => {
            if let Some(doodad_id) = world_selection.doodad_id {
                refresh_doodad_snapshot(&capture, &mut inspector, &mut overlay_focus, doodad_id);
            }
        }
        WorldSelectionCategory::ItemPile => {
            if let Some(pile_id) = world_selection.pile_id {
                refresh_pile_snapshot(&capture, &mut inspector, pile_id);
            } else {
                inspector.pile_snapshot = None;
            }
            overlay_focus.set_unit(None);
        }
        WorldSelectionCategory::None => {
            inspector.unit_snapshot = None;
            inspector.building_snapshot = None;
            inspector.blueprint_snapshot = None;
            inspector.doodad_snapshot = None;
            inspector.pile_snapshot = None;
            overlay_focus.set_unit(None);
        }
    }
}

fn refresh_pile_snapshot(
    capture: &InspectorCaptureParams,
    inspector: &mut WorldInspectorState,
    pile_id: crate::world::ItemPileId,
) {
    let key = InspectorCacheKey {
        category: WorldSelectionCategory::ItemPile,
        unit_id: None,
        building_id: None,
        doodad_id: None,
        pile_id: Some(pile_id),
        simulation_tick: capture.simulation.current_tick,
        paused: capture.simulation.paused,
    };
    if inspector.cache_key == key && inspector.pile_snapshot.is_some() {
        return;
    }
    let Some(snapshot) = super::capture::capture_item_pile_inspector_snapshot(
        &capture.world,
        &capture.items,
        pile_id,
    ) else {
        inspector.clear();
        inspector.last_message = "Selected pile no longer exists".into();
        return;
    };
    inspector.unit_snapshot = None;
    inspector.building_snapshot = None;
    inspector.blueprint_snapshot = None;
    inspector.doodad_snapshot = None;
    inspector.pile_snapshot = Some(snapshot);
    inspector.cache_key = key;
}

fn refresh_unit_snapshot(
    capture: &InspectorCaptureParams,
    inspector: &mut WorldInspectorState,
    overlay_focus: &mut InspectorOverlayFocus,
    unit_id: crate::world::UnitId,
) {
    let paused = capture.simulation.paused;
    let key = InspectorCacheKey {
        category: WorldSelectionCategory::Units,
        unit_id: Some(unit_id),
        building_id: None,
        doodad_id: None,
        pile_id: None,
        simulation_tick: capture.simulation.current_tick,
        paused,
    };
    let selection_changed = inspector.cache_key != key;
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

fn refresh_building_snapshot(
    capture: &mut InspectorCaptureParams,
    presentation: &BuildingInspectorPresentationParams,
    blueprint_inspection: &BlueprintInspectionState,
    inspector: &mut WorldInspectorState,
    overlay_focus: &mut InspectorOverlayFocus,
    building_id: crate::world::BuildingId,
) {
    let paused = capture.simulation.paused;
    let key = InspectorCacheKey {
        category: WorldSelectionCategory::Building,
        unit_id: None,
        building_id: Some(building_id),
        doodad_id: None,
        pile_id: None,
        simulation_tick: capture.simulation.current_tick,
        paused,
    };
    if !inspector.needs_refresh(key) {
        return;
    }

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
    // Navigation Editor owns blueprint snapshots while inspection/edit is active.
    if !blueprint_inspection.active {
        inspector.blueprint_snapshot = capture_building_blueprint_inspection_snapshot(
            &capture.world,
            &capture.building_catalog,
            &capture.nav_blueprint_catalog,
            building_id,
            blueprint_inspection.selected_floor_id,
        );
    }
    inspector.unit_snapshot = None;
    inspector.doodad_snapshot = None;
    inspector.cache_key = key;
    overlay_focus.set_unit(None);
}

fn refresh_doodad_snapshot(
    capture: &InspectorCaptureParams,
    inspector: &mut WorldInspectorState,
    overlay_focus: &mut InspectorOverlayFocus,
    doodad_id: crate::world::DoodadId,
) {
    let paused = capture.simulation.paused;
    let key = InspectorCacheKey {
        category: WorldSelectionCategory::Doodad,
        unit_id: None,
        building_id: None,
        doodad_id: Some(doodad_id),
        pile_id: None,
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
        return;
    };

    inspector.doodad_snapshot = Some(snapshot);
    inspector.building_snapshot = None;
    inspector.blueprint_snapshot = None;
    inspector.unit_snapshot = None;
    inspector.cache_key = key;
    overlay_focus.set_unit(None);
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
    blueprint_inspection: Res<BlueprintInspectionState>,
    pick: InspectorPickParams,
    mut capture: InspectorCaptureParams,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut selection_params: WorldSelectionWriteParams,
    mut inspector: ResMut<WorldInspectorState>,
) {
    let alt = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
    if !dev_state.enabled && !alt {
        return;
    }

    if panel_hovered.hovered || gate.spawn_handled_this_frame || gizmo_edit.dragging {
        return;
    }
    // Live edit session owns world clicks (snapshot.edit_active can lag one frame).
    if blueprint_inspection.editing {
        return;
    }
    if inspector
        .blueprint_snapshot
        .as_ref()
        .is_some_and(|snap| snap.edit_active)
    {
        return;
    }

    if !mouse_buttons.just_pressed(MouseButton::Left) || box_drag.is_box_drag() {
        return;
    }

    let Some(ray) = cursor_world_ray(&pick.windows, &pick.camera) else {
        return;
    };

    let mut apply_params = selection_params.apply(None);

    if let Some(unit_id) = pick_unit_along_ray(
        &ray,
        &capture.world,
        &capture.unit_catalog,
        &pick.units,
        crate::world::SelectionControllabilityPolicy::dev_inspect(),
    ) {
        gate.block_gameplay_mouse = true;
        apply_world_selection(
            WorldSelectionChange::SelectUnit { unit_id },
            &mut apply_params,
        );
        inspector.last_message = format!("Inspecting unit #{}", unit_id.raw());
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
            apply_world_selection(
                WorldSelectionChange::SelectDoodad { doodad_id },
                &mut apply_params,
            );
            inspector.last_message = format!("Inspecting doodad #{}", doodad_id.raw());
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
        apply_world_selection(
            WorldSelectionChange::SelectBuilding { building_id },
            &mut apply_params,
        );
        inspector.last_message = format!("Inspecting building #{}", building_id.raw());
        return;
    }

    if !dev_state.enabled {
        return;
    }

    if dev_state.inventory.pile_placement_armed {
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
            &capture.pile_settings,
        ) {
            apply_world_selection(
                WorldSelectionChange::SelectItemPile { pile_id },
                &mut apply_params,
            );
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

/// Marker for production repeat-mode control in Selected Object.
#[derive(Component, Debug)]
pub struct BuildingProductionRepeatModeButton;

#[derive(Component, Debug)]
pub struct BuildingProductionRepeatModeButtonText;
