//! Inspector snapshot text formatting (ADR-048). UI lives in Selected Object (Slice 5).

use bevy::prelude::*;

use super::snapshot::{
    BuildingBlueprintInspectorSnapshot, BuildingInspectorSnapshot, DoodadInspectorSnapshot,
    InteractionInspectorSnapshot, UnitInspectorSnapshot,
};

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

fn format_interaction_snapshot(s: &InteractionInspectorSnapshot) -> String {
    format!(
        "Interaction probe\n\
         terrain_hit={} type={}\n\
         doodad={}\n\
         command={}\n\
         order={}",
        s.terrain_hit,
        s.interaction_type,
        s.doodad_hit
            .as_ref()
            .map(|id| id.as_str())
            .unwrap_or("none"),
        s.resolved_command.as_deref().unwrap_or("none"),
        s.resolved_order
            .as_ref()
            .map(|o| format!("{o:?}"))
            .unwrap_or_else(|| "none".into()),
    )
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

pub(crate) fn format_building_snapshot_full(
    s: &BuildingInspectorSnapshot,
    advanced: bool,
    blueprint: Option<&BuildingBlueprintInspectorSnapshot>,
) -> String {
    let mut out = format!(
        "Building #{}  {}  def={}\n\
         state={}  progress={:.0}%  operational={}\n\
         hp={}/{}  affiliation={}\n\
         Settlement: {}\n\
         Chunk ({},{})\n\
         {}\n\
         bindings: {}\n\
         interaction point: {}\n\
         render key: {}\n\
         asset: {}  load: {}\n\
         runtime entity: {}  fallback: {} {}\n\
         scene tags: space={} roof={}\n\
         --- Production ---\n\
         operation: {}  lifecycle: {}\n\
         blocking: {}  progress: {}  completions: {}\n\
         mode: {}  remaining: {}  workers: {} (active {})\n\
         supported: {}  default: {}  category: {}  base labor: {}  max workers: {}\n\
         validation: {}\n\
         execution inputs: {}\n\
         execution outputs: {}\n\
         inventories: {}\n\
         execution blocking: {}\n\
         terrain assessment: {}\n\
         assessment revision: {}  stale: {}\n\
         --- Logistics ---\n\
         {}\n\
         --- Settlement Runtime ---\n\
{}\n\
         policy enabled: {}  paused: {}  control: {}  priority: {}\n\
         efficiency terrain: {}  final: {}  limiting: {}\n\
         Dev building actions: see Selected Object → Building sections (Construction, Production, Inventory, Logistics, Lifecycle).\n",
        s.building_id.raw(),
        s.display_name,
        s.definition_id.as_str(),
        s.lifecycle_state,
        s.progress_percent,
        s.operational,
        s.current_hp,
        s.max_hp,
        s.affiliation,
        s.settlement_membership,
        s.chunk.x,
        s.chunk.z,
        s.inventory_summary.as_deref().unwrap_or("no inventory"),
        s.inventory_bindings_summary.as_deref().unwrap_or("—"),
        s.interaction_point.as_deref().unwrap_or("—"),
        s.desired_render_key.as_deref().unwrap_or("—"),
        s.resolved_asset_path.as_deref().unwrap_or("—"),
        s.asset_load_state.as_deref().unwrap_or("—"),
        s.runtime_entity
            .map(|bits| bits.to_string())
            .unwrap_or_else(|| "—".into()),
        s.uses_diagnostic_fallback,
        s.fallback_reason.as_deref().unwrap_or("—"),
        s.space_tag_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "—".into()),
        s.roof_tag_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "—".into()),
        s.selected_operation.as_deref().unwrap_or("None"),
        s.production_lifecycle.as_deref().unwrap_or("—"),
        s.production_blocking_reason.as_deref().unwrap_or("—"),
        s.operation_progress.as_deref().unwrap_or("—"),
        s.operation_completions
            .map(|count| count.to_string())
            .unwrap_or_else(|| "—".into()),
        s.repeat_mode.as_deref().unwrap_or("—"),
        s.remaining_repeat_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "—".into()),
        s.assigned_workers.as_deref().unwrap_or("—"),
        s.active_worker_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "—".into()),
        s.supported_operations.as_deref().unwrap_or("—"),
        s.default_operation.as_deref().unwrap_or("—"),
        s.operation_category.as_deref().unwrap_or("—"),
        s.base_labor
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".into()),
        s.max_workers
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".into()),
        s.validation_state.as_deref().unwrap_or("—"),
        s.execution_inputs_summary.as_deref().unwrap_or("—"),
        s.execution_outputs_summary.as_deref().unwrap_or("—"),
        s.execution_inventory_summary.as_deref().unwrap_or("—"),
        s.execution_blocking.as_deref().unwrap_or("—"),
        s.terrain_assessment_summary.as_deref().unwrap_or("—"),
        s.terrain_assessment_revision
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".into()),
        s.terrain_assessment_stale
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".into()),
        s.hauling_requests_summary.as_deref().unwrap_or("—"),
        s.planner_summary.as_deref().unwrap_or("—"),
        s.policy_enabled
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".into()),
        s.policy_paused
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".into()),
        s.control_source.as_deref().unwrap_or("—"),
        s.policy_priority
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".into()),
        s.terrain_output_rate.as_deref().unwrap_or("—"),
        s.final_output_rate.as_deref().unwrap_or("—"),
        s.operation_limiting_factor.as_deref().unwrap_or("—"),
    );
    if advanced {
        out.push_str(&format!(
            "\n--- Production Advanced ---\n\
             efficiency revision: {}\n\
             assigned worker ids: {}",
            s.last_efficiency_revision
                .map(|value| value.to_string())
                .unwrap_or_else(|| "—".into()),
            s.assigned_workers.as_deref().unwrap_or("—"),
        ));
    }
    if let Some(bp) = blueprint {
        out.push_str(&format_blueprint_section(bp));
    }
    out
}

pub(crate) fn format_blueprint_section(bp: &BuildingBlueprintInspectorSnapshot) -> String {
    let mut section = format!(
        "\n--- Building Navigation Blueprint ---\n\
         id: {}  source: {}\n\
         generator v{}  status: {}  cache fresh: {}\n\
         fingerprint: {}\n\
         floors: {:?}  selected: {}  vertices: {}\n\
         elevation: {}  entrances: {}  transitions: {}\n\
         validation: {} errors, {} warnings, {} info\n",
        bp.blueprint_id.as_deref().unwrap_or("—"),
        bp.blueprint_source,
        bp.generator_version,
        bp.generation_status,
        bp.cache_fresh,
        bp.source_fingerprint.as_deref().unwrap_or("—"),
        bp.floor_ids,
        bp.selected_floor_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "—".into()),
        bp.selected_floor_vertex_count,
        bp.selected_floor_elevation
            .map(|v| format!("{v:.2}m"))
            .unwrap_or_else(|| "—".into()),
        bp.entrance_count,
        bp.transition_count,
        bp.validation.error_count,
        bp.validation.warning_count,
        bp.validation.info_count,
    );
    if bp.inspection_active {
        section.push_str("inspection: ACTIVE (bird's-eye)\n");
    } else {
        section.push_str("inspection: use Selected Object → Open Navigation Editor\n");
    }
    if bp.edit_active {
        section.push_str(&format!(
            "edit: ACTIVE{} — use Navigation Editor window for tools and persistence\n",
            if bp.edit_dirty { " (unsaved)" } else { "" }
        ));
        if let Some(selected) = &bp.selected_element {
            section.push_str(&format!("selected: {selected}\n"));
        }
        if bp.variant_draft_active {
            section.push_str("--- Save As Variant ---\n");
            section.push_str(&format!(
                "name{}: {}\n",
                active_marker(bp.variant_draft_active_field.as_deref(), "display name"),
                bp.variant_draft_display_name.as_deref().unwrap_or("—")
            ));
            section.push_str(&format!(
                "asset id{}: {}\n",
                active_marker(bp.variant_draft_active_field.as_deref(), "asset id"),
                bp.variant_draft_asset_id.as_deref().unwrap_or("—")
            ));
            section.push_str(&format!(
                "description{}: {}\n",
                active_marker(bp.variant_draft_active_field.as_deref(), "description"),
                bp.variant_draft_description.as_deref().unwrap_or("—")
            ));
            section.push_str(
                "Use Navigation Editor variant draft controls and Selected Object cancel.\n",
            );
        }
    } else if bp.inspection_active {
        section.push_str("edit: use Navigation Editor → Edit mode\n");
        section.push_str("regenerate: Navigation Editor → Regenerate (confirms when authored)\n");
    }
    for (index, diag) in bp.validation.diagnostics.iter().enumerate().take(12) {
        let level = match diag.level {
            crate::world::BlueprintDiagnosticLevel::Error => "ERR",
            crate::world::BlueprintDiagnosticLevel::Warning => "WRN",
            crate::world::BlueprintDiagnosticLevel::Info => "INF",
        };
        section.push_str(&format!(
            "  [{index}] {level} {}: {}\n",
            diag.code, diag.message
        ));
    }
    if !bp.selected_floor_entrances.is_empty() {
        section.push_str("floor entrances:\n");
        for line in &bp.selected_floor_entrances {
            section.push_str(&format!("  {line}\n"));
        }
    }
    if !bp.selected_floor_transitions.is_empty() {
        section.push_str("floor transitions:\n");
        for line in &bp.selected_floor_transitions {
            section.push_str(&format!("  {line}\n"));
        }
    }
    section
}

fn active_marker(active_field: Option<&str>, field: &str) -> &'static str {
    if active_field == Some(field) {
        " *"
    } else {
        ""
    }
}
