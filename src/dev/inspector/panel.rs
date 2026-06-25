//! Inspector panel text formatting (ADR-048).

use bevy::prelude::*;

use super::snapshot::{InteractionInspectorSnapshot, UnitInspectorSnapshot};
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

    **label = if let Some(snapshot) = inspector.unit_snapshot.as_ref() {
        format_unit_snapshot(snapshot)
    } else if let Some(interaction) = inspector.interaction_snapshot.as_ref() {
        format_interaction_snapshot(interaction)
    } else if inspector.last_message.is_empty() {
        "Inspector: Alt+click unit or click terrain (Dev Mode) for interaction probe".into()
    } else {
        inspector.last_message.clone()
    };
}

fn format_unit_snapshot(s: &UnitInspectorSnapshot) -> String {
    let mut out = format!(
        "Unit #{}  def={}  state={}  hp={}/{}  combat={}  tick={}\n\
         Chunk ({},{}) terrain={} doodads={} units={}\n\
         Block: {}\n",
        s.unit_id.raw(),
        s.definition_id.as_str(),
        s.state_label,
        s.current_hp,
        s.max_hp,
        s.combat_state_label,
        s.simulation_tick,
        s.chunk.unit_chunk.x,
        s.chunk.unit_chunk.z,
        s.chunk.terrain_loaded,
        s.chunk.doodads_in_chunk,
        s.chunk.units_in_chunk,
        s.block_reason.as_deref().unwrap_or("none"),
    );

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
