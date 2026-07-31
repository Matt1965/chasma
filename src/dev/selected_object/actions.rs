//! Selected Object action routing (Slice 5).

use bevy::ecs::system::ParamSet;
use bevy::prelude::*;

use crate::client::selection::{WorldSelectionChange, apply_world_selection};
use crate::dev::gizmo::{DevTool, GizmoInputParams, activate_dev_transform_tool, selected_object};
use crate::dev::inspector::DevBuildingActionParams;
use crate::simulation::SimulationControlState;
use crate::world::{BuildingInventoryContext, OccupancyCatalogs, destroy_building, remove_doodad};

use super::panel::{
    DevSelectedObjectActionButton, DevSelectedObjectToggleButton, SelectedObjectAction,
    SelectedObjectToggle,
};
use super::state::{PendingDeleteTarget, SelectedObjectUiState};

use crate::debug::InspectorOverlayFocus;
use crate::dev::hotkeys::{
    cancel_blueprint_pending_confirmation, cancel_blueprint_variant_draft,
    exit_blueprint_inspection_from_ui, request_exit_blueprint_edit,
};
use crate::dev::inventory_tools::{DevInventoryEndpoint, dev_remove_entry};
use crate::dev::navigation_editor::navigation_editor_owns_session;

/// Handle Selected Object window buttons.
pub fn handle_selected_object_actions(
    mut gizmo: GizmoInputParams,
    mut overlay_focus: ResMut<InspectorOverlayFocus>,
    mut rts_camera: Query<&mut crate::camera::RtsCameraState, With<crate::camera::RtsCamera>>,
    action_params: DevBuildingActionParams,
    simulation: Res<SimulationControlState>,
    mut buttons: ParamSet<(
        Query<(&Interaction, &DevSelectedObjectActionButton), Changed<Interaction>>,
        Query<(&Interaction, &DevSelectedObjectToggleButton), Changed<Interaction>>,
    )>,
) {
    if !gizmo.dev_state.enabled {
        return;
    }

    for (interaction, toggle) in buttons.p1().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gizmo.gate.block_gameplay_mouse = true;
        match toggle.toggle {
            SelectedObjectToggle::Diagnostics => gizmo.selected_object_ui.toggle_diagnostics(),
        }
    }

    for (interaction, button) in buttons.p0().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gizmo.gate.block_gameplay_mouse = true;
        if matches!(
            button.action,
            SelectedObjectAction::Move | SelectedObjectAction::Rotate | SelectedObjectAction::Scale
        ) && navigation_editor_owns_session(
            gizmo.dev_state.enabled,
            &gizmo.window_registry,
            &gizmo.blueprint_inspection,
        ) {
            gizmo.inspector.last_message =
                "Transform editing is unavailable while Navigation Editor is active.".into();
            continue;
        }
        match button.action {
            SelectedObjectAction::Move => {
                activate_dev_transform_tool(&mut gizmo, DevTool::Translate);
            }
            SelectedObjectAction::Rotate => {
                activate_dev_transform_tool(&mut gizmo, DevTool::Rotate);
            }
            SelectedObjectAction::Scale => {
                activate_dev_transform_tool(&mut gizmo, DevTool::Scale);
            }
            SelectedObjectAction::Delete => {
                if let Some(target) = selected_object(&gizmo.world_selection) {
                    gizmo.selected_object_ui.request_delete(target);
                }
            }
            SelectedObjectAction::CancelDelete => {
                gizmo.selected_object_ui.clear_pending_delete();
            }
            SelectedObjectAction::ConfirmDelete => {
                let Some(pending) = gizmo.selected_object_ui.pending_delete.take() else {
                    continue;
                };
                execute_delete(pending, &mut gizmo, &action_params, simulation.current_tick);
            }
            SelectedObjectAction::ExitBlueprintInspection => {
                if let Ok(mut cam) = rts_camera.single_mut() {
                    exit_blueprint_inspection_from_ui(
                        &mut gizmo.blueprint_inspection,
                        &mut gizmo.inspector,
                        &mut overlay_focus,
                        &mut cam,
                    );
                }
            }
            SelectedObjectAction::ExitBlueprintEdit => {
                request_exit_blueprint_edit(&mut gizmo.blueprint_inspection, &mut gizmo.inspector);
            }
            SelectedObjectAction::CancelBlueprintPending => {
                cancel_blueprint_pending_confirmation(
                    &mut gizmo.blueprint_inspection,
                    &mut gizmo.inspector,
                );
            }
            SelectedObjectAction::CancelVariantDraft => {
                cancel_blueprint_variant_draft(
                    &mut gizmo.blueprint_inspection,
                    &mut gizmo.inspector,
                );
            }
        }
    }
}

fn execute_delete(
    pending: PendingDeleteTarget,
    gizmo: &mut GizmoInputParams,
    params: &DevBuildingActionParams,
    tick: u64,
) {
    let occ = OccupancyCatalogs {
        doodad: &params.doodad_catalog,
        building: &params.building_catalog,
        footprint: &params.footprint_catalog,
    };
    let inventory_ctx = params.inventory_ctx();
    let inventory_cleanup = BuildingInventoryContext {
        ctx: &inventory_ctx,
        pile_settings: &params.pile_settings,
        interaction_catalog: &params.interaction_catalog,
        tick,
    };

    let selection_params = &mut crate::client::selection::ApplyWorldSelectionParams {
        world_selection: &mut gizmo.world_selection,
        selected_units: &mut gizmo.selected_units,
        building_selection: &mut gizmo.building_selection,
        hud: None,
        revision: Some(&mut gizmo.selection_revision),
    };

    match pending {
        PendingDeleteTarget::Doodad(id) => match remove_doodad(&mut gizmo.world, id, Some(occ)) {
            Ok(_) => {
                gizmo.inspector.last_message = format!("Deleted doodad #{}", id.raw());
                apply_world_selection(WorldSelectionChange::ClearWorldObject, selection_params);
            }
            Err(err) => gizmo.inspector.last_message = format!("Delete failed: {err:?}"),
        },
        PendingDeleteTarget::Building(id) => {
            let _ = destroy_building(
                &mut gizmo.world,
                &params.building_catalog,
                &params.doodad_catalog,
                occ,
                id,
                "dev_destroy",
                Some(&inventory_cleanup),
            );
            gizmo.inspector.last_message = format!("Destroyed building #{}", id.raw());
            apply_world_selection(WorldSelectionChange::ClearWorldObject, selection_params);
        }
        PendingDeleteTarget::ItemPile(pile_id) => {
            match dev_remove_entry(
                &mut gizmo.world,
                &inventory_ctx,
                DevInventoryEndpoint::Pile(pile_id),
                0,
            ) {
                Ok(msg) => {
                    gizmo.inspector.last_message = msg;
                    apply_world_selection(WorldSelectionChange::ClearWorldObject, selection_params);
                }
                Err(err) => gizmo.inspector.last_message = err.to_string(),
            }
        }
    }

    gizmo.tool_state.active_tool = DevTool::Select;
    gizmo.edit.target = None;
    gizmo.edit.mode = DevTool::Select;
    gizmo.edit.cancel_drag();
}
