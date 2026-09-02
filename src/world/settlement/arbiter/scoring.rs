//! SA4 arbitration score components (Phase 6).

use bevy::prelude::*;

use crate::world::settlement::needs::{NeedCatalog, NeedId};
use crate::world::settlement::response::{CandidateResponse, ResponseType};
use crate::world::settlement::state::{NeedCategory, SettlementState};

/// Maximum workload penalty applied during arbitration.
pub const MAX_WORKLOAD_PENALTY: f32 = 60.0;

/// Workload units → penalty multiplier.
pub const WORKLOAD_PENALTY_FACTOR: f32 = 1.0;

/// Breakdown of SA4 arbitration score. `total` is the authoritative score.
#[derive(Debug, Clone, Copy, PartialEq, Reflect, Default)]
pub struct ArbitrationScoreBreakdown {
    pub raw_pressure: u8,
    pub authored_weight: f32,
    pub urgency: f32,
    pub response_quality: f32,
    pub policy_component: f32,
    pub workload_penalty: f32,
    pub total: f32,
}

impl ArbitrationScoreBreakdown {
    pub fn format_reasoning(&self) -> String {
        format!(
            "pressure={} weight={:.2} urgency={:.1} quality={:.1} policy={:+.1} workload=-{:.1} arb={:.1}",
            self.raw_pressure,
            self.authored_weight,
            self.urgency,
            self.response_quality,
            self.policy_component,
            self.workload_penalty,
            self.total,
        )
    }
}

/// Lookup authored `NeedTarget.weight` for a need via its definition's target category.
pub fn authored_weight_for_need(
    need_id: &NeedId,
    need_catalog: &NeedCatalog,
    state: &SettlementState,
) -> f32 {
    let category = need_catalog
        .get(need_id)
        .map(|def| def.target_category)
        .unwrap_or(NeedCategory::Food);
    state
        .need_targets
        .iter()
        .find(|target| target.category == category)
        .map(|target| target.weight)
        .unwrap_or(1.0)
        .max(0.01)
}

/// Compute urgency from objective SA2 pressure and authored settlement weight.
pub fn compute_urgency(pressure: u8, weight: f32) -> f32 {
    f32::from(pressure) * weight.max(0.01)
}

/// Settlement policy contribution — applied once in SA4 only.
pub fn policy_component(state: &SettlementState, candidate: &CandidateResponse) -> f32 {
    let mut bonus = 0.0;
    match candidate.response_type {
        ResponseType::Expand if state.policies.expansion_enabled => bonus += 15.0,
        ResponseType::Expand => bonus -= 30.0,
        ResponseType::Defend => bonus += f32::from(state.policies.aggression) / 16.0,
        ResponseType::IncreaseProduction | ResponseType::DecreaseProduction
            if !state.policies.automation_enabled =>
        {
            bonus -= 25.0;
        }
        ResponseType::Trade if state.policies.player_controlled => bonus += 5.0,
        _ => {}
    }
    bonus
}

pub fn workload_penalty(workload: f32) -> f32 {
    (workload * WORKLOAD_PENALTY_FACTOR).min(MAX_WORKLOAD_PENALTY)
}

/// Combine urgency, SA3 quality, policy, and workload into one arbitration score.
pub fn compute_arbitration_score(
    candidate: &CandidateResponse,
    pressure: u8,
    weight: f32,
    state: &SettlementState,
    workload: f32,
) -> ArbitrationScoreBreakdown {
    if !candidate.is_available() {
        return ArbitrationScoreBreakdown {
            raw_pressure: pressure,
            authored_weight: weight,
            ..Default::default()
        };
    }

    let urgency = compute_urgency(pressure, weight);
    let response_quality = candidate.quality_score.total;
    let policy_component = policy_component(state, candidate);
    let workload_penalty = workload_penalty(workload);
    let total = (urgency + response_quality + policy_component - workload_penalty).max(0.0);

    ArbitrationScoreBreakdown {
        raw_pressure: pressure,
        authored_weight: weight,
        urgency,
        response_quality,
        policy_component,
        workload_penalty,
        total,
    }
}
