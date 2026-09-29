//! Brain diagnostics panel inside the Debug window.

use bevy::prelude::*;

use super::history::BrainDecisionHistory;
use super::model::{
    BrainCandidateRaceRow, BrainIntentRow, BrainNeedPressureRow, BrainRejectedIntentRow,
    BrainSettlementSnapshot, BrainSpineStage, BrainUnitSnapshot, build_settlement_brain_snapshot,
    build_unit_brain_snapshot,
};
use crate::client::selection::WorldSelectionState;
use crate::client::CameraSettlementContext;
use crate::dev::dev_mode::DevModeState;
use crate::dev::input::DevPanelUi;
use crate::dev::widgets::theme::{TEXT_SECTION, small_text_font};
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::units::input::SelectedUnits;
use crate::world::WorldData;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BrainPanelState {
    pub view: BrainView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrainView {
    #[default]
    Unit,
    Settlement,
}

#[derive(Component, Debug)]
pub struct DevBrainPanelRoot;

#[derive(Component, Debug)]
pub struct DevBrainContentRoot;

#[derive(Component, Debug)]
pub struct DevBrainViewButton {
    pub view: BrainView,
}

#[derive(Component, Debug)]
pub struct DevBrainScoreBarFill;

#[derive(Component, Debug)]
pub struct DevBrainScoreBarRow {
    pub max_value: f32,
}

const BODY_TEXT: Color = Color::srgba(0.72, 0.82, 0.9, 1.0);
const ACCENT: Color = Color::srgba(0.45, 0.72, 0.92, 1.0);
const MUTED: Color = Color::srgba(0.55, 0.65, 0.72, 1.0);

pub fn setup_brain_panel(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevBrainPanelRoot,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                width: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|brain| {
            brain.spawn((
                DevPanelUi,
                Text::new("Brain diagnostics"),
                small_text_font(),
                TextColor(TEXT_SECTION),
            ));
            brain.spawn((
                DevPanelUi,
                Text::new("Why is this unit doing what it is doing?"),
                TextFont {
                    font_size: 9.0,
                    ..default()
                },
                TextColor(MUTED),
            ));
            brain
                .spawn((
                    DevPanelUi,
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    spawn_view_button(row, "Unit", BrainView::Unit);
                    spawn_view_button(row, "Settlement", BrainView::Settlement);
                });
            brain.spawn((
                DevBrainContentRoot,
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    width: Val::Percent(100.0),
                    ..default()
                },
            ));
        });
}

fn spawn_view_button(parent: &mut ChildSpawnerCommands<'_>, label: &'static str, view: BrainView) {
    parent
        .spawn((
            DevBrainViewButton { view },
            DevPanelUi,
            Button,
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.16, 0.22, 0.95)),
            BorderColor::all(Color::srgba(0.28, 0.42, 0.52, 0.85)),
        ))
        .with_children(|btn| {
            btn.spawn((
                DevPanelUi,
                Text::new(label),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(BODY_TEXT),
            ));
        });
}

pub fn handle_brain_view_buttons(
    registry: Res<DevWindowRegistry>,
    dev_state: Res<DevModeState>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut brain_state: ResMut<BrainPanelState>,
    buttons: Query<(&Interaction, &DevBrainViewButton), Changed<Interaction>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Debug) {
        return;
    }
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            brain_state.view = button.view;
        }
    }
}

pub fn sync_brain_panel(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    brain_state: Res<BrainPanelState>,
    world_selection: Res<WorldSelectionState>,
    selected_units: Res<SelectedUnits>,
    settlement_context: Res<CameraSettlementContext>,
    world: Res<WorldData>,
    history: Res<BrainDecisionHistory>,
    mut view_buttons: Query<
        (&DevBrainViewButton, &mut BackgroundColor, &mut BorderColor),
        Without<DevBrainContentRoot>,
    >,
    content_root: Query<Entity, With<DevBrainContentRoot>>,
    mut commands: Commands,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Debug) {
        return;
    }

    for (button, mut bg, mut border) in &mut view_buttons {
        let active = button.view == brain_state.view;
        *bg = BackgroundColor(if active {
            Color::srgba(0.16, 0.28, 0.38, 0.98)
        } else {
            Color::srgba(0.1, 0.16, 0.22, 0.95)
        });
        *border = BorderColor::all(if active {
            ACCENT
        } else {
            Color::srgba(0.28, 0.42, 0.52, 0.85)
        });
    }

    let Ok(root) = content_root.single() else {
        return;
    };
    commands.entity(root).despawn_children();

    let report = &world.worker_assignment_store().last_report;
    let unit_eval = world_selection
        .primary_unit(&selected_units)
        .and_then(|id| report.evaluations.iter().find(|e| e.unit_id == id));

    match brain_state.view {
        BrainView::Unit => {
            let history_lines = world_selection
                .primary_unit(&selected_units)
                .map(|id| history.lines_for(id))
                .unwrap_or_default();
            let snapshot = build_unit_brain_snapshot(
                &world_selection,
                &selected_units,
                &world,
                unit_eval,
                &history_lines,
            );
            commands.entity(root).with_children(|parent| {
                spawn_unit_view(parent, &snapshot);
            });
        }
        BrainView::Settlement => {
            let snapshot = build_settlement_brain_snapshot(&settlement_context, &world);
            commands.entity(root).with_children(|parent| {
                spawn_settlement_view(parent, &snapshot);
            });
        }
    }
}

fn spawn_unit_view(parent: &mut ChildSpawnerCommands<'_>, snapshot: &BrainUnitSnapshot) {
    if let Some(msg) = &snapshot.empty_message {
        spawn_line(parent, msg, MUTED);
        return;
    }
    spawn_line(parent, &snapshot.unit_label, ACCENT);
    spawn_line(parent, &format!("Action: {}", snapshot.action_summary), BODY_TEXT);
    if let Some(task) = &snapshot.task_summary {
        spawn_line(parent, task, BODY_TEXT);
    }
    if let Some(auth) = snapshot.authority {
        spawn_line(parent, &format!("Authority: {}", auth.label()), BODY_TEXT);
    }
    if !snapshot.spine.is_empty() {
        spawn_section(parent, "Decision spine");
        for stage in &snapshot.spine {
            spawn_spine_node(parent, stage);
        }
    }
    if !snapshot.candidate_race.is_empty() {
        spawn_section(parent, "Worker candidate race (SA7)");
        for row in &snapshot.candidate_race {
            spawn_candidate_row(parent, row);
        }
        if snapshot.extra_candidates > 0 {
            spawn_line(
                parent,
                &format!("+{} more eligible candidates", snapshot.extra_candidates),
                MUTED,
            );
        }
    }
    if !snapshot.blocked_alternatives.is_empty() {
        spawn_section(parent, "Why not this?");
        for line in &snapshot.blocked_alternatives {
            spawn_line(parent, line, MUTED);
        }
    }
    if !snapshot.history.is_empty() {
        spawn_section(parent, "Decision history");
        for line in &snapshot.history {
            spawn_line(parent, line, BODY_TEXT);
        }
    }
}

fn spawn_settlement_view(parent: &mut ChildSpawnerCommands<'_>, snapshot: &BrainSettlementSnapshot) {
    if let Some(msg) = &snapshot.empty_message {
        spawn_line(parent, msg, MUTED);
        return;
    }
    spawn_line(
        parent,
        &format!("Settlement: {}", snapshot.settlement_label),
        ACCENT,
    );
    if !snapshot.need_pressures.is_empty() {
        spawn_section(parent, "Need pressures (SA2)");
        for row in &snapshot.need_pressures {
            spawn_need_row(parent, row);
        }
    }
    if !snapshot.selected_intents.is_empty() {
        spawn_section(parent, "Selected intents (SA4)");
        for intent in &snapshot.selected_intents {
            spawn_intent_row(parent, intent);
        }
    }
    if !snapshot.rejected_intents.is_empty() {
        spawn_section(parent, "Rejected candidates (SA4)");
        for row in &snapshot.rejected_intents {
            spawn_rejected_row(parent, row);
        }
    }
    if !snapshot.downstream.is_empty() {
        spawn_section(parent, "Downstream");
        for link in &snapshot.downstream {
            spawn_line(parent, &format!("{}: {}", link.label, link.detail), BODY_TEXT);
        }
    }
}

fn spawn_section(parent: &mut ChildSpawnerCommands<'_>, title: &str) {
    parent.spawn((
        DevPanelUi,
        Text::new(title),
        small_text_font(),
        TextColor(TEXT_SECTION),
        Node {
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        },
    ));
}

fn spawn_line(parent: &mut ChildSpawnerCommands<'_>, text: &str, color: Color) {
    parent.spawn((
        DevPanelUi,
        Text::new(text),
        TextFont {
            font_size: 9.0,
            ..default()
        },
        TextColor(color),
    ));
}

fn spawn_spine_node(parent: &mut ChildSpawnerCommands<'_>, stage: &BrainSpineStage) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                DevPanelUi,
                Text::new("->"),
                TextFont {
                    font_size: 9.0,
                    ..default()
                },
                TextColor(ACCENT),
            ));
            row.spawn((
                DevPanelUi,
                Text::new(format!("{}: {}", stage.label, stage.detail)),
                TextFont {
                    font_size: 9.0,
                    ..default()
                },
                TextColor(BODY_TEXT),
            ));
        });
}

fn spawn_candidate_row(parent: &mut ChildSpawnerCommands<'_>, row: &BrainCandidateRaceRow) {
    let prefix = if row.chosen { "[chosen] " } else { "" };
    spawn_line(
        parent,
        &format!(
            "{}{} total={:.1} (pri={:.0} dist=-{:.2})",
            prefix,
            row.label,
            row.total_score,
            row.priority_component,
            row.distance_component
        ),
        if row.chosen { ACCENT } else { BODY_TEXT },
    );
    spawn_score_bar(parent, row.total_score.max(1.0), row.total_score, "score");
}

fn spawn_need_row(parent: &mut ChildSpawnerCommands<'_>, row: &BrainNeedPressureRow) {
    spawn_line(
        parent,
        &format!("{} pressure={}", row.need_id, row.pressure),
        BODY_TEXT,
    );
    if let Some(detail) = &row.detail {
        spawn_line(parent, detail, MUTED);
    }
}

fn spawn_intent_row(parent: &mut ChildSpawnerCommands<'_>, intent: &BrainIntentRow) {
    spawn_line(
        parent,
        &format!(
            "{} need={} response={} pri={:.1}",
            intent.intent_id, intent.need_id, intent.response_id, intent.priority
        ),
        BODY_TEXT,
    );
    for bar in &intent.arbitration_bars {
        spawn_score_bar(parent, bar.value.abs().max(1.0), bar.value, &bar.label);
    }
}

fn spawn_rejected_row(parent: &mut ChildSpawnerCommands<'_>, row: &BrainRejectedIntentRow) {
    spawn_line(
        parent,
        &format!(
            "{} / {}: {} (arb={:.1})",
            row.need_id, row.response_id, row.reason, row.arbitration_total
        ),
        MUTED,
    );
}

fn spawn_score_bar(
    parent: &mut ChildSpawnerCommands<'_>,
    max_value: f32,
    value: f32,
    label: &str,
) {
    let ratio = (value / max_value).clamp(0.0, 1.0);
    parent
        .spawn((
            DevBrainScoreBarRow { max_value },
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                width: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                DevPanelUi,
                Text::new(label),
                TextFont {
                    font_size: 8.0,
                    ..default()
                },
                TextColor(MUTED),
                Node {
                    min_width: Val::Px(72.0),
                    ..default()
                },
            ));
            row.spawn((
                DevPanelUi,
                Node {
                    flex_grow: 1.0,
                    height: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.1, 0.12, 1.0)),
            ))
            .with_children(|track| {
                track.spawn((
                    DevBrainScoreBarFill,
                    DevPanelUi,
                    Node {
                        width: Val::Percent(ratio * 100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(ACCENT),
                ));
            });
            row.spawn((
                DevPanelUi,
                Text::new(format!("{:.1}", value)),
                TextFont {
                    font_size: 8.0,
                    ..default()
                },
                TextColor(BODY_TEXT),
            ));
        });
}
