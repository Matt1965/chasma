//! Flush intent dispatch traces after the dispatcher runs.

use bevy::prelude::*;

use crate::client::ResolvedCommandFeedback;
use crate::debug::dispatch_pending::PendingDispatchTrace;
use crate::debug::{ClientBoundaryGuard, CommandTraceBuffer, IntentDispatchHistory};

/// Record dispatch traces and history from the pending batch (read-only simulation).
pub fn flush_intent_dispatch_trace(
    mut trace: ResMut<CommandTraceBuffer>,
    mut history: ResMut<IntentDispatchHistory>,
    mut pending: ResMut<PendingDispatchTrace>,
    mut resolved_command: ResMut<ResolvedCommandFeedback>,
    mut boundary: ResMut<ClientBoundaryGuard>,
) {
    if pending.records.is_empty() {
        pending.clear();
        boundary.end_intent_dispatch();
        return;
    }

    let tick = pending.tick;
    trace.begin_tick(tick);
    for record in pending.records.drain(..) {
        trace.record_intent_dispatch(
            tick,
            &record.intent,
            record.status,
            &record.unit_ids,
            record.move_report.as_ref(),
        );
    }
    if let Some(report) = pending.report.take() {
        CommandTraceBuffer::store_dispatch_history(&mut history, report);
    }
    if let Some(command_type) = pending.resolved_command.take() {
        if let Some(reason) = pending.unavailable_reason.take() {
            resolved_command.set_rejected(command_type, reason);
        } else {
            resolved_command.set_resolved(command_type);
        }
    }
    pending.command_tooltip = None;
    pending.clear();
    boundary.end_intent_dispatch();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{
        ClientIntent, IntentDispatchRecord, IntentDispatchReport, IntentDispatchStatus,
    };
    use crate::debug::dispatch_pending::PendingDispatchTraceRecord;
    use crate::debug::unit_ids_for_intent;
    use crate::world::{ChunkCoord, LocalPosition, UnitId, WorldPosition};

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn flush_creates_trace_from_pending_batch() {
        let mut trace = CommandTraceBuffer::default();
        let mut history = IntentDispatchHistory::default();
        let mut pending = PendingDispatchTrace {
            tick: 2,
            records: vec![PendingDispatchTraceRecord {
                intent: ClientIntent::ClearSelection,
                status: IntentDispatchStatus::Applied,
                unit_ids: vec![],
                move_report: None,
            }],
            report: Some(IntentDispatchReport {
                records: vec![IntentDispatchRecord {
                    intent: ClientIntent::ClearSelection,
                    status: IntentDispatchStatus::Applied,
                }],
            }),
            ..Default::default()
        };
        let mut boundary = ClientBoundaryGuard::default();
        boundary.dispatching_intents = true;

        trace.begin_tick(2);
        for record in pending.records.drain(..) {
            trace.record_intent_dispatch(
                2,
                &record.intent,
                record.status,
                &record.unit_ids,
                record.move_report.as_ref(),
            );
        }
        if let Some(report) = pending.report.take() {
            CommandTraceBuffer::store_dispatch_history(&mut history, report);
        }
        pending.clear();
        boundary.end_intent_dispatch();

        assert_eq!(trace.len(), 1);
        assert!(history.report.is_some());
    }

    #[test]
    fn overlay_toggle_state_does_not_require_world_mutation() {
        let intent = ClientIntent::MoveCommand {
            target: pos(1.0, 2.0),
        };
        let ids = unit_ids_for_intent(&intent);
        assert!(ids.is_empty());
    }
}
