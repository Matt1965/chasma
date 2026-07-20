//! Generic response scoring framework (SA3). Intentionally simple.

use super::definition::ResponseDefinition;
use crate::world::settlement::emergency::EmergencyCatalog;
use crate::world::settlement::needs::NeedSnapshot;
use crate::world::settlement::state::SettlementState;

/// Score a response option for a need snapshot.
///
/// ```text
/// score = pressure * relief * 100
///       - estimated_cost
///       + priority_modifier
///       + policy_bonus
///       + emergency_response_score (SA8, authored; not a second need-pressure bump)
/// ```
///
/// Unavailable candidates always score `0`. Result is clamped to `>= 0`.
pub fn score_candidate(
    definition: &ResponseDefinition,
    snapshot: &NeedSnapshot,
    state: &SettlementState,
    emergency_catalog: &EmergencyCatalog,
    available: bool,
) -> f32 {
    if !available {
        return 0.0;
    }
    let pressure = f32::from(snapshot.pressure);
    let relief = definition.expected_effect.pressure_relief.clamp(0.0, 1.0);
    let cost = definition.expected_effect.estimated_cost.max(0.0);
    let modifier = f32::from(definition.priority_modifier);
    let policy_bonus = policy_score_bonus(definition, state);
    let emergency_bonus = crate::world::settlement::emergency::emergency_response_score_delta(
        state,
        emergency_catalog,
        definition,
    );
    let raw = pressure * relief * 100.0 - cost + modifier + policy_bonus + emergency_bonus;
    raw.max(0.0)
}

fn policy_score_bonus(definition: &ResponseDefinition, state: &SettlementState) -> f32 {
    let mut bonus = 0.0;
    // Soft preference nudges from policies — not hard gates (gates live in capability checks).
    if state.policies.expansion_enabled
        && definition
            .ai_tags
            .iter()
            .any(|t| t == "expansion" || t == "growth")
    {
        bonus += 5.0;
    }
    if state.policies.aggression >= 128
        && definition.ai_tags.iter().any(|t| t == "defense")
    {
        bonus += 5.0;
    }
    if !state.policies.automation_enabled
        && definition
            .ai_tags
            .iter()
            .any(|t| t == "production")
    {
        bonus -= 10.0;
    }
    bonus
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::settlement::needs::{NeedId, NeedSnapshot};
    use crate::world::settlement::response::definition::{
        ExpectedEffect, ResponseDefinition, ResponseType,
    };
    use crate::world::settlement::state::{SettlementKind, SettlementState};
    use crate::world::settlement::SettlementId;

    #[test]
    fn higher_pressure_scores_higher() {
        let def = ResponseDefinition::new(
            "r",
            "R",
            "",
            [NeedId::new("food")],
            ResponseType::IncreaseProduction,
            ExpectedEffect::new(0.5, 10.0),
            0,
            [],
        );
        let state = SettlementState::new(SettlementId::new(1), SettlementKind::Town, false);
        let low = NeedSnapshot::with_values(NeedId::new("food"), 50.0, 100.0, 50, 0, "t");
        let high = NeedSnapshot::with_values(NeedId::new("food"), 0.0, 100.0, 100, 0, "t");
        let emergencies = crate::world::settlement::emergency::EmergencyCatalog::default();
        let s_low = score_candidate(&def, &low, &state, &emergencies, true);
        let s_high = score_candidate(&def, &high, &state, &emergencies, true);
        assert!(s_high > s_low);
    }

    #[test]
    fn unavailable_scores_zero() {
        let def = ResponseDefinition::new(
            "r",
            "R",
            "",
            [NeedId::new("food")],
            ResponseType::Trade,
            ExpectedEffect::new(1.0, 0.0),
            100,
            [],
        );
        let state = SettlementState::new(SettlementId::new(1), SettlementKind::Town, false);
        let snap = NeedSnapshot::with_values(NeedId::new("food"), 0.0, 100.0, 100, 0, "t");
        let emergencies = crate::world::settlement::emergency::EmergencyCatalog::default();
        assert_eq!(score_candidate(&def, &snap, &state, &emergencies, false), 0.0);
    }
}
