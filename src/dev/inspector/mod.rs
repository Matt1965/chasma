//! World inspector — read-only simulation introspection (ADR-048 U-DEV2).

mod blueprint_edit;
mod blueprint_inspection;
mod building_actions;
mod building_dev_action;
mod capture;
mod doodad_actions;
mod doodad_snapshot;
mod input;
mod panel;
mod params;
mod snapshot;
mod state;

pub use blueprint_edit::{
    BlueprintEditInputParams, FloorPlaneHit, blueprint_edit_blocks_building_selection,
    blueprint_local_to_world, confirm_blueprint_pending_action,
    cursor_ray_to_floor_blueprint_point, editor_add_region, editor_adjust_radius,
    editor_delete_selection, editor_request_apply_to_asset, editor_request_reset_to_asset,
    editor_save_instance_blueprint, editor_select_next_region, editor_select_prev_region,
    editor_submit_variant_draft, enter_blueprint_edit, exit_blueprint_edit_to_inspect,
    handle_blueprint_edit_input, navigation_edit_owns_world_pointer,
    ray_to_building_floor_local_xz, refresh_blueprint_edit_snapshot,
    world_point_to_blueprint_local_xz,
};
pub use blueprint_inspection::{
    BlueprintEditDrag, BlueprintEditSelection, BlueprintEditTool, BlueprintInspectionState,
    BlueprintPendingConfirmation, BlueprintVariantDraft, BlueprintVariantDraftField,
    GeneratedBlueprintDraft, accept_generated_blueprint_draft,
    adopt_generated_blueprint_draft_for_editing, capture_edit_blueprint_snapshot,
    discard_generated_blueprint_draft, enter_blueprint_inspection, exit_blueprint_inspection,
    format_adopted_draft_status_message, format_fatal_generation_failure_message,
    format_generated_draft_status_message, frame_building_for_inspection,
    handle_blueprint_inspection_input, restore_pre_adoption_working_copy,
    sync_navigation_blueprint_session,
};
pub use building_actions::handle_building_production_repeat_button;
pub use building_dev_action::{
    BuildingDevAction, DevBuildingActionButton, apply_building_dev_action,
    handle_building_dev_action_buttons,
};
pub use capture::capture_building_blueprint_inspection_snapshot;
pub use capture::{capture_item_pile_inspector_snapshot, capture_unit_inspector_snapshot};
pub use input::{
    BuildingProductionRepeatModeButton, BuildingProductionRepeatModeButtonText, DevInspectorUi,
    handle_inspector_input, refresh_inspector_snapshot, sync_inspector_on_selection_revision,
};
pub(crate) use panel::{
    format_blueprint_section, format_building_snapshot_full, format_doodad_snapshot_full,
    format_unit_snapshot_full,
};
pub use params::DevBuildingActionParams;
pub use snapshot::{
    BuildingBlueprintInspectorSnapshot, BuildingInspectorSnapshot, ChunkResidencySnapshot,
    DoodadInspectorSnapshot, ItemPileInspectorSnapshot, UnitInspectorSnapshot,
};
pub use state::WorldInspectorState;
