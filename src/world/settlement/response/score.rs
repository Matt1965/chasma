//! Generic response scoring framework (SA3). Response quality only — no pressure or policy.

use super::definition::ResponseDefinition;
use super::quality::{ResponseQualityScore, compute_response_quality};
use crate::world::settlement::emergency::EmergencyCatalog;
use crate::world::settlement::state::SettlementState;

/// Score a response option for intrinsic quality (SA3).
///
/// Does not read need pressure, `NeedTarget.weight`, or settlement policy.
/// Unavailable candidates always score `0`.
pub fn score_candidate(
    definition: &ResponseDefinition,
    state: &SettlementState,
    emergency_catalog: &EmergencyCatalog,
    available: bool,
) -> ResponseQualityScore {
    compute_response_quality(definition, state, emergency_catalog, available)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::settlement::SettlementId;
    use crate::world::settlement::needs::{NeedId, NeedSnapshot};
    use crate::world::settlement::response::definition::{
        ExpectedEffect, ResponseDefinition, ResponseType,
    };
    use crate::world::settlement::state::{
        NeedCategory, NeedTarget, SettlementKind, SettlementState,
    };

    #[test]
    fn pressure_does_not_change_response_quality() {
        let def = ResponseDefinition::new(
            "r",
            "R",
            "",
            [NeedId::new("food")],
            ResponseType::IncreaseProduction,
            ExpectedEffect::new(0.8, 20.0),
            10,
            [],
        );
        let state = SettlementState::new(SettlementId::new(1), SettlementKind::Town, false);
        let emergencies = crate::world::settlement::emergency::EmergencyCatalog::default();
        let low_pressure_snap =
            NeedSnapshot::with_values(NeedId::new("food"), 50.0, 100.0, 10, 0, "t");
        let high_pressure_snap =
            NeedSnapshot::with_values(NeedId::new("food"), 0.0, 100.0, 100, 0, "t");
        let _ = (low_pressure_snap, high_pressure_snap);
        let score = score_candidate(&def, &state, &emergencies, true);
        assert_eq!(
            score,
            score_candidate(&def, &state, &emergencies, true),
            "quality must not depend on snapshot pressure"
        );
        assert!(score.total > 0.0);
    }

    #[test]
    fn weight_does_not_change_response_quality() {
        let def = ResponseDefinition::new(
            "r",
            "R",
            "",
            [NeedId::new("food")],
            ResponseType::IncreaseProduction,
            ExpectedEffect::new(0.8, 20.0),
            10,
            [],
        );
        let mut low_weight =
            SettlementState::new(SettlementId::new(1), SettlementKind::Town, false);
        low_weight.need_targets = vec![NeedTarget::new(NeedCategory::Food, 100, 0.1)];
        let mut high_weight =
            SettlementState::new(SettlementId::new(2), SettlementKind::Town, false);
        high_weight.need_targets = vec![NeedTarget::new(NeedCategory::Food, 100, 5.0)];
        let emergencies = crate::world::settlement::emergency::EmergencyCatalog::default();
        let low = score_candidate(&def, &low_weight, &emergencies, true);
        let high = score_candidate(&def, &high_weight, &emergencies, true);
        assert_eq!(low, high);
    }

    #[test]
    fn higher_relief_improves_quality() {
        let emergencies = crate::world::settlement::emergency::EmergencyCatalog::default();
        let state = SettlementState::new(SettlementId::new(1), SettlementKind::Town, false);
        let low_relief = ResponseDefinition::new(
            "low",
            "Low",
            "",
            [NeedId::new("food")],
            ResponseType::IncreaseProduction,
            ExpectedEffect::new(0.4, 20.0),
            0,
            [],
        );
        let high_relief = ResponseDefinition::new(
            "high",
            "High",
            "",
            [NeedId::new("food")],
            ResponseType::IncreaseProduction,
            ExpectedEffect::new(0.9, 20.0),
            0,
            [],
        );
        let low = score_candidate(&low_relief, &state, &emergencies, true);
        let high = score_candidate(&high_relief, &state, &emergencies, true);
        assert!(high.total > low.total);
    }

    #[test]
    fn higher_cost_lowers_quality() {
        let emergencies = crate::world::settlement::emergency::EmergencyCatalog::default();
        let state = SettlementState::new(SettlementId::new(1), SettlementKind::Town, false);
        let cheap = ResponseDefinition::new(
            "cheap",
            "Cheap",
            "",
            [NeedId::new("food")],
            ResponseType::IncreaseProduction,
            ExpectedEffect::new(0.8, 5.0),
            0,
            [],
        );
        let costly = ResponseDefinition::new(
            "costly",
            "Costly",
            "",
            [NeedId::new("food")],
            ResponseType::IncreaseProduction,
            ExpectedEffect::new(0.8, 40.0),
            0,
            [],
        );
        let cheap_score = score_candidate(&cheap, &state, &emergencies, true);
        let costly_score = score_candidate(&costly, &state, &emergencies, true);
        assert!(cheap_score.total > costly_score.total);
    }

    #[test]
    fn quality_components_reproduce_total() {
        let def = ResponseDefinition::new(
            "r",
            "R",
            "",
            [NeedId::new("food")],
            ResponseType::IncreaseProduction,
            ExpectedEffect::new(0.75, 25.0),
            12,
            [],
        );
        let state = SettlementState::new(SettlementId::new(1), SettlementKind::Town, false);
        let emergencies = crate::world::settlement::emergency::EmergencyCatalog::default();
        let score = score_candidate(&def, &state, &emergencies, true);
        let expected = (score.relief_component + score.priority_modifier + score.emergency_bonus
            - score.cost_penalty)
            .max(0.0);
        assert!((score.total - expected).abs() < 0.001);
    }

    #[test]
    fn policy_does_not_change_response_quality() {
        let def = ResponseDefinition::new(
            "r",
            "R",
            "",
            [NeedId::new("food")],
            ResponseType::IncreaseProduction,
            ExpectedEffect::new(0.8, 20.0),
            10,
            [],
        );
        let mut passive = SettlementState::new(SettlementId::new(1), SettlementKind::Town, false);
        passive.policies.expansion_enabled = false;
        passive.policies.automation_enabled = false;
        passive.policies.aggression = 0;
        let mut aggressive =
            SettlementState::new(SettlementId::new(2), SettlementKind::Town, false);
        aggressive.policies.expansion_enabled = true;
        aggressive.policies.automation_enabled = true;
        aggressive.policies.aggression = 255;
        let emergencies = crate::world::settlement::emergency::EmergencyCatalog::default();
        let passive_score = score_candidate(&def, &passive, &emergencies, true);
        let aggressive_score = score_candidate(&def, &aggressive, &emergencies, true);
        assert_eq!(passive_score, aggressive_score);
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
        let emergencies = crate::world::settlement::emergency::EmergencyCatalog::default();
        assert_eq!(
            score_candidate(&def, &state, &emergencies, false).total,
            0.0
        );
    }
}
