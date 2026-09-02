//! Selected Object summary and diagnostics formatting (Slice 5).

use crate::client::selection::WorldSelectionCategory;
use crate::dev::gizmo::{DevToolState, TransformEditState};
use crate::dev::inspector::{
    BuildingBlueprintInspectorSnapshot, BuildingInspectorSnapshot, DoodadInspectorSnapshot,
    ItemPileInspectorSnapshot, UnitInspectorSnapshot,
};
use crate::world::Affiliation;

pub const EMPTY_STATE: &str = "Select a unit, building, doodad, or item pile to inspect it.";

pub fn format_unit_summary(snapshot: &UnitInspectorSnapshot, selected_count: usize) -> String {
    let header = if selected_count > 1 {
        format!(
            "{}\n{} selected — primary: #{}",
            snapshot.definition_id.as_str(),
            selected_count,
            snapshot.unit_id.raw()
        )
    } else {
        format!(
            "{}\nUnit #{}",
            snapshot.definition_id.as_str(),
            snapshot.unit_id.raw()
        )
    };
    let target = snapshot
        .combat
        .target_unit_id
        .map(|id| format!("#{}", id.raw()))
        .unwrap_or_else(|| "none".into());
    format!(
        "{header}\nAffiliation: {}\nHP: {}/{}  State: {}\nCombat: {}  Target: {}\nInventory: {}",
        snapshot.affiliation,
        snapshot.current_hp,
        snapshot.max_hp,
        snapshot.state_label,
        snapshot.combat_state_label,
        target,
        snapshot.inventory_summary.as_deref().unwrap_or("none"),
    )
}

pub fn unit_is_player_commandable(snapshot: &UnitInspectorSnapshot) -> bool {
    snapshot.affiliation == Affiliation::Player.label()
}

pub fn format_unit_diagnostics(snapshot: &UnitInspectorSnapshot) -> String {
    crate::dev::inspector::format_unit_snapshot_full(snapshot)
}

pub fn format_building_summary(snapshot: &BuildingInspectorSnapshot) -> String {
    format!(
        "{}\n{} | HP {}/{}",
        snapshot.display_name, snapshot.lifecycle_state, snapshot.current_hp, snapshot.max_hp,
    )
}

pub fn format_building_navigation_strip(
    blueprint: Option<&BuildingBlueprintInspectorSnapshot>,
) -> String {
    let Some(bp) = blueprint else {
        return "Navigation: None resolved".into();
    };
    format!(
        "Blueprint: {}\nSource: {}  Status: {}\nFloors: {:?}  Validation: {} err / {} warn",
        bp.blueprint_id.as_deref().unwrap_or("—"),
        bp.blueprint_source,
        bp.generation_status,
        bp.floor_ids,
        bp.validation.error_count,
        bp.validation.warning_count,
    )
}

pub fn format_building_navigation_authoring(
    blueprint: Option<&BuildingBlueprintInspectorSnapshot>,
) -> String {
    let Some(bp) = blueprint else {
        return "No blueprint — open the Navigation Editor from Selected Object.".into();
    };
    format!(
        "Blueprint: {}  Source: {}\nOpen Navigation Editor to inspect or edit.",
        bp.blueprint_id.as_deref().unwrap_or("—"),
        bp.blueprint_source,
    )
}

pub fn format_building_diagnostics(
    snapshot: &BuildingInspectorSnapshot,
    caps: crate::dev::inspector::BuildingDevCapabilities,
) -> String {
    super::building_diagnostics::format_contextual_building_diagnostics(snapshot, caps)
}

pub fn format_doodad_summary(
    snapshot: &DoodadInspectorSnapshot,
    tool_state: &DevToolState,
) -> String {
    format!(
        "{}\nDoodad #{}\nPos (m): ({:.2}, {:.2}, {:.2})\nYaw: {:.1}°  Scale: ({:.2}, {:.2}, {:.2})\nSize (m): ({:.2}, {:.2}, {:.2})  Cells: {}\nGizmo: {}",
        snapshot.definition_id,
        snapshot.doodad_id.raw(),
        snapshot.position.x,
        snapshot.position.y,
        snapshot.position.z,
        snapshot.rotation_deg.y,
        snapshot.scale.x,
        snapshot.scale.y,
        snapshot.scale.z,
        snapshot.visual_size.x,
        snapshot.visual_size.y,
        snapshot.visual_size.z,
        snapshot.occupied_cell_count,
        tool_state.active_tool.label(),
    )
}

pub fn format_doodad_diagnostics(
    snapshot: &DoodadInspectorSnapshot,
    tool_state: &DevToolState,
    edit: &TransformEditState,
) -> String {
    crate::dev::inspector::format_doodad_snapshot_full(snapshot, tool_state, edit)
}

pub fn format_pile_summary(snapshot: &ItemPileInspectorSnapshot) -> String {
    format!(
        "{}\nQty: {}  Weight: {}g\n{}",
        snapshot.item_name, snapshot.quantity, snapshot.weight_grams, snapshot.location_summary
    )
}

pub fn format_pile_diagnostics(snapshot: &ItemPileInspectorSnapshot) -> String {
    format!(
        "Pile {:?}\nItem def: {}\nChunk ({}, {})",
        snapshot.pile_id,
        snapshot.item_definition_id.as_str(),
        snapshot.chunk.x,
        snapshot.chunk.z,
    )
}

pub fn category_label(category: WorldSelectionCategory) -> &'static str {
    match category {
        WorldSelectionCategory::None => "None",
        WorldSelectionCategory::Units => "Unit",
        WorldSelectionCategory::Building => "Building",
        WorldSelectionCategory::Doodad => "Doodad",
        WorldSelectionCategory::ItemPile => "Item pile",
    }
}
