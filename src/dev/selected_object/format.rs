//! Selected Object summary and diagnostics formatting (Slice 5).

use crate::dev::gizmo::{DevToolState, TransformEditState};
use crate::dev::inspector::{
    BuildingInspectorSnapshot, DoodadInspectorSnapshot,
    ItemPileInspectorSnapshot, UnitInspectorSnapshot,
};

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

#[cfg(test)]
pub fn unit_is_player_commandable(snapshot: &UnitInspectorSnapshot) -> bool {
    use crate::world::Affiliation;
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
        "{}\nQty: {}  Weight: {}g\nRotation (deg): pitch={:.1} yaw={:.1} roll={:.1}\n{}",
        snapshot.item_name,
        snapshot.quantity,
        snapshot.weight_grams,
        snapshot.rotation_deg.x,
        snapshot.rotation_deg.y,
        snapshot.rotation_deg.z,
        snapshot.location_summary
    )
}

pub fn format_pile_diagnostics(snapshot: &ItemPileInspectorSnapshot) -> String {
    format!(
        "Pile {:?}\nItem def: {}\nChunk ({}, {})\n\
         Hotkeys: [ ] yaw  ; ' pitch  - = roll\n\
         Use Align to Surface to re-conform to terrain.",
        snapshot.pile_id,
        snapshot.item_definition_id.as_str(),
        snapshot.chunk.x,
        snapshot.chunk.z,
    )
}

