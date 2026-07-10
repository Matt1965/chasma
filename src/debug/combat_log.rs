//! Combat trace formatting for dev presentation (ADR-061 C8).

use crate::world::UnitId;

use super::trace::{CommandTraceBuffer, CommandTraceEntry, CommandTraceOutcome};

/// Outcomes surfaced in combat log / inspector views.
pub const COMBAT_LOG_OUTCOMES: &[CommandTraceOutcome] = &[
    CommandTraceOutcome::AttackOrderAccepted,
    CommandTraceOutcome::AttackOrderRejected,
    CommandTraceOutcome::AttackEnteredRange,
    CommandTraceOutcome::CombatAttackStrikeApplied,
    CommandTraceOutcome::ProjectileSpawned,
    CommandTraceOutcome::ProjectileHit,
    CommandTraceOutcome::ProjectileDamageApplied,
    CommandTraceOutcome::UnitDied,
    CommandTraceOutcome::UnitRemoved,
];

pub fn is_combat_log_outcome(outcome: CommandTraceOutcome) -> bool {
    COMBAT_LOG_OUTCOMES.contains(&outcome)
}

pub fn outcome_label(outcome: CommandTraceOutcome) -> &'static str {
    match outcome {
        CommandTraceOutcome::AttackOrderAccepted => "AttackOrderAccepted",
        CommandTraceOutcome::AttackOrderRejected => "AttackOrderRejected",
        CommandTraceOutcome::AttackEnteredRange => "AttackEnteredRange",
        CommandTraceOutcome::CombatAttackStrikeApplied => "AttackStrikeApplied",
        CommandTraceOutcome::ProjectileSpawned => "ProjectileSpawned",
        CommandTraceOutcome::ProjectileHit => "ProjectileHit",
        CommandTraceOutcome::ProjectileDamageApplied => "ProjectileDamageApplied",
        CommandTraceOutcome::UnitDied => "UnitDied",
        CommandTraceOutcome::UnitRemoved => "UnitRemoved",
        _ => "Other",
    }
}

pub fn format_trace_entry(entry: &CommandTraceEntry) -> String {
    let units: Vec<_> = entry.unit_ids.iter().map(|id| id.raw().to_string()).collect();
    format!(
        "t{} #{}{} {:?}",
        entry.tick,
        entry.sequence,
        if units.is_empty() {
            String::new()
        } else {
            format!(" units=[{}]", units.join(","))
        },
        outcome_label(entry.outcome)
    )
}

/// Recent combat log lines in deterministic buffer order (oldest first among matches).
pub fn recent_combat_log_lines(
    trace: &CommandTraceBuffer,
    unit_filter: Option<UnitId>,
    limit: usize,
) -> Vec<String> {
    let mut lines: Vec<_> = trace
        .entries()
        .filter(|entry| is_combat_log_outcome(entry.outcome))
        .filter(|entry| {
            unit_filter.is_none_or(|unit_id| entry.unit_ids.contains(&unit_id))
        })
        .map(format_trace_entry)
        .collect();
    if lines.len() > limit {
        lines = lines.split_off(lines.len() - limit);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debug::trace::CommandTraceBuffer;
    use crate::world::{CombatEngagementReport, CombatEngagementStatus, CombatEngagementTrace};

    #[test]
    fn combat_log_records_deterministic_order() {
        let mut buffer = CommandTraceBuffer::default();
        buffer.begin_tick(1);
        buffer.record_combat_engagement(
            1,
            &CombatEngagementReport {
                traces: vec![CombatEngagementTrace {
                    unit_id: UnitId::new(1),
                    status: CombatEngagementStatus::InRangeReady,
                    target: None,
                    center_distance_meters: Some(2.0),
                    edge_distance_meters: Some(1.0),
                    weapon_range_meters: Some(8.0),
                    chase_destination: None,
                }],
            },
        );
        buffer.record_combat_strike(
            1,
            &crate::world::CombatStrikeReport {
                traces: vec![crate::world::CombatStrikeTrace {
                    attacker_id: UnitId::new(1),
                    target_id: UnitId::new(2),
                    weapon_id: crate::world::WeaponDefinitionId::new("weapon_fists"),
                    event: crate::world::CombatStrikeEvent::AttackStrikeApplied {
                        damage: 4.0,
                        target_hp_before: 5,
                        target_hp_after: 1,
                    },
                }],
            },
        );
        let lines = recent_combat_log_lines(&buffer, None, 10);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("AttackEnteredRange"));
        assert!(lines[1].contains("AttackStrikeApplied"));
    }
}
