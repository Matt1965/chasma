//! Gameplay-facing UI snapshot derived from client state (ADR-040 U-UI4).
//!
//! Pure data — updated by sync systems, consumed by HUD and feedback layers.

use bevy::prelude::*;

use crate::client::{ClientIntent, CommandType, IntentDispatchStatus, ResolvedCommandFeedback};
use crate::debug::IntentDispatchHistory;
use crate::debug::{
    CommandTraceBuffer, CommandTraceEntry, CommandTraceIntentKind, CommandTraceOutcome,
};
use crate::units::input::SelectedUnits;
use crate::world::UnitId;

/// High-level command label shown in the minimal HUD.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GameplayCommandState {
    #[default]
    Idle,
    Moving,
}

/// Contextual cursor mode driven by selection and hover (intent layer inputs).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GameplayCursorMode {
    #[default]
    Default,
    Move,
    Attack,
    AttackMove,
}

/// Hover classification for cursor presentation (read-only).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum CommandHoverContext {
    #[default]
    None,
    Terrain,
    Unit,
}

/// Read-only gameplay UI mirror updated when inputs change.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct GameplayUiState {
    pub snapshot: GameplayUiSnapshot,
    /// Set when HUD text or portraits need refreshing.
    pub hud_dirty: bool,
}

/// Serializable HUD-facing state for tests and change detection.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GameplayUiSnapshot {
    pub selection_count: u32,
    pub leader_unit: Option<UnitId>,
    pub command_state: GameplayCommandState,
    pub cursor_mode: GameplayCursorMode,
    pub resolved_command_type: Option<CommandType>,
    pub command_tooltip: Option<String>,
    pub debug_overlay_active: bool,
}

/// Whether a trace entry is player-facing (excludes dev-only resolve/shift noise).
pub fn is_gameplay_visible_trace(entry: &CommandTraceEntry) -> bool {
    !matches!(
        entry.intent_kind,
        CommandTraceIntentKind::ShiftModifier | CommandTraceIntentKind::CommandResolve
    )
}

/// Derive HUD snapshot from allowed read-only client sources.
pub fn derive_gameplay_snapshot(
    selection: &SelectedUnits,
    history: &IntentDispatchHistory,
    trace: &CommandTraceBuffer,
    resolved_command: &ResolvedCommandFeedback,
    debug_overlay_active: bool,
    cursor_mode: GameplayCursorMode,
) -> GameplayUiSnapshot {
    let selection_count = selection.0.len() as u32;
    let leader_unit = leader_unit_id(selection);
    let command_state = derive_command_state(selection, history, trace, resolved_command);

    GameplayUiSnapshot {
        selection_count,
        leader_unit,
        command_state,
        cursor_mode,
        resolved_command_type: resolved_command.command_type,
        command_tooltip: resolved_command.tooltip.clone(),
        debug_overlay_active,
    }
}

fn leader_unit_id(selection: &SelectedUnits) -> Option<UnitId> {
    selection.iter().min_by_key(|id| id.raw())
}

pub fn derive_command_state(
    selection: &SelectedUnits,
    history: &IntentDispatchHistory,
    trace: &CommandTraceBuffer,
    resolved_command: &ResolvedCommandFeedback,
) -> GameplayCommandState {
    if selection.is_empty() {
        return GameplayCommandState::Idle;
    }

    if resolved_command.command_type == Some(CommandType::Stop) {
        return GameplayCommandState::Idle;
    }

    let selected: std::collections::HashSet<UnitId> = selection.iter().collect();

    if history
        .report
        .as_ref()
        .is_some_and(|report| report_has_applied_move_for_selection(report, &selected))
    {
        return GameplayCommandState::Moving;
    }

    for entry in trace
        .entries()
        .rev()
        .filter(|entry| is_gameplay_visible_trace(entry))
    {
        if trace_entry_implies_moving(entry, &selected) {
            return GameplayCommandState::Moving;
        }
    }

    GameplayCommandState::Idle
}

fn report_has_applied_move_for_selection(
    report: &crate::client::IntentDispatchReport,
    selected: &std::collections::HashSet<UnitId>,
) -> bool {
    report.records.iter().any(|record| {
        command_intent_applied(&record.intent)
            && record.status == IntentDispatchStatus::Applied
            && record
                .intent
                .affected_units_empty_implies_group_move(selected)
    })
}

fn command_intent_applied(intent: &ClientIntent) -> bool {
    matches!(
        intent,
        ClientIntent::MoveCommand { .. } | ClientIntent::ContextualCommand { .. }
    )
}

fn trace_entry_implies_moving(
    entry: &CommandTraceEntry,
    selected: &std::collections::HashSet<UnitId>,
) -> bool {
    let affects_selection = entry
        .unit_ids
        .iter()
        .any(|unit_id| selected.contains(unit_id));

    match entry.intent_kind {
        CommandTraceIntentKind::MoveCommand | CommandTraceIntentKind::ContextualCommand => {
            affects_selection
                && matches!(
                    entry.outcome,
                    CommandTraceOutcome::Applied
                        | CommandTraceOutcome::OrderQueued
                        | CommandTraceOutcome::OrderResolved
                )
        }
        _ => {
            affects_selection
                && matches!(
                    entry.outcome,
                    CommandTraceOutcome::OrderQueued | CommandTraceOutcome::OrderResolved
                )
        }
    }
}

pub fn derive_cursor_mode(
    has_selection: bool,
    hover: CommandHoverContext,
    armed: Option<crate::client::CommandType>,
    hover_attackable: bool,
) -> GameplayCursorMode {
    if !has_selection {
        return GameplayCursorMode::Default;
    }
    if let Some(crate::client::CommandType::Attack) = armed {
        return match hover {
            CommandHoverContext::Terrain => GameplayCursorMode::AttackMove,
            _ if hover_attackable => GameplayCursorMode::Attack,
            _ => GameplayCursorMode::Default,
        };
    }
    if armed == Some(crate::client::CommandType::AttackMove) {
        return if matches!(hover, CommandHoverContext::Terrain) {
            GameplayCursorMode::AttackMove
        } else {
            GameplayCursorMode::Default
        };
    }
    match hover {
        CommandHoverContext::Terrain | CommandHoverContext::Unit => GameplayCursorMode::Move,
        CommandHoverContext::None => GameplayCursorMode::Default,
    }
}

pub fn command_state_display(
    command_state: GameplayCommandState,
    resolved_command_type: Option<CommandType>,
) -> String {
    if let Some(command_type) = resolved_command_type {
        return command_type.label().to_string();
    }
    match command_state {
        GameplayCommandState::Idle => "Idle".to_string(),
        GameplayCommandState::Moving => "Move".to_string(),
    }
}

trait MoveIntentSelectionCheck {
    fn affected_units_empty_implies_group_move(
        &self,
        selected: &std::collections::HashSet<UnitId>,
    ) -> bool;
}

impl MoveIntentSelectionCheck for ClientIntent {
    fn affected_units_empty_implies_group_move(
        &self,
        selected: &std::collections::HashSet<UnitId>,
    ) -> bool {
        match self {
            ClientIntent::MoveCommand { .. } | ClientIntent::ContextualCommand { .. } => {
                !selected.is_empty()
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{IntentDispatchRecord, IntentDispatchReport, ResolvedCommandFeedback};
    use crate::debug::CommandTraceEntry;
    use crate::world::{ChunkCoord, LocalPosition, UnitOrder, WorldPosition};

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn hud_reflects_selection_count() {
        let mut selection = SelectedUnits::default();
        selection.replace_with([UnitId::new(1), UnitId::new(2), UnitId::new(3)]);
        let snapshot = derive_gameplay_snapshot(
            &selection,
            &IntentDispatchHistory::default(),
            &CommandTraceBuffer::default(),
            &ResolvedCommandFeedback::default(),
            false,
            GameplayCursorMode::Default,
        );
        assert_eq!(snapshot.selection_count, 3);
        assert_eq!(snapshot.leader_unit, Some(UnitId::new(1)));
    }

    #[test]
    fn command_state_moving_on_applied_move_intent() {
        let mut selection = SelectedUnits::default();
        selection.set_single(UnitId::new(5));
        let history = IntentDispatchHistory {
            report: Some(IntentDispatchReport {
                records: vec![IntentDispatchRecord {
                    intent: ClientIntent::MoveCommand {
                        target: pos(10.0, 10.0),
                    },
                    status: IntentDispatchStatus::Applied,
                }],
            }),
        };
        let state = derive_command_state(
            &selection,
            &history,
            &CommandTraceBuffer::default(),
            &ResolvedCommandFeedback::default(),
        );
        assert_eq!(state, GameplayCommandState::Moving);
    }

    #[test]
    fn command_state_idle_when_selection_empty() {
        let selection = SelectedUnits::default();
        let history = IntentDispatchHistory {
            report: Some(IntentDispatchReport {
                records: vec![IntentDispatchRecord {
                    intent: ClientIntent::MoveCommand {
                        target: pos(1.0, 1.0),
                    },
                    status: IntentDispatchStatus::Applied,
                }],
            }),
        };
        assert_eq!(
            derive_command_state(
                &selection,
                &history,
                &CommandTraceBuffer::default(),
                &ResolvedCommandFeedback::default(),
            ),
            GameplayCommandState::Idle
        );
    }

    #[test]
    fn cursor_mode_move_when_selection_and_terrain() {
        assert_eq!(
            derive_cursor_mode(true, CommandHoverContext::Terrain, None, false),
            GameplayCursorMode::Move
        );
        assert_eq!(
            derive_cursor_mode(false, CommandHoverContext::Terrain, None, false),
            GameplayCursorMode::Default
        );
    }

    #[test]
    fn ui_hook_reflects_resolved_command_type() {
        let mut feedback = ResolvedCommandFeedback::default();
        feedback.set_resolved(crate::client::CommandType::Move);
        let label = command_state_display(GameplayCommandState::Idle, feedback.command_type);
        assert_eq!(label, "Move");
        assert!(
            feedback
                .tooltip
                .as_ref()
                .is_some_and(|t| t.contains("Move"))
        );
    }

    #[test]
    fn selection_ui_leader_is_lowest_unit_id() {
        let mut selection = SelectedUnits::default();
        selection.replace_with([UnitId::new(9), UnitId::new(2), UnitId::new(7)]);
        assert_eq!(leader_unit_id(&selection), Some(UnitId::new(2)));
    }

    #[test]
    fn trace_filter_hides_debug_resolve_entries() {
        let entry = CommandTraceEntry {
            tick: 1,
            sequence: 0,
            intent_kind: CommandTraceIntentKind::CommandResolve,
            unit_ids: vec![UnitId::new(1)],
            order: Some(UnitOrder::MoveTo {
                target: pos(0.0, 0.0),
            }),
            outcome: CommandTraceOutcome::OrderResolved,
            path_waypoint_count: Some(3),
            error: None,
            combat_status: None,
            center_distance_meters: None,
            edge_distance_meters: None,
            weapon_range_meters: None,
        };
        assert!(!is_gameplay_visible_trace(&entry));
    }

    #[test]
    fn deterministic_snapshot_from_same_inputs() {
        let mut selection = SelectedUnits::default();
        selection.set_single(UnitId::new(4));
        let history = IntentDispatchHistory::default();
        let trace = CommandTraceBuffer::default();
        let a = derive_gameplay_snapshot(
            &selection,
            &history,
            &trace,
            &ResolvedCommandFeedback::default(),
            false,
            GameplayCursorMode::Default,
        );
        let b = derive_gameplay_snapshot(
            &selection,
            &history,
            &trace,
            &ResolvedCommandFeedback::default(),
            false,
            GameplayCursorMode::Default,
        );
        assert_eq!(a, b);
    }
}
