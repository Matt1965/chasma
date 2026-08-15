//! Navigation Editor input routing (Slice 7).

use bevy::prelude::*;

use crate::camera::{RtsCamera, RtsCameraState};
use crate::client::selection::WorldSelectionState;
use crate::debug::UnitPathDiagnosticStore;
use crate::dev::dev_mode::DevModeState;
use crate::dev::inspector::{
    BlueprintEditInputParams, BlueprintEditTool, BlueprintInspectionState,
    BlueprintPendingConfirmation, WorldInspectorState, accept_generated_blueprint_draft,
    adopt_generated_blueprint_draft_for_editing, confirm_blueprint_pending_action,
    discard_generated_blueprint_draft, editor_adjust_radius, editor_delete_selection,
    editor_request_apply_to_asset, editor_request_reset_to_asset, editor_save_instance_blueprint,
    editor_submit_variant_draft,
};
use crate::dev::window::DevWindowRegistry;
use crate::world::{
    BuildingCatalog, BuildingNavigationBlueprintCatalog, WorldData, classify_blueprint_authority,
};

use super::commands::{
    FloorStep, begin_edit_for_building, begin_inspection_for_building, change_floor,
    exit_edit_session, frame_selected_building, open_navigation_editor, refresh_editor_snapshot,
    request_close_navigation_editor, set_edit_tool, start_variant_draft,
};
use super::panel::{
    DevNavigationEditorActionButton, DevNavigationEditorOpenButton, NavigationEditorAction,
};
use super::state::NavigationEditorUiState;
use crate::dev::NavigationEditorBlockedAction;
use crate::dev::inspector::{
    editor_add_region, editor_select_next_region, editor_select_prev_region,
};
use crate::dev::widgets::queue_button_activation_flash;

/// Open/focus Navigation Editor from launcher buttons.
pub fn handle_open_navigation_editor_buttons(
    mut dev_state: ResMut<DevModeState>,
    mut registry: ResMut<DevWindowRegistry>,
    mut inspection: ResMut<BlueprintInspectionState>,
    mut inspector: ResMut<WorldInspectorState>,
    world_selection: Res<WorldSelectionState>,
    mut overlay_focus: ResMut<crate::debug::InspectorOverlayFocus>,
    world: Res<WorldData>,
    building_catalog: Res<BuildingCatalog>,
    nav_catalog: Res<BuildingNavigationBlueprintCatalog>,
    camera_settings: Res<crate::camera::CameraSettings>,
    mut camera: Query<&mut RtsCameraState, With<RtsCamera>>,
    buttons: Query<&Interaction, (With<DevNavigationEditorOpenButton>, Changed<Interaction>)>,
) {
    if !dev_state.enabled {
        return;
    }
    for interaction in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        open_navigation_editor(&mut registry);
        let Some(building_id) = world_selection.building_id else {
            continue;
        };
        if !inspection.active {
            if let Ok(mut cam) = camera.single_mut() {
                begin_inspection_for_building(
                    building_id,
                    &mut inspection,
                    &mut inspector,
                    &mut overlay_focus,
                    &mut cam,
                    &mut dev_state.debug_config,
                    &world,
                    &building_catalog,
                    &nav_catalog,
                    &camera_settings,
                    false,
                );
            }
        }
    }
}

/// Navigation Editor action buttons.
pub fn handle_navigation_editor_actions(
    time: Res<Time>,
    mut commands: Commands,
    mut edit_params: BlueprintEditInputParams<'_, '_>,
    mut path_store: ResMut<UnitPathDiagnosticStore>,
    buttons: Query<(Entity, &Interaction, &DevNavigationEditorActionButton), Changed<Interaction>>,
) {
    if !edit_params.dev_state.enabled {
        return;
    }
    if !edit_params
        .window_registry
        .is_visible(crate::dev::window::DevWindowId::NavigationEditor)
    {
        return;
    }

    for (entity, interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if button.disabled {
            continue;
        }
        edit_params.gate.block_gameplay_mouse = true;
        match button.action {
            NavigationEditorAction::InspectMode => {
                let Some(building_id) = edit_params.world_selection.building_id else {
                    continue;
                };
                if let Ok(mut cam) = edit_params.rts_camera.single_mut() {
                    begin_inspection_for_building(
                        building_id,
                        &mut edit_params.inspection,
                        &mut edit_params.inspector,
                        &mut edit_params.overlay_focus,
                        &mut cam,
                        &mut edit_params.dev_state.debug_config,
                        &edit_params.world,
                        &edit_params.building_catalog,
                        &edit_params.nav_catalog,
                        &edit_params.camera_settings,
                        false,
                    );
                }
            }
            NavigationEditorAction::EditMode => {
                let Some(building_id) = edit_params.world_selection.building_id else {
                    continue;
                };
                if let Ok(mut cam) = edit_params.rts_camera.single_mut() {
                    begin_edit_for_building(
                        building_id,
                        &mut edit_params.inspection,
                        &mut edit_params.inspector,
                        &mut edit_params.overlay_focus,
                        &mut cam,
                        &mut edit_params.dev_state.debug_config,
                        &edit_params.world,
                        &edit_params.building_catalog,
                        &edit_params.nav_catalog,
                        &edit_params.camera_settings,
                    );
                }
            }
            NavigationEditorAction::ExitEdit => {
                exit_edit_session(&mut edit_params.inspection, &mut edit_params.inspector);
            }
            NavigationEditorAction::FloorPrev => step_floor(&mut edit_params, FloorStep::Previous),
            NavigationEditorAction::FloorNext => step_floor(&mut edit_params, FloorStep::Next),
            NavigationEditorAction::ToolSelect => {
                set_edit_tool(&mut edit_params.inspection, BlueprintEditTool::Select);
            }
            NavigationEditorAction::ToolAddCorner => {
                if !edit_params.inspection.editing {
                    let Some(building_id) = edit_params.world_selection.building_id else {
                        continue;
                    };
                    if let Ok(mut cam) = edit_params.rts_camera.single_mut() {
                        if begin_edit_for_building(
                            building_id,
                            &mut edit_params.inspection,
                            &mut edit_params.inspector,
                            &mut edit_params.overlay_focus,
                            &mut cam,
                            &mut edit_params.dev_state.debug_config,
                            &edit_params.world,
                            &edit_params.building_catalog,
                            &edit_params.nav_catalog,
                            &edit_params.camera_settings,
                        ) {
                            edit_params
                                .inspection
                                .sync_selected_floor_from_working_copy();
                            edit_params.overlay_focus.blueprint_floor_id =
                                edit_params.inspection.selected_floor_id;
                        }
                    }
                }
                set_edit_tool(&mut edit_params.inspection, BlueprintEditTool::AddVertex);
            }
            NavigationEditorAction::ToolAddEntrance => {
                set_edit_tool(&mut edit_params.inspection, BlueprintEditTool::AddEntrance);
            }
            NavigationEditorAction::ToolAddRegion => editor_add_region(&mut edit_params),
            NavigationEditorAction::ToolAddConnection => {
                set_edit_tool(
                    &mut edit_params.inspection,
                    BlueprintEditTool::AddConnection,
                );
                edit_params.inspection.pending_connection_regions = None;
                edit_params.inspector.last_message =
                    "Click source region, then destination region".into();
            }
            NavigationEditorAction::SelectRegionPrev => {
                editor_select_prev_region(&mut edit_params);
            }
            NavigationEditorAction::SelectRegionNext => {
                editor_select_next_region(&mut edit_params);
            }
            NavigationEditorAction::DeleteSelection => editor_delete_selection(&mut edit_params),
            NavigationEditorAction::RadiusUp => editor_adjust_radius(&mut edit_params, 0.1),
            NavigationEditorAction::RadiusDown => editor_adjust_radius(&mut edit_params, -0.1),
            NavigationEditorAction::FrameBuilding => {
                if let Ok(mut cam) = edit_params.rts_camera.single_mut() {
                    frame_selected_building(
                        &edit_params.inspection,
                        &edit_params.inspector,
                        &mut cam,
                        &edit_params.camera_settings,
                    );
                }
            }
            NavigationEditorAction::ReturnCamera => {
                if let Ok(mut cam) = edit_params.rts_camera.single_mut() {
                    if let Some(saved) = edit_params.inspection.saved_camera.take() {
                        *cam = saved;
                        edit_params.inspection.saved_camera = Some(saved);
                    }
                }
            }
            NavigationEditorAction::Regenerate => request_regenerate(&mut edit_params),
            NavigationEditorAction::EditDraft => {
                if edit_params.inspection.dirty && edit_params.inspection.working_copy.is_some() {
                    edit_params.inspection.pending_confirmation =
                        Some(BlueprintPendingConfirmation::AdoptGeneratedDraft);
                    edit_params.inspector.last_message =
                        "Unsaved working-copy edits — confirm adopt generated draft or cancel."
                            .into();
                    continue;
                }
                match adopt_generated_blueprint_draft_for_editing(&mut edit_params.inspection) {
                    Ok(()) => {
                        edit_params
                            .inspection
                            .sync_selected_floor_from_working_copy();
                        edit_params.overlay_focus.blueprint_floor_id =
                            edit_params.inspection.selected_floor_id;
                        edit_params.dev_state.debug_config.nav_blueprint = true;
                        edit_params.inspector.last_message =
                            "Editing generated draft in working copy (unsaved).".into();
                        if let Some(building_id) = edit_params.world_selection.building_id {
                            refresh_editor_snapshot(
                                &edit_params.world,
                                &edit_params.building_catalog,
                                &edit_params.nav_catalog,
                                building_id,
                                &edit_params.inspection,
                                &mut edit_params.inspector,
                            );
                        }
                    }
                    Err(err) => edit_params.inspector.last_message = err,
                }
            }
            NavigationEditorAction::ReplaceWorkingCopy => {
                let (current_regions, current_connections) = edit_params
                    .inspection
                    .working_topology_summary()
                    .unwrap_or((0, 0));
                let (draft_regions, draft_connections) = edit_params
                    .inspection
                    .draft_topology_summary()
                    .unwrap_or((0, 0));
                edit_params.inspection.pending_confirmation =
                    Some(BlueprintPendingConfirmation::ReplaceWorkingCopyWithDraft {
                        current_regions,
                        current_connections,
                        draft_regions,
                        draft_connections,
                    });
                edit_params.inspector.last_message = format!(
                    "Replace working copy?\nCurrent: {current_regions} regions · {current_connections} connections\nGenerated: {draft_regions} regions · {draft_connections} connections"
                );
            }
            NavigationEditorAction::AcceptDraft => {
                match accept_generated_blueprint_draft(&mut edit_params.inspection) {
                    Ok(()) => {
                        edit_params
                            .inspection
                            .sync_selected_floor_from_working_copy();
                        edit_params.overlay_focus.blueprint_floor_id =
                            edit_params.inspection.selected_floor_id;
                        edit_params.dev_state.debug_config.nav_blueprint = true;
                        edit_params.inspector.last_message =
                            "Accepted generated draft into working copy (unsaved).".into();
                        if let Some(building_id) = edit_params.world_selection.building_id {
                            refresh_editor_snapshot(
                                &edit_params.world,
                                &edit_params.building_catalog,
                                &edit_params.nav_catalog,
                                building_id,
                                &edit_params.inspection,
                                &mut edit_params.inspector,
                            );
                        }
                    }
                    Err(err) => edit_params.inspector.last_message = err,
                }
            }
            NavigationEditorAction::DiscardDraft => {
                discard_generated_blueprint_draft(&mut edit_params.inspection);
                edit_params.inspector.last_message =
                    "Discarded generated draft; working copy unchanged.".into();
            }
            NavigationEditorAction::ToggleDraftPreview => {
                if edit_params.inspection.has_pending_generated_draft() {
                    edit_params.inspection.draft_preview_active =
                        !edit_params.inspection.draft_preview_active;
                    edit_params.inspector.last_message =
                        if edit_params.inspection.draft_preview_active {
                            "Draft preview overlay enabled.".into()
                        } else {
                            "Draft preview overlay disabled.".into()
                        };
                }
            }
            NavigationEditorAction::Validate => {
                edit_params.nav_ui.validation_expanded = true;
                if let Some(building_id) = edit_params.world_selection.building_id {
                    refresh_editor_snapshot(
                        &edit_params.world,
                        &edit_params.building_catalog,
                        &edit_params.nav_catalog,
                        building_id,
                        &edit_params.inspection,
                        &mut edit_params.inspector,
                    );
                }
            }
            NavigationEditorAction::SaveInstance => {
                editor_save_instance_blueprint(&mut edit_params)
            }
            NavigationEditorAction::ApplyToAsset => editor_request_apply_to_asset(&mut edit_params),
            NavigationEditorAction::ResetToAsset => editor_request_reset_to_asset(&mut edit_params),
            NavigationEditorAction::SaveAsVariant => {
                let Some(building_id) = edit_params.world_selection.building_id else {
                    continue;
                };
                start_variant_draft(
                    &mut edit_params.inspection,
                    &mut edit_params.inspector,
                    &edit_params.world,
                    &edit_params.building_catalog,
                    building_id,
                    &mut edit_params.nav_ui,
                );
            }
            NavigationEditorAction::CreateVariant => {
                let display_name = edit_params.nav_ui.variant_display_name.clone();
                let asset_id = edit_params.nav_ui.variant_asset_id.clone();
                let description = edit_params.nav_ui.variant_description.clone();
                editor_submit_variant_draft(
                    &mut edit_params,
                    &display_name,
                    &asset_id,
                    &description,
                );
            }
            NavigationEditorAction::ConfirmPending => {
                let close_after = matches!(
                    edit_params.nav_ui.pending_blocked_action,
                    Some(NavigationEditorBlockedAction::CloseWindow)
                );
                let _ = confirm_blueprint_pending_action(&mut edit_params);
                if close_after && edit_params.inspection.pending_confirmation.is_none() {
                    edit_params
                        .window_registry
                        .hide(crate::dev::window::DevWindowId::NavigationEditor);
                    edit_params.nav_ui.clear_blocked();
                }
            }
            NavigationEditorAction::CancelPending => {
                edit_params.inspection.pending_confirmation = None;
                edit_params.nav_ui.clear_blocked();
                edit_params.inspector.last_message = "Cancelled pending blueprint action".into();
            }
            NavigationEditorAction::CancelVariant => {
                edit_params.inspection.variant_draft = None;
                edit_params.inspector.last_message = "Cancelled Save As Variant".into();
            }
            NavigationEditorAction::OverlayBlueprint => {
                edit_params.dev_state.debug_config.nav_blueprint =
                    !edit_params.dev_state.debug_config.nav_blueprint;
            }
            NavigationEditorAction::OverlayEntrances => {
                edit_params.dev_state.debug_config.nav_entrances =
                    !edit_params.dev_state.debug_config.nav_entrances;
                edit_params.dev_state.debug_config.enabled = true;
            }
            NavigationEditorAction::OverlayBlockedArea => {
                edit_params.dev_state.debug_config.nav_blockers =
                    !edit_params.dev_state.debug_config.nav_blockers;
                edit_params.dev_state.debug_config.enabled = true;
            }
            NavigationEditorAction::ClearRecordedPath => {
                path_store.clear_all();
                edit_params.inspector.last_message = "Cleared recorded unit path traces.".into();
            }
        }
        queue_button_activation_flash(&mut commands, entity, time.elapsed_secs());
    }
}

fn step_floor(params: &mut BlueprintEditInputParams<'_, '_>, direction: FloorStep) {
    let Some(building_id) = params.world_selection.building_id else {
        return;
    };
    change_floor(
        &mut params.inspection,
        &mut params.inspector,
        &mut params.overlay_focus,
        &params.world,
        &params.building_catalog,
        &params.nav_catalog,
        building_id,
        direction,
        false,
    );
}

fn request_regenerate(params: &mut BlueprintEditInputParams<'_, '_>) {
    let Some(building_id) = params.world_selection.building_id else {
        return;
    };
    let Some(record) = params.world.get_building(building_id) else {
        return;
    };
    let Some(definition) = params.building_catalog.get(&record.definition_id) else {
        return;
    };
    let authority = classify_blueprint_authority(
        definition,
        &params.nav_catalog,
        record.interior.navigation_blueprint_override.as_ref(),
    );
    if authority != crate::world::BlueprintAuthoritySource::None {
        params.inspection.pending_confirmation =
            Some(BlueprintPendingConfirmation::RegenerateFromMesh {
                current_source: authority.label().to_string(),
                destructive: false,
            });
        params.inspector.last_message = format!(
            "{} a separate generated draft from the building mesh (current source: {}). \
             The working copy is unchanged until you replace it.",
            if params.inspection.has_pending_generated_draft() {
                "Regenerate"
            } else {
                "Generate"
            },
            authority.label()
        );
    } else {
        #[cfg(feature = "data-import")]
        {
            params.inspection.pending_confirmation =
                Some(BlueprintPendingConfirmation::RegenerateFromMesh {
                    current_source: "generated".into(),
                    destructive: false,
                });
            params.inspector.last_message =
                "Generate a separate draft from the building mesh? The working copy stays unchanged."
                    .into();
        }
        #[cfg(not(feature = "data-import"))]
        {
            params.inspector.last_message =
                "Blueprint regeneration requires the data-import feature.".into();
        }
    }
}

/// Intercept Navigation Editor close when edits are dirty.
pub fn handle_navigation_editor_close_guard(
    dev_state: Res<DevModeState>,
    mut registry: ResMut<DevWindowRegistry>,
    mut inspection: ResMut<BlueprintInspectionState>,
    mut ui_state: ResMut<NavigationEditorUiState>,
    close_buttons: Query<
        (&Interaction, &crate::dev::window::DevWindowCloseButton),
        Changed<Interaction>,
    >,
) {
    if !dev_state.enabled {
        return;
    }
    for (interaction, button) in &close_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if button.id != crate::dev::window::DevWindowId::NavigationEditor {
            continue;
        }
        if inspection.editing && inspection.dirty {
            let _ = request_close_navigation_editor(&mut registry, &mut inspection, &mut ui_state);
            registry.show(crate::dev::window::DevWindowId::NavigationEditor);
        }
    }
}
