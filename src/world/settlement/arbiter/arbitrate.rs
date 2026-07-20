//! Settlement Response Arbiter — CandidateResponses → SettlementIntent (SA4).
//!
//! Selects strategic intent only. Never executes.

use crate::world::settlement::needs::SettlementNeedEvaluation;
use crate::world::settlement::response::{
    CandidateResponse, ResponseCatalog, ResponseType, SettlementResponseCandidates,
};
use crate::world::settlement::state::SettlementState;
use crate::world::settlement::SettlementId;
use crate::world::{BuildingLifecycleState, WorldData};

use super::intent::{
    IntentId, IntentPersistence, IntentRejectionReason, RejectedIntentCandidate, SettlementIntent,
    SettlementIntentPlan,
};

/// Maximum simultaneous intents a settlement may hold.
pub const MAX_SETTLEMENT_INTENTS: usize = 6;

/// Minimum arbitration score to select (filters noise).
pub const MIN_ARBITRATION_SCORE: f32 = 1.0;

/// Max intents per need when pressure is high (>= HIGH_PRESSURE_THRESHOLD).
pub const MAX_INTENTS_PER_NEED_HIGH: usize = 2;

/// Max intents per need when pressure is moderate.
pub const MAX_INTENTS_PER_NEED_NORMAL: usize = 1;

pub const HIGH_PRESSURE_THRESHOLD: u8 = 40;

/// Read-only arbitration context.
pub struct ArbitrationContext<'a> {
    pub world: &'a WorldData,
    pub response_catalog: &'a ResponseCatalog,
    pub settlement_id: SettlementId,
    pub state: &'a SettlementState,
    pub need_evaluation: &'a SettlementNeedEvaluation,
    pub candidates: &'a SettlementResponseCandidates,
    pub simulation_tick: u64,
}

/// Evaluate, rank, and select multiple CandidateResponses into SettlementIntent.
pub fn arbitrate_settlement_intent(ctx: &ArbitrationContext<'_>) -> SettlementIntentPlan {
    let mut plan = SettlementIntentPlan {
        settlement_id: ctx.settlement_id,
        planned_tick: ctx.simulation_tick,
        source_response_tick: ctx.candidates.evaluated_tick,
        source_need_tick: ctx.need_evaluation.evaluated_tick,
        intents: Vec::new(),
        rejected: Vec::new(),
        diagnostics: Vec::new(),
    };

    let workload = estimate_workload(ctx);
    plan.diagnostics
        .push(format!("workload_proxy={workload:.1}"));

    let mut ranked: Vec<RankedCandidate<'_>> = Vec::new();
    for candidate in &ctx.candidates.candidates {
        let pressure = ctx
            .need_evaluation
            .snapshot(&candidate.need_id)
            .map(|s| s.pressure)
            .unwrap_or(0);
        let arb_score = arbitration_score(candidate, pressure, ctx.state, workload);

        if !candidate.priority_score.is_finite() || !arb_score.is_finite() {
            plan.rejected.push(RejectedIntentCandidate {
                response_id: candidate.response_id.clone(),
                need_id: candidate.need_id.clone(),
                candidate_score: candidate.priority_score,
                arbitration_score: arb_score,
                reason: IntentRejectionReason::InvalidScore,
            });
            continue;
        }

        if ctx.response_catalog.get(&candidate.response_id).is_none() {
            plan.rejected.push(RejectedIntentCandidate {
                response_id: candidate.response_id.clone(),
                need_id: candidate.need_id.clone(),
                candidate_score: candidate.priority_score,
                arbitration_score: arb_score,
                reason: IntentRejectionReason::UnknownResponse,
            });
            continue;
        }

        if !candidate.is_available() {
            plan.rejected.push(RejectedIntentCandidate {
                response_id: candidate.response_id.clone(),
                need_id: candidate.need_id.clone(),
                candidate_score: candidate.priority_score,
                arbitration_score: arb_score,
                reason: IntentRejectionReason::Unavailable,
            });
            continue;
        }

        if pressure == 0 {
            plan.rejected.push(RejectedIntentCandidate {
                response_id: candidate.response_id.clone(),
                need_id: candidate.need_id.clone(),
                candidate_score: candidate.priority_score,
                arbitration_score: arb_score,
                reason: IntentRejectionReason::ZeroPressure,
            });
            continue;
        }

        if arb_score < MIN_ARBITRATION_SCORE {
            plan.rejected.push(RejectedIntentCandidate {
                response_id: candidate.response_id.clone(),
                need_id: candidate.need_id.clone(),
                candidate_score: candidate.priority_score,
                arbitration_score: arb_score,
                reason: IntentRejectionReason::BelowScoreThreshold,
            });
            continue;
        }

        ranked.push(RankedCandidate {
            candidate,
            pressure,
            arb_score,
        });
    }

    ranked.sort_by(|a, b| {
        b.arb_score
            .partial_cmp(&a.arb_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.candidate
                    .response_id
                    .as_str()
                    .cmp(b.candidate.response_id.as_str())
            })
            .then_with(|| a.candidate.need_id.as_str().cmp(b.candidate.need_id.as_str()))
    });

    let mut selected_response_ids = std::collections::BTreeSet::new();
    let mut per_need_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut selected_types_by_need: std::collections::BTreeMap<String, Vec<ResponseType>> =
        std::collections::BTreeMap::new();
    let mut intent_index = 0u32;

    for entry in ranked {
        let need_key = entry.candidate.need_id.as_str().to_string();
        let max_for_need = if entry.pressure >= HIGH_PRESSURE_THRESHOLD {
            MAX_INTENTS_PER_NEED_HIGH
        } else {
            MAX_INTENTS_PER_NEED_NORMAL
        };
        let need_count = *per_need_counts.get(&need_key).unwrap_or(&0);

        if plan.intents.len() >= MAX_SETTLEMENT_INTENTS {
            plan.rejected.push(reject(
                entry.candidate,
                entry.arb_score,
                IntentRejectionReason::GlobalBudgetFull,
            ));
            continue;
        }

        if need_count >= max_for_need {
            plan.rejected.push(reject(
                entry.candidate,
                entry.arb_score,
                IntentRejectionReason::NeedSlotFull,
            ));
            continue;
        }

        if selected_response_ids.contains(entry.candidate.response_id.as_str()) {
            plan.rejected.push(reject(
                entry.candidate,
                entry.arb_score,
                IntentRejectionReason::ConflictWithSelected(
                    entry.candidate.response_id.as_str().to_string(),
                ),
            ));
            continue;
        }

        if let Some(conflict) =
            find_type_conflict(entry.candidate.response_type, selected_types_by_need.get(&need_key))
        {
            plan.rejected.push(reject(
                entry.candidate,
                entry.arb_score,
                IntentRejectionReason::ConflictWithSelected(conflict),
            ));
            continue;
        }

        let persistence = if entry.pressure >= 80 {
            IntentPersistence::UntilPressureLow
        } else {
            IntentPersistence::Ephemeral
        };

        let reasoning = format!(
            "pressure={} candidate_score={:.1} arb_score={:.1} workload={:.1}",
            entry.pressure, entry.candidate.priority_score, entry.arb_score, workload
        );

        let intent = SettlementIntent {
            intent_id: IntentId::compose(
                ctx.settlement_id,
                &entry.candidate.response_id,
                &entry.candidate.need_id,
                ctx.simulation_tick,
                intent_index,
            ),
            source_need: entry.candidate.need_id.clone(),
            chosen_response: entry.candidate.response_id.clone(),
            response_type: entry.candidate.response_type,
            priority: entry.arb_score,
            desired_persistence: persistence,
            reasoning,
            diagnostics: entry.candidate.diagnostics.clone(),
            ai_seams: Vec::new(),
        };

        selected_response_ids.insert(entry.candidate.response_id.as_str().to_string());
        per_need_counts.insert(need_key.clone(), need_count + 1);
        selected_types_by_need
            .entry(need_key)
            .or_default()
            .push(entry.candidate.response_type);
        plan.intents.push(intent);
        intent_index += 1;
    }

    plan.diagnostics.push(format!(
        "selected={} rejected={}",
        plan.intents.len(),
        plan.rejected.len()
    ));
    plan
}

struct RankedCandidate<'a> {
    candidate: &'a CandidateResponse,
    pressure: u8,
    arb_score: f32,
}

fn reject(
    candidate: &CandidateResponse,
    arb_score: f32,
    reason: IntentRejectionReason,
) -> RejectedIntentCandidate {
    RejectedIntentCandidate {
        response_id: candidate.response_id.clone(),
        need_id: candidate.need_id.clone(),
        candidate_score: candidate.priority_score,
        arbitration_score: arb_score,
        reason,
    }
}

/// Generic arbitration score. Intentionally simple.
pub fn arbitration_score(
    candidate: &CandidateResponse,
    pressure: u8,
    state: &SettlementState,
    workload: f32,
) -> f32 {
    if !candidate.is_available() {
        return 0.0;
    }
    // SA8: emergency effects already applied at SA2 (pressure) and SA3 (response score).
    // Do not double-count with a second emergency bonus here.
    let mut score = candidate.priority_score + f32::from(pressure) * 2.0;
    score += policy_bonus(state, candidate);
    // Soft workload penalty — does not hard-block high pressure.
    score -= (workload * 0.5).min(40.0);
    score.max(0.0)
}

fn policy_bonus(state: &SettlementState, candidate: &CandidateResponse) -> f32 {
    let mut bonus = 0.0;
    match candidate.response_type {
        ResponseType::Expand if state.policies.expansion_enabled => bonus += 5.0,
        ResponseType::Expand => bonus -= 20.0,
        ResponseType::Defend => bonus += f32::from(state.policies.aggression) / 32.0,
        ResponseType::IncreaseProduction | ResponseType::DecreaseProduction
            if !state.policies.automation_enabled =>
        {
            bonus -= 15.0;
        }
        ResponseType::Trade if state.policies.player_controlled => bonus += 2.0,
        _ => {}
    }
    bonus
}

fn estimate_workload(ctx: &ArbitrationContext<'_>) -> f32 {
    let building_ids = ctx
        .world
        .settlement_store()
        .buildings_for_settlement(ctx.settlement_id);
    let mut incomplete = 0u32;
    for building_id in building_ids {
        let Some(record) = ctx.world.get_building(building_id) else {
            continue;
        };
        if matches!(
            record.lifecycle_state,
            BuildingLifecycleState::Planned
                | BuildingLifecycleState::Foundation
                | BuildingLifecycleState::InProgress
        ) {
            incomplete += 1;
        }
    }
    incomplete as f32 * 5.0
}

fn find_type_conflict(
    candidate_type: ResponseType,
    selected: Option<&Vec<ResponseType>>,
) -> Option<String> {
    let Some(selected) = selected else {
        return None;
    };
    for existing in selected {
        let conflicts = matches!(
            (candidate_type, *existing),
            (ResponseType::IncreaseProduction, ResponseType::DecreaseProduction)
                | (ResponseType::DecreaseProduction, ResponseType::IncreaseProduction)
        );
        if conflicts {
            return Some(format!(
                "{} vs {}",
                candidate_type.as_str(),
                existing.as_str()
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::settlement::needs::NeedId;
    use crate::world::settlement::response::{ResponseAvailability, ResponseId};
    use crate::world::settlement::state::{SettlementKind, SettlementState};
    use crate::world::settlement::SettlementId;
    use crate::world::BuildingId;

    fn candidate(need: &str, response: &str, score: f32, available: bool) -> CandidateResponse {
        CandidateResponse {
            response_id: ResponseId::new(response),
            need_id: NeedId::new(need),
            response_type: ResponseType::IncreaseProduction,
            expected_impact: 0.5,
            estimated_cost: 10.0,
            availability: if available {
                ResponseAvailability::Available
            } else {
                ResponseAvailability::Unavailable
            },
            blocking_reason: None,
            priority_score: score,
            supporting_buildings: Vec::<BuildingId>::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn higher_pressure_raises_arbitration_score() {
        let state = SettlementState::new(SettlementId::new(1), SettlementKind::Town, false);
        let c = candidate("food", "trade_for_food", 100.0, true);
        let low = arbitration_score(&c, 20, &state, 0.0);
        let high = arbitration_score(&c, 90, &state, 0.0);
        assert!(high > low);
    }
}
