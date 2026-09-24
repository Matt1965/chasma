//! World inspector — read-only simulation introspection (ADR-048 U-DEV2).

mod blueprint_edit;
mod blueprint_inspection;
mod building_actions;
mod building_capabilities;
mod building_dev_action;
mod capture;
mod doodad_snapshot;
mod input;
mod panel;
mod params;
mod snapshot;
mod state;

pub use blueprint_edit::{
    BlueprintEditInputParams, blueprint_edit_blocks_building_selection,
    blueprint_local_to_world, confirm_blueprint_pending_action, editor_add_region, editor_adjust_radius,
    editor_delete_selection, editor_request_apply_to_asset, editor_request_reset_to_asset,
    editor_save_instance_blueprint, editor_select_next_region, editor_select_prev_region,
    editor_submit_variant_draft, enter_blueprint_edit, exit_blueprint_edit_to_inspect,
    handle_blueprint_edit_input, navigation_edit_owns_world_pointer, refresh_blueprint_edit_snapshot,
};
pub use blueprint_inspection::{
    BlueprintEditDrag, BlueprintEditSelection, BlueprintEditTool, BlueprintInspectionState,
    BlueprintPendingConfirmation, BlueprintVariantDraft, BlueprintVariantDraftField, accept_generated_blueprint_draft,
    adopt_generated_blueprint_draft_for_editing, capture_edit_blueprint_snapshot,
    discard_generated_blueprint_draft, enter_blueprint_inspection, exit_blueprint_inspection,
    format_adopted_draft_status_message, frame_building_for_inspection,
    handle_blueprint_inspection_input,
};
#[cfg(test)]
pub use blueprint_inspection::{
    GeneratedBlueprintDraft, format_generated_draft_status_message, restore_pre_adoption_working_copy,
};
pub use building_actions::handle_building_production_repeat_button;
pub use building_capabilities::BuildingDevCapabilities;
pub use building_dev_action::{
    BuildingDevAction, DevBuildingActionButton, DevProductionOperationButton, handle_building_dev_action_buttons,
    handle_production_operation_buttons,
};
pub use capture::capture_building_blueprint_inspection_snapshot;
pub use capture::capture_unit_inspector_snapshot;
pub use input::{
    BuildingProductionRepeatModeButton, BuildingProductionRepeatModeButtonText,
    handle_inspector_input, refresh_inspector_snapshot, sync_inspector_on_selection_revision,
};
pub(crate) use panel::{
    format_doodad_snapshot_full,
    format_unit_snapshot_full,
};
pub use params::DevBuildingActionParams;
pub use snapshot::{
    BuildingBlueprintInspectorSnapshot, BuildingInspectorSnapshot,
    DoodadInspectorSnapshot, ItemPileInspectorSnapshot, UnitInspectorSnapshot,
};
#[cfg(test)]
pub use snapshot::ChunkResidencySnapshot;
pub use state::WorldInspectorState;
