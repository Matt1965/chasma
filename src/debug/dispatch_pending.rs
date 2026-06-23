//! Pending client dispatch data for trace flush (keeps dispatch system param count low).

use bevy::prelude::*;

use crate::client::commands::CommandType;
use crate::client::{ClientIntent, IntentDispatchReport, IntentDispatchStatus};
use crate::units::input::MoveOrdersReport;
use crate::world::UnitId;

/// One dispatch-side trace payload awaiting flush.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingDispatchTraceRecord {
    pub intent: ClientIntent,
    pub status: IntentDispatchStatus,
    pub unit_ids: Vec<UnitId>,
    pub move_report: Option<MoveOrdersReport>,
}

/// Batch written by dispatch and consumed by trace flush.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct PendingDispatchTrace {
    pub tick: u64,
    pub records: Vec<PendingDispatchTraceRecord>,
    pub report: Option<IntentDispatchReport>,
    pub resolved_command: Option<CommandType>,
    pub command_tooltip: Option<String>,
}

impl PendingDispatchTrace {
    pub fn clear(&mut self) {
        self.tick = 0;
        self.records.clear();
        self.report = None;
        self.resolved_command = None;
        self.command_tooltip = None;
    }
}
