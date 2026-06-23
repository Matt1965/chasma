//! Command trace capture for intent → command observability (ADR-039 U-UI3).

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::client::{ClientIntent, IntentDispatchRecord, IntentDispatchReport, IntentDispatchStatus};
use crate::units::input::MoveOrdersReport;
use crate::world::{
    CommandBufferResolveReport, CommandResolveSuccess, UnitId, UnitOrder, UnitOrderError,
    WorldPosition,
};

/// Monotonic client frame index for trace ordering.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClientFrameIndex(pub u64);

/// Last frame's intent dispatch report (read-only for overlays).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct IntentDispatchHistory {
    pub report: Option<IntentDispatchReport>,
}

/// Outcome recorded for one traced command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTraceOutcome {
    Applied,
    Ignored,
    OrderQueued,
    OrderFailed,
    OrderResolved,
    ResolveFailed,
}

/// Simplified intent kind for trace entries (stable for tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandTraceIntentKind {
    SelectUnit,
    ToggleUnitSelection,
    BoxSelect,
    BoxSelectAdd,
    ClearSelection,
    ContextualCommand,
    MoveCommand,
    ShiftModifier,
    CommandResolve,
}

impl CommandTraceIntentKind {
    pub fn from_intent(intent: &ClientIntent) -> Self {
        match intent {
            ClientIntent::SelectUnit { .. } => Self::SelectUnit,
            ClientIntent::ToggleUnitSelection { .. } => Self::ToggleUnitSelection,
            ClientIntent::BoxSelect { .. } => Self::BoxSelect,
            ClientIntent::BoxSelectAdd { .. } => Self::BoxSelectAdd,
            ClientIntent::ClearSelection => Self::ClearSelection,
            ClientIntent::ContextualCommand { .. } => Self::ContextualCommand,
            ClientIntent::MoveCommand { .. } => Self::MoveCommand,
            ClientIntent::ShiftModifier { .. } => Self::ShiftModifier,
        }
    }
}

/// One observable command / intent trace entry.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandTraceEntry {
    pub tick: u64,
    pub sequence: u32,
    pub intent_kind: CommandTraceIntentKind,
    pub unit_ids: Vec<UnitId>,
    pub order: Option<UnitOrder>,
    pub outcome: CommandTraceOutcome,
    pub path_waypoint_count: Option<u32>,
    pub error: Option<UnitOrderError>,
}

/// Ring buffer of recent command traces (simulation writes, overlays read).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct CommandTraceBuffer {
    entries: VecDeque<CommandTraceEntry>,
    entries_this_tick: u32,
    next_sequence: u32,
    active_tick: u64,
}

pub const TRACE_BUFFER_CAPACITY: usize = 256;
pub const MAX_TRACE_ENTRIES_PER_TICK: u32 = 64;

impl Default for CommandTraceBuffer {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(TRACE_BUFFER_CAPACITY),
            entries_this_tick: 0,
            next_sequence: 0,
            active_tick: 0,
        }
    }
}

impl CommandTraceBuffer {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> impl DoubleEndedIterator<Item = &CommandTraceEntry> {
        self.entries.iter()
    }

    pub fn entries_for_tick(&self, tick: u64) -> impl Iterator<Item = &CommandTraceEntry> {
        self.entries.iter().filter(move |entry| entry.tick == tick)
    }

    pub fn latest(&self) -> Option<&CommandTraceEntry> {
        self.entries.back()
    }

    pub fn begin_tick(&mut self, tick: u64) {
        self.active_tick = tick;
        self.entries_this_tick = 0;
        self.next_sequence = 0;
    }

    fn push_entry(&mut self, entry: CommandTraceEntry) -> bool {
        if self.entries_this_tick >= MAX_TRACE_ENTRIES_PER_TICK {
            return false;
        }
        if self.is_duplicate(&entry) {
            return false;
        }
        if self.entries.len() >= TRACE_BUFFER_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        self.entries_this_tick += 1;
        true
    }

    fn is_duplicate(&self, entry: &CommandTraceEntry) -> bool {
        self.entries.iter().any(|existing| {
            existing.tick == entry.tick
                && existing.intent_kind == entry.intent_kind
                && existing.unit_ids == entry.unit_ids
                && existing.order == entry.order
                && existing.outcome == entry.outcome
        })
    }

    fn next_sequence(&mut self) -> u32 {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        sequence
    }

    pub fn record_intent_dispatch(
        &mut self,
        tick: u64,
        intent: &ClientIntent,
        status: IntentDispatchStatus,
        unit_ids: &[UnitId],
        move_report: Option<&MoveOrdersReport>,
    ) {
        let outcome = match status {
            IntentDispatchStatus::Applied => CommandTraceOutcome::Applied,
            IntentDispatchStatus::Ignored => CommandTraceOutcome::Ignored,
        };

        if let Some(report) = move_report {
            for trace in &report.unit_traces {
                let unit_outcome = if trace.error.is_some() {
                    CommandTraceOutcome::OrderFailed
                } else {
                    CommandTraceOutcome::OrderQueued
                };
                let sequence = self.next_sequence();
                self.push_entry(CommandTraceEntry {
                    tick,
                    sequence,
                    intent_kind: CommandTraceIntentKind::MoveCommand,
                    unit_ids: vec![trace.unit_id],
                    order: Some(trace.order),
                    outcome: unit_outcome,
                    path_waypoint_count: None,
                    error: trace.error,
                });
            }
        }

        let sequence = self.next_sequence();
        self.push_entry(CommandTraceEntry {
            tick,
            sequence,
            intent_kind: CommandTraceIntentKind::from_intent(intent),
            unit_ids: unit_ids.to_vec(),
            order: None,
            outcome,
            path_waypoint_count: None,
            error: None,
        });
    }

    pub fn record_command_resolve(&mut self, tick: u64, report: &CommandBufferResolveReport) {
        for success in &report.successes {
            let sequence = self.next_sequence();
            self.push_entry(CommandTraceEntry {
                tick,
                sequence,
                intent_kind: CommandTraceIntentKind::CommandResolve,
                unit_ids: vec![success.unit_id],
                order: Some(UnitOrder::MoveTo {
                    target: success.target,
                }),
                outcome: CommandTraceOutcome::OrderResolved,
                path_waypoint_count: Some(success.path_waypoint_count),
                error: None,
            });
        }
        for (unit_id, error) in &report.failures {
            let sequence = self.next_sequence();
            self.push_entry(CommandTraceEntry {
                tick,
                sequence,
                intent_kind: CommandTraceIntentKind::CommandResolve,
                unit_ids: vec![*unit_id],
                order: None,
                outcome: CommandTraceOutcome::ResolveFailed,
                path_waypoint_count: None,
                error: Some(*error),
            });
        }
    }

    pub fn store_dispatch_history(history: &mut IntentDispatchHistory, report: IntentDispatchReport) {
        history.report = Some(report);
    }
}

pub fn unit_ids_for_intent(intent: &ClientIntent) -> Vec<UnitId> {
    match intent {
        ClientIntent::SelectUnit { unit_id }
        | ClientIntent::ToggleUnitSelection { unit_id } => vec![*unit_id],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::IntentDispatchStatus;
    use crate::units::input::MoveOrderUnitTrace;
    use crate::world::{ChunkCoord, LocalPosition};

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn intent_dispatch_creates_command_trace_entry() {
        let mut buffer = CommandTraceBuffer::default();
        buffer.begin_tick(1);
        buffer.record_intent_dispatch(
            1,
            &ClientIntent::SelectUnit {
                unit_id: UnitId::new(7),
            },
            IntentDispatchStatus::Applied,
            &[UnitId::new(7)],
            None,
        );
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.latest().unwrap().intent_kind, CommandTraceIntentKind::SelectUnit);
    }

    #[test]
    fn move_command_intent_produces_unit_order_trace() {
        let mut buffer = CommandTraceBuffer::default();
        let unit_id = UnitId::new(3);
        let order = UnitOrder::MoveTo {
            target: pos(12.0, 8.0),
        };
        let move_report = MoveOrdersReport {
            issued: 1,
            failed: 0,
            unit_traces: vec![MoveOrderUnitTrace {
                unit_id,
                order,
                error: None,
            }],
        };
        buffer.begin_tick(4);
        buffer.record_intent_dispatch(
            4,
            &ClientIntent::MoveCommand {
                target: pos(12.0, 8.0),
            },
            IntentDispatchStatus::Applied,
            &[unit_id],
            Some(&move_report),
        );
        let order_entry = buffer
            .entries()
            .find(|entry| entry.outcome == CommandTraceOutcome::OrderQueued)
            .expect("queued order trace");
        assert_eq!(order_entry.order, Some(order));
        assert_eq!(order_entry.unit_ids, vec![unit_id]);
    }

    #[test]
    fn clear_selection_trace_records_applied_outcome() {
        let mut buffer = CommandTraceBuffer::default();
        buffer.begin_tick(2);
        buffer.record_intent_dispatch(
            2,
            &ClientIntent::ClearSelection,
            IntentDispatchStatus::Applied,
            &[],
            None,
        );
        assert_eq!(
            buffer.latest().unwrap().outcome,
            CommandTraceOutcome::Applied
        );
        assert_eq!(
            buffer.latest().unwrap().intent_kind,
            CommandTraceIntentKind::ClearSelection
        );
    }

    #[test]
    fn ignored_intent_records_ignored_outcome() {
        let mut buffer = CommandTraceBuffer::default();
        buffer.begin_tick(3);
        buffer.record_intent_dispatch(
            3,
            &ClientIntent::MoveCommand {
                target: pos(1.0, 1.0),
            },
            IntentDispatchStatus::Ignored,
            &[],
            None,
        );
        assert_eq!(
            buffer.latest().unwrap().outcome,
            CommandTraceOutcome::Ignored
        );
    }

    #[test]
    fn no_duplicate_trace_entries_per_tick() {
        let mut buffer = CommandTraceBuffer::default();
        buffer.begin_tick(5);
        buffer.record_intent_dispatch(
            5,
            &ClientIntent::SelectUnit {
                unit_id: UnitId::new(1),
            },
            IntentDispatchStatus::Applied,
            &[UnitId::new(1)],
            None,
        );
        buffer.record_intent_dispatch(
            5,
            &ClientIntent::SelectUnit {
                unit_id: UnitId::new(1),
            },
            IntentDispatchStatus::Applied,
            &[UnitId::new(1)],
            None,
        );
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn deterministic_dispatch_order_preserved_in_history() {
        let report = IntentDispatchReport {
            records: vec![
                IntentDispatchRecord {
                    intent: ClientIntent::ShiftModifier { pressed: true },
                    status: IntentDispatchStatus::Applied,
                },
                IntentDispatchRecord {
                    intent: ClientIntent::SelectUnit {
                        unit_id: UnitId::new(2),
                    },
                    status: IntentDispatchStatus::Applied,
                },
            ],
        };
        let mut history = IntentDispatchHistory::default();
        CommandTraceBuffer::store_dispatch_history(&mut history, report.clone());
        assert_eq!(history.report, Some(report));
    }

    #[test]
    fn resolve_success_includes_path_metadata() {
        let mut buffer = CommandTraceBuffer::default();
        buffer.begin_tick(6);
        buffer.record_command_resolve(
            6,
            &CommandBufferResolveReport {
                resolved: 1,
                failed: 0,
                failures: Vec::new(),
                successes: vec![CommandResolveSuccess {
                    unit_id: UnitId::new(9),
                    target: pos(20.0, 20.0),
                    path_waypoint_count: 5,
                }],
            },
        );
        let entry = buffer.latest().unwrap();
        assert_eq!(entry.path_waypoint_count, Some(5));
        assert_eq!(entry.outcome, CommandTraceOutcome::OrderResolved);
    }
}
