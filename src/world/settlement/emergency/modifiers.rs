//! Apply authored emergency modifiers at earliest planning layers (SA8).
//!
//! Need pressure: SA2. Response score/availability: SA3.
//! Do not re-apply the same bonus at SA4.

use super::catalog::EmergencyCatalog;
use super::definition::EmergencyDefinition;
use crate::world::settlement::response::{ResponseDefinition, ResponseId};
use crate::world::settlement::state::{ActiveEmergencyInstance, SettlementState};
use crate::world::task::TaskPriority;

/// Additive need-pressure bump from active emergencies (severity-scaled).
pub fn emergency_need_pressure_delta(
    state: &SettlementState,
    catalog: &EmergencyCatalog,
    need_id: &str,
) -> f32 {
    if !state.policies.auto_emergency_response && !has_manual_force(state) {
        // Still apply if instances exist from manual force / continuity.
    }
    if !state.policies.auto_production_reprioritize && !has_manual_force(state) {
        // Pressure reweight is part of production/planning prioritization.
        // Manual force still applies modifiers so forced emergencies matter.
        if !state.emergencies.instances.iter().any(|i| i.manual_force) {
            return 0.0;
        }
    }
    let mut delta = 0.0;
    for instance in &state.emergencies.instances {
        let Some(def) = catalog.get_str(&instance.emergency_id) else {
            continue;
        };
        for m in &def.need_pressure_modifiers {
            if m.need_id.as_str() == need_id {
                delta += m.pressure_delta_at_full * instance.severity.clamp(0.0, 1.0);
            }
        }
    }
    delta
}

/// Response score delta from active emergencies (severity-scaled). Distinct from need pressure.
pub fn emergency_response_score_delta(
    state: &SettlementState,
    catalog: &EmergencyCatalog,
    definition: &ResponseDefinition,
) -> f32 {
    if !state.policies.auto_production_reprioritize
        && !state.emergencies.instances.iter().any(|i| i.manual_force)
    {
        return 0.0;
    }
    let mut delta = 0.0;
    for instance in &state.emergencies.instances {
        let Some(def) = catalog.get_str(&instance.emergency_id) else {
            continue;
        };
        let severity = instance.severity.clamp(0.0, 1.0);
        for m in &def.response_score_modifiers {
            let matches_id = m
                .response_id
                .as_ref()
                .is_some_and(|id| id == &definition.id);
            let matches_tag = m.response_tag.as_ref().is_some_and(|tag| {
                definition.ai_tags.iter().any(|t| t == tag)
            });
            if matches_id || matches_tag {
                delta += m.score_delta_at_full * severity;
            }
        }
    }
    delta
}

/// True when an active emergency blocks this response.
pub fn emergency_blocks_response(
    state: &SettlementState,
    catalog: &EmergencyCatalog,
    definition: &ResponseDefinition,
) -> Option<String> {
    for instance in &state.emergencies.instances {
        let Some(def) = catalog.get_str(&instance.emergency_id) else {
            continue;
        };
        if def
            .block_response_ids
            .iter()
            .any(|id| id == &definition.id)
        {
            return Some(format!(
                "blocked by emergency `{}`",
                instance.emergency_id
            ));
        }
        for tag in &def.block_response_tags {
            if definition.ai_tags.iter().any(|t| t == tag) {
                return Some(format!(
                    "tag `{tag}` blocked by emergency `{}`",
                    instance.emergency_id
                ));
            }
        }
    }
    None
}

/// True when response requires unlock and an active emergency unlocks it.
pub fn emergency_unlocks_response(
    state: &SettlementState,
    catalog: &EmergencyCatalog,
    response_id: &ResponseId,
) -> bool {
    for instance in &state.emergencies.instances {
        let Some(def) = catalog.get_str(&instance.emergency_id) else {
            continue;
        };
        if def.unlock_response_ids.iter().any(|id| id == response_id) {
            return true;
        }
    }
    false
}

/// Whether response is emergency-only (ai_tag) and currently unlocked.
pub fn emergency_only_gate(
    state: &SettlementState,
    catalog: &EmergencyCatalog,
    definition: &ResponseDefinition,
) -> Result<(), String> {
    let is_emergency_only = definition.ai_tags.iter().any(|t| t == "emergency_only");
    if !is_emergency_only {
        return Ok(());
    }
    if emergency_unlocks_response(state, catalog, &definition.id) {
        Ok(())
    } else {
        Err("emergency_only response not unlocked".into())
    }
}

/// Best interruption relaxation from active emergencies for a settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmergencyPreemptRelaxation {
    pub min_stick_ticks: u64,
    pub min_priority_rank_gap: u8,
    pub max_interruptible: TaskPriority,
}

pub fn emergency_preempt_relaxation(
    state: &SettlementState,
    catalog: &EmergencyCatalog,
) -> Option<EmergencyPreemptRelaxation> {
    if !state.policies.auto_task_interruption {
        return None;
    }
    let mut best: Option<EmergencyPreemptRelaxation> = None;
    for instance in &state.emergencies.instances {
        let Some(def) = catalog.get_str(&instance.emergency_id) else {
            continue;
        };
        if !def.interruption.allow_interruption {
            continue;
        }
        let candidate = EmergencyPreemptRelaxation {
            min_stick_ticks: def.interruption.min_stick_ticks.unwrap_or(20),
            min_priority_rank_gap: def.interruption.min_priority_rank_gap.unwrap_or(1),
            max_interruptible: def
                .interruption
                .max_interruptible_priority
                .unwrap_or(TaskPriority::Normal),
        };
        best = Some(match best {
            None => candidate,
            Some(prev) => EmergencyPreemptRelaxation {
                min_stick_ticks: prev.min_stick_ticks.min(candidate.min_stick_ticks),
                min_priority_rank_gap: prev
                    .min_priority_rank_gap
                    .min(candidate.min_priority_rank_gap),
                max_interruptible: if candidate.max_interruptible.rank()
                    > prev.max_interruptible.rank()
                {
                    // Higher rank value = lower priority ceiling... TaskPriority::Normal=2, Low=3
                    // max_interruptible means "can interrupt up to this priority" — higher enum rank = can interrupt lower importance.
                    // PlayerAssigned=0 High=1 Normal=2 Low=3. "max interruptible Normal" means can interrupt Normal and Low.
                    // Prefer the more permissive (higher rank number).
                    candidate.max_interruptible
                } else {
                    prev.max_interruptible
                },
            },
        });
    }
    best
}

/// Optionally bump task priority one tier for emergency-tagged emits.
pub fn emergency_bump_task_priority(
    state: &SettlementState,
    catalog: &EmergencyCatalog,
    response_id: &str,
    priority: TaskPriority,
) -> TaskPriority {
    if priority == TaskPriority::PlayerAssigned || priority == TaskPriority::High {
        return priority;
    }
    let mut bump = false;
    for instance in &state.emergencies.instances {
        let Some(def) = catalog.get_str(&instance.emergency_id) else {
            continue;
        };
        for m in &def.task_priority_modifiers {
            if !m.bump_one_tier {
                continue;
            }
            if let Some(tag) = &m.response_tag {
                if response_id.contains(tag) || tag == "food" && response_id.contains("food") {
                    bump = true;
                }
                if tag == "defense" && response_id.contains("defend") {
                    bump = true;
                }
            } else {
                bump = true;
            }
        }
    }
    if !bump {
        return priority;
    }
    match priority {
        TaskPriority::Low => TaskPriority::Normal,
        TaskPriority::Normal => TaskPriority::High,
        other => other,
    }
}

fn has_manual_force(state: &SettlementState) -> bool {
    state.emergencies.instances.iter().any(|i| i.manual_force)
}

/// Active definitions paired with instances (sorted by emergency id).
pub fn active_definitions<'a>(
    state: &'a SettlementState,
    catalog: &'a EmergencyCatalog,
) -> Vec<(&'a ActiveEmergencyInstance, &'a EmergencyDefinition)> {
    let mut out = Vec::new();
    for instance in &state.emergencies.instances {
        if let Some(def) = catalog.get_str(&instance.emergency_id) {
            out.push((instance, def));
        }
    }
    out.sort_by(|a, b| a.0.emergency_id.cmp(&b.0.emergency_id));
    out
}
