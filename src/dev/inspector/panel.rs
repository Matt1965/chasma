//! Inspector snapshot text formatting (ADR-048). UI lives in Selected Object (Slice 5).

use bevy::prelude::*;

use super::snapshot::{DoodadInspectorSnapshot, UnitInspectorSnapshot};

pub(crate) fn format_unit_snapshot_full(s: &UnitInspectorSnapshot) -> String {
    let mut out = format!(
        "Unit #{}  def={}  state={}  hp={}/{}  combat={}  tick={}\n\
         Space: {} (id={})  floor={}\n\
         Chunk ({},{}) terrain={} doodads={} units={}\n\
         Block: {}\n",
        s.unit_id.raw(),
        s.definition_id.as_str(),
        s.state_label,
        s.current_hp,
        s.max_hp,
        s.combat_state_label,
        s.simulation_tick,
        s.current_space_id.raw(),
        s.current_space_id.raw(),
        s.display_floor_label,
        s.chunk.unit_chunk.x,
        s.chunk.unit_chunk.z,
        s.chunk.terrain_loaded,
        s.chunk.doodads_in_chunk,
        s.chunk.units_in_chunk,
        s.block_reason.as_deref().unwrap_or("none"),
    );

    out.push_str(&format!(
        "Inventory: {}\nSettlement: {}\n",
        s.inventory_summary.as_deref().unwrap_or("none"),
        s.settlement_membership,
    ));
    if let (Some(current), Some(max)) = (s.nutrition_current, s.nutrition_max) {
        out.push_str(&format!(
            "Nutrition: {current:.1}/{max:.1}  hunger={}  self_maintenance={}\n",
            s.hunger_stage.as_deref().unwrap_or("-"),
            s.self_maintenance_label.as_deref().unwrap_or("-"),
        ));
    }

    out.push_str(&format!(
        "\nCombat detail: weapon={} target={} phase={}\n",
        s.combat.weapon_name.as_deref().unwrap_or("none"),
        s.combat
            .target_unit_id
            .map(|id| format!("#{}", id.raw()))
            .unwrap_or_else(|| "none".into()),
        s.combat.attack_phase.as_deref().unwrap_or("none"),
    ));

    if !s.projectiles.is_empty() {
        out.push_str("\nProjectiles:\n");
        for projectile in &s.projectiles {
            out.push_str(&format!(
                "  #{} src=#{} tgt=#{} weapon={} speed={:.1} status={}\n",
                projectile.projectile_id.raw(),
                projectile.source_unit_id.raw(),
                projectile.target_unit_id.raw(),
                projectile.weapon_id,
                projectile.speed_mps,
                projectile.status,
            ));
        }
    }

    out.push_str(&format!(
        "\nPath: {} wp  idx={}  len={:.1}m\n",
        s.path.waypoints.len(),
        s.path.waypoint_index,
        s.path.length_meters,
    ));
    for (i, wp) in s.path.waypoints.iter().enumerate() {
        let mark = if i == s.path.waypoint_index { ">" } else { " " };
        out.push_str(&format!(
            "{mark} wp{i}: chunk({}, {}) local({:.1},{:.1})\n",
            wp.chunk.x, wp.chunk.z, wp.local.0.x, wp.local.0.z,
        ));
    }
    if !s.path.chunk_transitions.is_empty() {
        let chunks: Vec<_> = s
            .path
            .chunk_transitions
            .iter()
            .map(|c| format!("({},{})", c.x, c.z))
            .collect();
        out.push_str(&format!("Chunk transitions: {}\n", chunks.join(" -> ")));
    }

    out.push_str(&format!(
        "\nFormation: slot={:?} peers={} spacing={:.2}m\n\
         offset=({:.2},{:.2}) target={}\n",
        s.formation.slot_index,
        s.formation.peers_sharing_target,
        s.formation.spacing_meters,
        s.formation.offset_xz.x,
        s.formation.offset_xz.y,
        s.formation
            .target
            .map(|t| format!("({}, {})", t.chunk.x, t.local.0.x))
            .unwrap_or_else(|| "n/a".into()),
    ));

    out.push_str(&format!(
        "\nSteering: neighbors={}\n\
         path_dir=({:.2},{:.2}) sep=({:.2},{:.2}) coh=({:.2},{:.2})\n\
         align=({:.2},{:.2}) final=({:.2},{:.2})\n",
        s.steering.neighbor_count,
        s.steering.path_direction.x,
        s.steering.path_direction.y,
        s.steering.separation.x,
        s.steering.separation.y,
        s.steering.cohesion.x,
        s.steering.cohesion.y,
        s.steering.alignment.x,
        s.steering.alignment.y,
        s.steering.final_direction.x,
        s.steering.final_direction.y,
    ));

    out
}

pub(crate) fn format_doodad_snapshot_full(
    s: &DoodadInspectorSnapshot,
    tool_state: &crate::dev::gizmo::DevToolState,
    edit: &crate::dev::gizmo::TransformEditState,
) -> String {
    let mut out = format!(
        "Doodad #{}  def={}\n\
         Position (m): ({:.2}, {:.2}, {:.2})\n\
         Rotation (deg): pitch={:.1} yaw={:.1} roll={:.1}\n\
         Scale: ({:.3}, {:.3}, {:.3})\n\
         Visual size (m): ({:.2}, {:.2}, {:.2})\n\
         Collision: {}  cells={}\n",
        s.doodad_id.raw(),
        s.definition_id,
        s.position.x,
        s.position.y,
        s.position.z,
        s.rotation_deg.x,
        s.rotation_deg.y,
        s.rotation_deg.z,
        s.scale.x,
        s.scale.y,
        s.scale.z,
        s.visual_size.x,
        s.visual_size.y,
        s.visual_size.z,
        s.collision_shape,
        s.occupied_cell_count,
    );
    if let Some(warning) = &s.tilt_warning {
        out.push_str(&format!("Tilt warning: {warning}\n"));
    }
    out.push_str(&format!(
        "\nGizmo: {}  drag={}  valid={}\n\
         , . / = Move / Rotate / Scale (world-aligned)\n\
         Hotkeys: arrows move  [ ] yaw\n",
        tool_state.active_tool.label(),
        edit.dragging,
        edit.preview_valid,
    ));
    if !edit.last_error.is_empty() {
        out.push_str(&format!("Gizmo error: {}\n", edit.last_error));
    }
    out
}
