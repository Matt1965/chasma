//! Inspector panel text formatting (ADR-048).

use bevy::prelude::*;

use crate::debug::{CommandTraceBuffer, recent_combat_log_lines};

use super::snapshot::{
    BuildingInspectorSnapshot, DoodadInspectorSnapshot, InteractionInspectorSnapshot,
    UnitInspectorSnapshot,
};
use super::state::WorldInspectorState;

#[derive(Component, Debug)]
pub(crate) struct DevInspectorText;

pub(crate) fn setup_inspector_panel(parent: &mut ChildSpawnerCommands<'_>) {
    parent.spawn((
        DevInspectorText,
        super::DevInspectorUi,
        crate::dev::DevPanelUi,
        Text::new("Click a unit (Alt+click or Dev Mode) to inspect"),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(Color::srgba(0.78, 0.86, 0.94, 1.0)),
        Node {
            display: Display::None,
            max_height: Val::Px(280.0),
            overflow: Overflow::scroll_y(),
            ..default()
        },
    ));
}

pub(crate) fn sync_inspector_panel(
    dev_state: Res<crate::dev::DevModeState>,
    inspector: Res<WorldInspectorState>,
    tool_state: Res<crate::dev::gizmo::DevToolState>,
    edit: Res<crate::dev::gizmo::TransformEditState>,
    trace: Res<CommandTraceBuffer>,
    mut text: Query<(&mut Text, &mut Node), With<DevInspectorText>>,
) {
    let Ok((mut label, mut node)) = text.single_mut() else {
        return;
    };

    let show = dev_state.enabled && dev_state.active_tab == crate::dev::DevTab::Inspector;
    node.display = if show { Display::Flex } else { Display::None };

    if !show {
        return;
    }

    **label = if let Some(snapshot) = inspector.doodad_snapshot.as_ref() {
        format_doodad_snapshot(snapshot, &tool_state, &edit)
    } else if let Some(snapshot) = inspector.building_snapshot.as_ref() {
        format_building_snapshot(snapshot)
    } else if let Some(snapshot) = inspector.unit_snapshot.as_ref() {
        let mut body = format_unit_snapshot(snapshot);
        let unit_filter = inspector.selected_unit;
        let log_lines = recent_combat_log_lines(&trace, unit_filter, 6);
        if !log_lines.is_empty() {
            body.push_str("\nCombat log:\n");
            for line in log_lines {
                body.push_str(&format!("  {line}\n"));
            }
        }
        body
    } else if let Some(interaction) = inspector.interaction_snapshot.as_ref() {
        format_interaction_snapshot(interaction)
    } else if inspector.last_message.is_empty() {
        "Inspector: Alt+click unit, click doodad/building (Dev Mode), or terrain probe".into()
    } else {
        inspector.last_message.clone()
    };
}

fn format_unit_snapshot(s: &UnitInspectorSnapshot) -> String {
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
        "Inventory: {}\n",
        s.inventory_summary.as_deref().unwrap_or("none"),
    ));

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

fn format_doodad_snapshot(
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
        "\nGizmo: {}  space={}  drag={}  valid={}\n\
         W/E/R = Translate/Rotate/Scale  L = World/Local  Esc = cancel\n\
         Hotkeys: arrows move  [ ] yaw  hold G ground  hold O overlap\n",
        tool_state.active_tool.label(),
        edit.coordinate_space.label(),
        edit.dragging,
        edit.preview_valid,
    ));
    if !edit.last_error.is_empty() {
        out.push_str(&format!("Gizmo error: {}\n", edit.last_error));
    }
    out
}

fn format_building_snapshot(s: &BuildingInspectorSnapshot) -> String {
    format!(
        "Building #{}  {}  def={}\n\
         state={}  progress={:.0}%  operational={}\n\
         hp={}/{}  affiliation={}\n\
         Chunk ({},{})\n\
         {}\n\
         interaction point: {}\n\
         render key: {}\n\
         asset: {}  load: {}\n\
         runtime entity: {}  fallback: {} {}\n\
         scene tags: space={} roof={}\n\
         terrain output: {}  final output: {}\n\
         operation progress: {}  completions: {}  limiting: {}\n\
         Dev: [D]amage [H]eal [X]estroy [R]uins [C]omplete [P]+progress\n\
         Container: [I]nspect [G]old [T]ransfer [U]lock [V]alidate",
        s.building_id.raw(),
        s.display_name,
        s.definition_id.as_str(),
        s.lifecycle_state,
        s.progress_percent,
        s.operational,
        s.current_hp,
        s.max_hp,
        s.affiliation,
        s.chunk.x,
        s.chunk.z,
        s.inventory_summary.as_deref().unwrap_or("no inventory"),
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
        s.terrain_output_rate.as_deref().unwrap_or("—"),
        s.final_output_rate.as_deref().unwrap_or("—"),
        s.operation_progress.as_deref().unwrap_or("—"),
        s.operation_completions
            .map(|count| count.to_string())
            .unwrap_or_else(|| "—".into()),
        s.operation_limiting_factor.as_deref().unwrap_or("—"),
    )
}
