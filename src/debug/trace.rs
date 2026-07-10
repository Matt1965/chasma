//! Command trace capture for intent → command observability (ADR-039 U-UI3).

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::client::{ClientIntent, IntentDispatchReport, IntentDispatchStatus};
use crate::units::input::MoveOrdersReport;
use crate::world::{
    BlockedMovementReason, CommandBufferResolveReport, CombatAiReport, CombatAiTraceOutcome,
    CombatEngagementReport, CombatEngagementStatus, CombatStrikeEvent, CombatStrikeReport,
    ProjectileEvent, ProjectileReport, UnitDeathEvent, UnitDeathReport, UnitId, UnitMovementTrace,
    UnitOrder, UnitOrderError,
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
    CombatRangeReady,
    CombatChasing,
    CombatTargetInvalid,
    CombatPathUnavailable,
    CombatTerrainUnavailable,
    CombatAttackMoveAcquired,
    AttackOrderAccepted,
    AttackOrderRejected,
    AttackEnteredRange,
    CombatAttackWindupStarted,
    CombatAttackStrikeApplied,
    CombatAttackStrikeMissed,
    CombatAttackRecoveryStarted,
    CombatAttackCooldownStarted,
    CombatUnsupportedProjectileMode,
    CombatAttackCycleResetRetarget,
    CombatAttackCycleClearedInvalidTarget,
    CombatAttackCycleClearedOrderCancelled,
    CombatAttackStrikeSkippedStateMismatch,
    ProjectileSpawned,
    ProjectileHit,
    ProjectileExpired,
    ProjectileImpactRejected,
    ProjectileDamageApplied,
    UnitDied,
    UnitRemovalQueued,
    UnitRemoved,
    TargetClearedDueToDeath,
    AiTargetAcquired,
    AiScanNoTarget,
    AiScanSkippedBudget,
    UnitMovementBlocked,
    CommandRejected,
    HealthBarShown,
    HealthBarHidden,
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
    PaletteCommand,
    ShiftModifier,
    CommandResolve,
    CombatEngagement,
    CombatStrike,
    Projectile,
    UnitDeath,
    CombatAi,
    UnitMovement,
    HealthBar,
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
            ClientIntent::PaletteCommand { .. } => Self::PaletteCommand,
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
    pub combat_status: Option<CombatEngagementStatus>,
    pub center_distance_meters: Option<f32>,
    pub edge_distance_meters: Option<f32>,
    pub weapon_range_meters: Option<f32>,
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

    /// Presentation-only trace rows (health bars, optional debug).
    pub fn push_presentation_entry(
        &mut self,
        tick: u64,
        intent_kind: CommandTraceIntentKind,
        unit_ids: Vec<UnitId>,
        outcome: CommandTraceOutcome,
    ) {
        let sequence = self.next_sequence();
        self.push_entry(CommandTraceEntry {
            tick,
            sequence,
            intent_kind,
            unit_ids,
            order: None,
            outcome,
            path_waypoint_count: None,
            error: None,
            combat_status: None,
            center_distance_meters: None,
            edge_distance_meters: None,
            weapon_range_meters: None,
        });
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
            IntentDispatchStatus::Rejected(_) => CommandTraceOutcome::CommandRejected,
        };

        if let Some(report) = move_report {
            for trace in &report.unit_traces {
                let unit_outcome = if trace.error.is_some() {
                    if matches!(trace.order, UnitOrder::Attack { .. }) {
                        CommandTraceOutcome::AttackOrderRejected
                    } else {
                        CommandTraceOutcome::OrderFailed
                    }
                } else if matches!(trace.order, UnitOrder::Attack { .. }) {
                    CommandTraceOutcome::AttackOrderAccepted
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
                    combat_status: None,
                    center_distance_meters: None,
                    edge_distance_meters: None,
                    weapon_range_meters: None,
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
            combat_status: None,
            center_distance_meters: None,
            edge_distance_meters: None,
            weapon_range_meters: None,
        });
    }

    pub fn record_combat_engagement(&mut self, tick: u64, report: &CombatEngagementReport) {
        for trace in &report.traces {
            let outcome = match trace.status {
                CombatEngagementStatus::InRangeReady => CommandTraceOutcome::AttackEnteredRange,
                CombatEngagementStatus::OutOfRangeChasing => CommandTraceOutcome::CombatChasing,
                CombatEngagementStatus::TargetInvalid => CommandTraceOutcome::CombatTargetInvalid,
                CombatEngagementStatus::PathUnavailable => CommandTraceOutcome::CombatPathUnavailable,
                CombatEngagementStatus::TerrainUnavailable => {
                    CommandTraceOutcome::CombatTerrainUnavailable
                }
                CombatEngagementStatus::AttackMoveAcquired => {
                    CommandTraceOutcome::CombatAttackMoveAcquired
                }
                CombatEngagementStatus::MissingWeapon
                | CombatEngagementStatus::AttackMoveMoving => continue,
            };
            let sequence = self.next_sequence();
            self.push_entry(CommandTraceEntry {
                tick,
                sequence,
                intent_kind: CommandTraceIntentKind::CombatEngagement,
                unit_ids: vec![trace.unit_id],
                order: None,
                outcome,
                path_waypoint_count: None,
                error: None,
                combat_status: Some(trace.status),
                center_distance_meters: trace.center_distance_meters,
                edge_distance_meters: trace.edge_distance_meters,
                weapon_range_meters: trace.weapon_range_meters,
            });
        }
    }

    pub fn record_combat_strike(&mut self, tick: u64, report: &CombatStrikeReport) {
        for trace in &report.traces {
            let outcome = match &trace.event {
                CombatStrikeEvent::AttackWindupStarted => {
                    CommandTraceOutcome::CombatAttackWindupStarted
                }
                CombatStrikeEvent::AttackStrikeApplied { .. } => {
                    CommandTraceOutcome::CombatAttackStrikeApplied
                }
                CombatStrikeEvent::AttackStrikeMissedInvalidTarget => {
                    CommandTraceOutcome::CombatAttackStrikeMissed
                }
                CombatStrikeEvent::AttackRecoveryStarted => {
                    CommandTraceOutcome::CombatAttackRecoveryStarted
                }
                CombatStrikeEvent::AttackCooldownStarted => {
                    CommandTraceOutcome::CombatAttackCooldownStarted
                }
                CombatStrikeEvent::UnsupportedProjectileMode => {
                    CommandTraceOutcome::CombatUnsupportedProjectileMode
                }
                CombatStrikeEvent::AttackCycleResetRetarget { .. } => {
                    CommandTraceOutcome::CombatAttackCycleResetRetarget
                }
                CombatStrikeEvent::AttackCycleClearedInvalidTarget { .. } => {
                    CommandTraceOutcome::CombatAttackCycleClearedInvalidTarget
                }
                CombatStrikeEvent::AttackCycleClearedOrderCancelled => {
                    CommandTraceOutcome::CombatAttackCycleClearedOrderCancelled
                }
                CombatStrikeEvent::AttackStrikeSkippedStateMismatch { .. } => {
                    CommandTraceOutcome::CombatAttackStrikeSkippedStateMismatch
                }
            };
            let sequence = self.next_sequence();
            self.push_entry(CommandTraceEntry {
                tick,
                sequence,
                intent_kind: CommandTraceIntentKind::CombatStrike,
                unit_ids: vec![trace.attacker_id],
                order: None,
                outcome,
                path_waypoint_count: None,
                error: None,
                combat_status: None,
                center_distance_meters: None,
                edge_distance_meters: None,
                weapon_range_meters: None,
            });
        }
    }

    pub fn record_projectile(&mut self, tick: u64, report: &ProjectileReport) {
        for trace in &report.traces {
            let outcome = match &trace.event {
                ProjectileEvent::Spawned => CommandTraceOutcome::ProjectileSpawned,
                ProjectileEvent::Hit => CommandTraceOutcome::ProjectileHit,
                ProjectileEvent::Expired => CommandTraceOutcome::ProjectileExpired,
                ProjectileEvent::ImpactRejected { .. } => {
                    CommandTraceOutcome::ProjectileImpactRejected
                }
                ProjectileEvent::DamageApplied { .. } => {
                    CommandTraceOutcome::ProjectileDamageApplied
                }
            };
            let sequence = self.next_sequence();
            self.push_entry(CommandTraceEntry {
                tick,
                sequence,
                intent_kind: CommandTraceIntentKind::Projectile,
                unit_ids: vec![trace.source_unit_id],
                order: None,
                outcome,
                path_waypoint_count: None,
                error: None,
                combat_status: None,
                center_distance_meters: None,
                edge_distance_meters: None,
                weapon_range_meters: None,
            });
        }
    }

    pub fn record_combat_ai(&mut self, tick: u64, report: &CombatAiReport) {
        for trace in &report.traces {
            let outcome = match trace.outcome {
                CombatAiTraceOutcome::AiTargetAcquired => CommandTraceOutcome::AiTargetAcquired,
                CombatAiTraceOutcome::AiScanNoTarget => CommandTraceOutcome::AiScanNoTarget,
                CombatAiTraceOutcome::AiScanSkippedBudget => CommandTraceOutcome::AiScanSkippedBudget,
                CombatAiTraceOutcome::AiScanSkippedInterval => continue,
            };
            let mut unit_ids = vec![trace.unit_id];
            if let Some(target) = trace.target {
                unit_ids.push(target);
            }
            let sequence = self.next_sequence();
            self.push_entry(CommandTraceEntry {
                tick,
                sequence,
                intent_kind: CommandTraceIntentKind::CombatAi,
                unit_ids,
                order: trace.target.map(|target| UnitOrder::Attack { target }),
                outcome,
                path_waypoint_count: None,
                error: None,
                combat_status: None,
                center_distance_meters: None,
                edge_distance_meters: None,
                weapon_range_meters: None,
            });
        }
    }

    pub fn record_unit_death(&mut self, tick: u64, report: &UnitDeathReport) {
        for trace in &report.traces {
            let outcome = match &trace.event {
                UnitDeathEvent::UnitDied { .. } => CommandTraceOutcome::UnitDied,
                UnitDeathEvent::UnitRemovalQueued { .. } => CommandTraceOutcome::UnitRemovalQueued,
                UnitDeathEvent::UnitRemoved { .. } => CommandTraceOutcome::UnitRemoved,
                UnitDeathEvent::TargetClearedDueToDeath { .. } => {
                    CommandTraceOutcome::TargetClearedDueToDeath
                }
            };
            let sequence = self.next_sequence();
            self.push_entry(CommandTraceEntry {
                tick,
                sequence,
                intent_kind: CommandTraceIntentKind::UnitDeath,
                unit_ids: vec![trace.unit_id],
                order: None,
                outcome,
                path_waypoint_count: None,
                error: None,
                combat_status: None,
                center_distance_meters: None,
                edge_distance_meters: None,
                weapon_range_meters: None,
            });
        }
    }

    pub fn record_unit_movement(&mut self, tick: u64, traces: &[UnitMovementTrace]) {
        for trace in traces {
            let sequence = self.next_sequence();
            self.push_entry(CommandTraceEntry {
                tick,
                sequence,
                intent_kind: CommandTraceIntentKind::UnitMovement,
                unit_ids: vec![trace.unit_id],
                order: Some(UnitOrder::MoveTo {
                    target: trace.target,
                }),
                outcome: CommandTraceOutcome::UnitMovementBlocked,
                path_waypoint_count: Some(trace.waypoint_index as u32),
                error: movement_block_order_error(trace.reason),
                combat_status: None,
                center_distance_meters: None,
                edge_distance_meters: None,
                weapon_range_meters: None,
            });
        }
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
                combat_status: None,
                center_distance_meters: None,
                edge_distance_meters: None,
                weapon_range_meters: None,
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
                combat_status: None,
                center_distance_meters: None,
                edge_distance_meters: None,
                weapon_range_meters: None,
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

fn movement_block_order_error(reason: BlockedMovementReason) -> Option<UnitOrderError> {
    Some(match reason {
        BlockedMovementReason::TerrainUnavailable => UnitOrderError::PathTerrainUnavailable,
        BlockedMovementReason::BlockedByDoodad
        | BlockedMovementReason::SlopeTooSteep
        | BlockedMovementReason::SlopeUnavailable
        | BlockedMovementReason::DestinationBlocked
        | BlockedMovementReason::TargetUnavailable => UnitOrderError::PathGoalBlocked,
        BlockedMovementReason::PathUnavailable | BlockedMovementReason::InvalidWaypoint => {
            UnitOrderError::NoPath
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{IntentDispatchRecord, IntentDispatchStatus};
    use crate::units::input::MoveOrderUnitTrace;
    use crate::world::{
        ChunkCoord, CommandResolveSuccess, LocalPosition, WorldPosition,
    };

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

    #[test]
    fn rejected_attack_order_records_error_in_trace() {
        let mut buffer = CommandTraceBuffer::default();
        buffer.begin_tick(9);
        let report = MoveOrdersReport {
            issued: 0,
            failed: 1,
            unit_traces: vec![MoveOrderUnitTrace {
                unit_id: UnitId::new(1),
                order: UnitOrder::Attack {
                    target: UnitId::new(1),
                },
                error: Some(UnitOrderError::SelfTarget),
            }],
        };
        buffer.record_intent_dispatch(
            9,
            &ClientIntent::ContextualCommand {
                target: crate::client::commands::CommandTarget::Unit {
                    unit_id: UnitId::new(1),
                },
            },
            IntentDispatchStatus::Applied,
            &[UnitId::new(1)],
            Some(&report),
        );
        let attack_trace = buffer
            .entries()
            .find(|entry| entry.error == Some(UnitOrderError::SelfTarget))
            .expect("attack rejection trace");
        assert_eq!(attack_trace.unit_ids, vec![UnitId::new(1)]);
        assert!(matches!(
            attack_trace.order,
            Some(UnitOrder::Attack { target }) if target == UnitId::new(1)
        ));
    }
}
