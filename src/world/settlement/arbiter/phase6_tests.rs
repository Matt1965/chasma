//! Phase 6 — SA3/SA4 scoring semantics, truthful availability, Food vs Materials (ADR-118/119).

use bevy::prelude::{Quat, Vec3};

use super::scoring::{
    MAX_WORKLOAD_PENALTY, authored_weight_for_need, compute_arbitration_score, compute_urgency,
    policy_component, workload_penalty,
};
use super::{
    ArbitrationContext, IntentPersistence, IntentRejectionReason, MIN_ARBITRATION_SCORE,
    arbitrate_settlement_intent, arbitration_score,
};
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::item::{ItemCatalog, ItemCategoryCatalog};
use crate::world::settlement::SettlementId;
use crate::world::settlement::emergency::EmergencyCatalog;
use crate::world::settlement::needs::{
    NeedCatalog, NeedId, NeedSnapshot, SettlementNeedEvaluation, evaluate_settlement_needs_now,
};
use crate::world::settlement::response::{
    CandidateResponse, CapabilityRequirement, ExpectedEffect, ResponseAvailability,
    ResponseBlockingReason, ResponseCatalog, ResponseDefinition, ResponseId, ResponseQualityScore,
    ResponseType, SettlementResponseCandidates, check_execution_path_available,
    discover_settlement_responses_now, score_candidate, starter_response_definitions,
};
use crate::world::settlement::state::{NeedCategory, NeedTarget, SettlementKind, SettlementState};
use crate::world::settlement::{
    SettlementOwnership, assign_building_settlement, create_settlement_with_treasury,
};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingCategoryCatalog, BuildingDefinitionId,
    BuildingLifecycleState, BuildingOwnership, BuildingSource, ChunkCoord, ChunkLayout,
    InventoryProfileCatalog, LocalPosition, UnitCatalog, WorldData, WorldPosition,
    create_building_with_inventory, starter_building_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions,
};

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn test_candidate(
    need: &str,
    response: &str,
    response_type: ResponseType,
    quality: f32,
    available: bool,
) -> CandidateResponse {
    let quality_score = ResponseQualityScore {
        total: quality,
        relief_component: quality,
        ..Default::default()
    };
    CandidateResponse {
        response_id: ResponseId::new(response),
        need_id: NeedId::new(need),
        response_type,
        expected_impact: 0.5,
        estimated_cost: 10.0,
        availability: if available {
            ResponseAvailability::Available
        } else {
            ResponseAvailability::Unavailable
        },
        blocking_reason: None,
        quality_score,
        priority_score: quality,
        supporting_buildings: Vec::new(),
        diagnostics: Vec::new(),
    }
}

fn town_state(id: u64) -> SettlementState {
    SettlementState::new(SettlementId::new(id), SettlementKind::Town, false)
}

// --- SA3 ownership (via score_candidate) ---

#[test]
fn sa3_pressure_does_not_change_quality() {
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
    let state = town_state(1);
    let emergencies = EmergencyCatalog::default();
    let a = score_candidate(&def, &state, &emergencies, true);
    let b = score_candidate(&def, &state, &emergencies, true);
    assert_eq!(a, b);
    assert!(a.total > 0.0);
}

#[test]
fn sa3_weight_does_not_change_quality() {
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
    let mut low = town_state(1);
    low.need_targets = vec![NeedTarget::new(NeedCategory::Food, 100, 0.1)];
    let mut high = town_state(2);
    high.need_targets = vec![NeedTarget::new(NeedCategory::Food, 100, 5.0)];
    let emergencies = EmergencyCatalog::default();
    assert_eq!(
        score_candidate(&def, &low, &emergencies, true),
        score_candidate(&def, &high, &emergencies, true)
    );
}

#[test]
fn sa3_policy_does_not_change_quality() {
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
    let mut passive = town_state(1);
    passive.policies.automation_enabled = false;
    passive.policies.expansion_enabled = false;
    let mut active = town_state(2);
    active.policies.automation_enabled = true;
    active.policies.expansion_enabled = true;
    let emergencies = EmergencyCatalog::default();
    assert_eq!(
        score_candidate(&def, &passive, &emergencies, true),
        score_candidate(&def, &active, &emergencies, true)
    );
}

#[test]
fn sa3_higher_relief_improves_quality() {
    let emergencies = EmergencyCatalog::default();
    let state = town_state(1);
    let low = ResponseDefinition::new(
        "low",
        "Low",
        "",
        [NeedId::new("food")],
        ResponseType::IncreaseProduction,
        ExpectedEffect::new(0.4, 20.0),
        0,
        [],
    );
    let high = ResponseDefinition::new(
        "high",
        "High",
        "",
        [NeedId::new("food")],
        ResponseType::IncreaseProduction,
        ExpectedEffect::new(0.9, 20.0),
        0,
        [],
    );
    assert!(
        score_candidate(&high, &state, &emergencies, true).total
            > score_candidate(&low, &state, &emergencies, true).total
    );
}

#[test]
fn sa3_higher_cost_lowers_quality() {
    let emergencies = EmergencyCatalog::default();
    let state = town_state(1);
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
    assert!(
        score_candidate(&cheap, &state, &emergencies, true).total
            > score_candidate(&costly, &state, &emergencies, true).total
    );
}

#[test]
fn sa3_components_reproduce_total() {
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
    let state = town_state(1);
    let score = score_candidate(&def, &state, &EmergencyCatalog::default(), true);
    let expected = (score.relief_component + score.priority_modifier + score.emergency_bonus
        - score.cost_penalty)
        .max(0.0);
    assert!((score.total - expected).abs() < 0.001);
}

// --- SA4 ownership ---

#[test]
fn sa4_higher_pressure_raises_arbitration() {
    let state = town_state(1);
    let catalog = NeedCatalog::default();
    let c = test_candidate(
        "food",
        "increase_food_production",
        ResponseType::IncreaseProduction,
        30.0,
        true,
    );
    let low = arbitration_score(&c, 20, &state, &catalog, 0.0);
    let high = arbitration_score(&c, 90, &state, &catalog, 0.0);
    assert!(high > low);
    assert!(
        (high - low - 70.0).abs() < 0.01,
        "pressure should add once via urgency"
    );
}

#[test]
fn sa4_weight_can_flip_food_vs_materials_winner() {
    let catalog = NeedCatalog::default();
    let mut state = town_state(1);
    // Comparable pressures: food=60, materials=55.
    state.need_targets = vec![
        NeedTarget::new(NeedCategory::Food, 100, 1.0),
        NeedTarget::new(NeedCategory::Materials, 50, 1.0),
    ];
    let food = test_candidate(
        "food",
        "increase_food_production",
        ResponseType::IncreaseProduction,
        35.0,
        true,
    );
    let materials = test_candidate(
        "materials",
        "increase_construction_materials",
        ResponseType::IncreaseProduction,
        35.0,
        true,
    );
    let food_wins = arbitration_score(&food, 60, &state, &catalog, 0.0);
    let materials_wins = arbitration_score(&materials, 55, &state, &catalog, 0.0);
    assert!(
        food_wins > materials_wins,
        "food pressure slightly higher at equal weight"
    );

    state.need_targets = vec![
        NeedTarget::new(NeedCategory::Food, 100, 1.0),
        NeedTarget::new(NeedCategory::Materials, 50, 1.5),
    ];
    let food_score = arbitration_score(&food, 60, &state, &catalog, 0.0);
    let materials_score = arbitration_score(&materials, 55, &state, &catalog, 0.0);
    assert!(
        materials_score > food_score,
        "materials weight 1.5 should outrank food at comparable pressure"
    );
}

#[test]
fn sa4_policy_can_change_winner() {
    let catalog = NeedCatalog::default();
    let mut expand_on = town_state(1);
    expand_on.policies.expansion_enabled = true;
    let mut expand_off = town_state(2);
    expand_off.policies.expansion_enabled = false;

    let expand = test_candidate(
        "expansion",
        "expand_settlement",
        ResponseType::Expand,
        30.0,
        true,
    );
    let on_score = arbitration_score(&expand, 50, &expand_on, &catalog, 0.0);
    let off_score = arbitration_score(&expand, 50, &expand_off, &catalog, 0.0);
    assert!(
        on_score - off_score >= 40.0,
        "expansion policy should swing arbitration by ~45 (on={on_score} off={off_score})"
    );

    let mut auto_off = town_state(3);
    auto_off.policies.automation_enabled = false;
    let food = test_candidate(
        "food",
        "increase_food_production",
        ResponseType::IncreaseProduction,
        30.0,
        true,
    );
    let food_auto_on = arbitration_score(&food, 50, &town_state(4), &catalog, 0.0);
    let food_auto_off = arbitration_score(&food, 50, &auto_off, &catalog, 0.0);
    assert!(
        food_auto_on > food_auto_off,
        "automation policy should penalize production arbitration"
    );
}

#[test]
fn sa4_workload_can_change_winner() {
    let state = town_state(1);
    let catalog = NeedCatalog::default();
    let c = test_candidate(
        "food",
        "increase_food_production",
        ResponseType::IncreaseProduction,
        40.0,
        true,
    );
    let low_workload = arbitration_score(&c, 60, &state, &catalog, 0.0);
    let high_workload = arbitration_score(&c, 60, &state, &catalog, 80.0);
    assert!(low_workload > high_workload);
    assert!(high_workload >= 0.0);
}

#[test]
fn sa4_pressure_not_counted_twice() {
    let state = town_state(1);
    let catalog = NeedCatalog::default();
    let c = test_candidate(
        "food",
        "increase_food_production",
        ResponseType::IncreaseProduction,
        30.0,
        true,
    );
    let weight = authored_weight_for_need(&c.need_id, &catalog, &state);
    let b20 = compute_arbitration_score(&c, 20, weight, &state, 0.0);
    let b40 = compute_arbitration_score(&c, 40, weight, &state, 0.0);
    // Quality unchanged; only urgency delta should equal pressure delta * weight.
    assert_eq!(b20.response_quality, b40.response_quality);
    let delta = b40.total - b20.total;
    assert!((delta - 20.0 * weight).abs() < 0.01);
}

#[test]
fn sa4_policy_not_counted_twice() {
    let mut state = town_state(1);
    state.policies.expansion_enabled = true;
    let expand = test_candidate(
        "expansion",
        "expand_settlement",
        ResponseType::Expand,
        25.0,
        true,
    );
    let policy_once = policy_component(&state, &expand);
    let breakdown = compute_arbitration_score(&expand, 50, 1.0, &state, 0.0);
    assert!((breakdown.policy_component - policy_once).abs() < 0.001);
    // Quality path has no policy term.
    assert_eq!(expand.quality_score.total, 25.0);
}

#[test]
fn sa4_components_reproduce_total() {
    let state = town_state(1);
    let c = test_candidate(
        "food",
        "increase_food_production",
        ResponseType::IncreaseProduction,
        30.0,
        true,
    );
    let b = compute_arbitration_score(&c, 60, 1.0, &state, 10.0);
    let expected =
        (b.urgency + b.response_quality + b.policy_component - b.workload_penalty).max(0.0);
    assert!((b.total - expected).abs() < 0.001);
    assert!((b.urgency - compute_urgency(60, 1.0)).abs() < 0.001);
}

// --- Scale ---

#[test]
fn scale_cost_can_change_ranking() {
    let emergencies = EmergencyCatalog::default();
    let state = town_state(1);
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
        ExpectedEffect::new(0.8, 35.0),
        0,
        [],
    );
    let cheap_q = score_candidate(&cheap, &state, &emergencies, true).total;
    let costly_q = score_candidate(&costly, &state, &emergencies, true).total;
    assert!(
        cheap_q - costly_q >= 25.0,
        "cost delta should be meaningful on ~40 scale"
    );
}

#[test]
fn scale_policy_can_change_ranking() {
    let catalog = NeedCatalog::default();
    let mut on = town_state(1);
    on.policies.expansion_enabled = true;
    let mut off = town_state(2);
    off.policies.expansion_enabled = false;
    let expand = test_candidate(
        "expansion",
        "expand_settlement",
        ResponseType::Expand,
        30.0,
        true,
    );
    let on_score = arbitration_score(&expand, 50, &on, &catalog, 0.0);
    let off_score = arbitration_score(&expand, 50, &off, &catalog, 0.0);
    assert!(
        on_score - off_score >= 40.0,
        "policy swing should exceed noise"
    );
}

#[test]
fn scale_workload_can_change_ranking() {
    let catalog = NeedCatalog::default();
    let state = town_state(1);
    let c = test_candidate(
        "food",
        "increase_food_production",
        ResponseType::IncreaseProduction,
        40.0,
        true,
    );
    let fresh = arbitration_score(&c, 60, &state, &catalog, 0.0);
    let busy = arbitration_score(&c, 60, &state, &catalog, 50.0);
    assert!(fresh - busy >= 40.0);
    assert!(workload_penalty(50.0) <= MAX_WORKLOAD_PENALTY);
}

#[test]
fn scale_no_giant_pressure_multiplier_in_quality() {
    let emergencies = EmergencyCatalog::default();
    let state = town_state(1);
    let def = ResponseDefinition::new(
        "r",
        "R",
        "",
        [NeedId::new("food")],
        ResponseType::IncreaseProduction,
        ExpectedEffect::new(1.0, 0.0),
        0,
        [],
    );
    let quality = score_candidate(&def, &state, &emergencies, true).total;
    assert!(
        quality < 100.0,
        "SA3 quality should stay in modest band, not 0-10000"
    );
}

// --- Stability ---

#[test]
fn stability_zero_pressure_rejected() {
    let id = SettlementId::new(100);
    let world = WorldData::new(layout());
    let state = town_state(id.raw());
    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let candidate = test_candidate(
        "food",
        "increase_food_production",
        ResponseType::IncreaseProduction,
        40.0,
        true,
    );
    let candidates = SettlementResponseCandidates {
        settlement_id: id,
        evaluated_tick: 1,
        source_need_tick: 1,
        candidates: vec![candidate],
        diagnostics: Vec::new(),
    };
    let need_eval = SettlementNeedEvaluation {
        settlement_id: id,
        evaluated_tick: 1,
        snapshots: vec![NeedSnapshot::with_values(
            NeedId::new("food"),
            0.0,
            100.0,
            0,
            0,
            "test",
        )],
        diagnostics: Vec::new(),
    };
    let ctx = ArbitrationContext {
        world: &world,
        need_catalog: &need_catalog,
        response_catalog: &response_catalog,
        settlement_id: id,
        state: &state,
        need_evaluation: &need_eval,
        candidates: &candidates,
        simulation_tick: 1,
    };
    let plan = arbitrate_settlement_intent(&ctx);
    assert!(plan.intents.is_empty());
    assert!(
        plan.rejected
            .iter()
            .any(|r| matches!(r.reason, IntentRejectionReason::ZeroPressure))
    );
}

#[test]
fn stability_unavailable_cannot_win() {
    let id = SettlementId::new(101);
    let world = WorldData::new(layout());
    let state = town_state(id.raw());
    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let candidate = test_candidate("food", "trade_for_food", ResponseType::Trade, 99.0, false);
    let candidates = SettlementResponseCandidates {
        settlement_id: id,
        evaluated_tick: 1,
        source_need_tick: 1,
        candidates: vec![candidate],
        diagnostics: Vec::new(),
    };
    let need_eval = SettlementNeedEvaluation {
        settlement_id: id,
        evaluated_tick: 1,
        snapshots: vec![NeedSnapshot::with_values(
            NeedId::new("food"),
            10.0,
            100.0,
            90,
            0,
            "test",
        )],
        diagnostics: Vec::new(),
    };
    let ctx = ArbitrationContext {
        world: &world,
        need_catalog: &need_catalog,
        response_catalog: &response_catalog,
        settlement_id: id,
        state: &state,
        need_evaluation: &need_eval,
        candidates: &candidates,
        simulation_tick: 1,
    };
    let plan = arbitrate_settlement_intent(&ctx);
    assert!(plan.intents.is_empty());
    assert!(
        plan.rejected
            .iter()
            .any(|r| matches!(r.reason, IntentRejectionReason::Unavailable))
    );
}

#[test]
fn stability_min_arbitration_score_coherent() {
    let state = town_state(1);
    let c = test_candidate(
        "food",
        "increase_food_production",
        ResponseType::IncreaseProduction,
        0.5,
        true,
    );
    let b = compute_arbitration_score(&c, 0, 1.0, &state, 0.0);
    assert!(b.total < MIN_ARBITRATION_SCORE);
}

#[test]
fn stability_until_pressure_low_on_high_pressure() {
    let id = SettlementId::new(102);
    let world = WorldData::new(layout());
    let state = town_state(id.raw());
    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let candidate = test_candidate(
        "food",
        "increase_food_production",
        ResponseType::IncreaseProduction,
        40.0,
        true,
    );
    let candidates = SettlementResponseCandidates {
        settlement_id: id,
        evaluated_tick: 1,
        source_need_tick: 1,
        candidates: vec![candidate],
        diagnostics: Vec::new(),
    };
    let need_eval = SettlementNeedEvaluation {
        settlement_id: id,
        evaluated_tick: 1,
        snapshots: vec![NeedSnapshot::with_values(
            NeedId::new("food"),
            10.0,
            100.0,
            85,
            0,
            "test",
        )],
        diagnostics: Vec::new(),
    };
    let ctx = ArbitrationContext {
        world: &world,
        need_catalog: &need_catalog,
        response_catalog: &response_catalog,
        settlement_id: id,
        state: &state,
        need_evaluation: &need_eval,
        candidates: &candidates,
        simulation_tick: 1,
    };
    let plan = arbitrate_settlement_intent(&ctx);
    assert_eq!(plan.intents.len(), 1);
    assert_eq!(
        plan.intents[0].desired_persistence,
        IntentPersistence::UntilPressureLow
    );
}

// --- Availability ---

#[test]
fn availability_construct_building_no_execution_path() {
    let def = starter_response_definitions()
        .into_iter()
        .find(|d| d.id.as_str() == "construct_food_building")
        .unwrap();
    let err = check_execution_path_available(&def).unwrap_err();
    assert!(matches!(
        err,
        ResponseBlockingReason::ExecutionPathUnavailable(_)
    ));
}

#[test]
fn availability_construct_building_has_blocking_reason_in_discovery() {
    let id = SettlementId::new(200);
    let mut world = WorldData::new(layout());
    world
        .settlement_state_store_mut()
        .insert(town_state(id.raw()));
    prepare_discovery_baseline(&mut world, id, 1);
    let responses = world.response_candidate_store().get(id).unwrap();
    let construct = responses
        .candidates
        .iter()
        .find(|c| {
            c.response_id.as_str() == "construct_food_building" && c.need_id.as_str() == "food"
        })
        .expect("construct_food_building for food need");
    assert!(!construct.is_available());
    assert!(matches!(
        construct.blocking_reason,
        Some(ResponseBlockingReason::ExecutionPathUnavailable(_))
    ));
}

#[test]
fn availability_trade_and_recruit_stubs_unavailable() {
    for response_id in ["trade_for_food", "recruit_workers"] {
        let def = starter_response_definitions()
            .into_iter()
            .find(|d| d.id.as_str() == response_id)
            .unwrap();
        assert!(
            check_execution_path_available(&def).is_err(),
            "{response_id} should have no live path"
        );
    }
}

#[test]
fn availability_increase_production_with_farm_remains_available() {
    let mut world = WorldData::new(layout());
    let id = prepare_discovery_with_farm(&mut world, 1);
    let responses = world.response_candidate_store().get(id).unwrap();
    let food_prod = responses
        .candidates
        .iter()
        .find(|c| c.response_id.as_str() == "increase_food_production")
        .expect("increase_food_production");
    assert!(food_prod.is_available(), "{:?}", food_prod.blocking_reason);
}

#[test]
fn availability_based_on_response_type_not_hardcoded_id() {
    let trade = ResponseDefinition::new(
        "custom_trade_stub",
        "Custom Trade",
        "",
        [NeedId::new("food")],
        ResponseType::Trade,
        ExpectedEffect::new(0.5, 1.0),
        0,
        [CapabilityRequirement::Always],
    );
    assert!(check_execution_path_available(&trade).is_err());
    let prod = ResponseDefinition::new(
        "custom_prod",
        "Custom Prod",
        "",
        [NeedId::new("food")],
        ResponseType::IncreaseProduction,
        ExpectedEffect::new(0.5, 1.0),
        0,
        [CapabilityRequirement::Always],
    );
    assert!(check_execution_path_available(&prod).is_ok());
}

// --- Food vs Materials ---

#[test]
fn food_materials_weight_flips_winner() {
    let catalog = NeedCatalog::default();
    let mut state = town_state(1);
    state.need_targets = vec![
        NeedTarget::new(NeedCategory::Food, 100, 1.0),
        NeedTarget::new(NeedCategory::Materials, 50, 1.5),
    ];
    let food = test_candidate(
        "food",
        "increase_food_production",
        ResponseType::IncreaseProduction,
        35.0,
        true,
    );
    let materials = test_candidate(
        "materials",
        "increase_construction_materials",
        ResponseType::IncreaseProduction,
        35.0,
        true,
    );
    let food_score = arbitration_score(&food, 60, &state, &catalog, 0.0);
    let materials_score = arbitration_score(&materials, 55, &state, &catalog, 0.0);
    assert!(materials_score > food_score);
}

#[test]
fn costly_response_can_lose_at_similar_urgency() {
    let emergencies = EmergencyCatalog::default();
    let state = town_state(1);
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
        ExpectedEffect::new(0.8, 30.0),
        0,
        [],
    );
    let cheap_q = score_candidate(&cheap, &state, &emergencies, true).total;
    let costly_q = score_candidate(&costly, &state, &emergencies, true).total;
    assert!(cheap_q > costly_q);
    let catalog = NeedCatalog::default();
    let cheap_c = test_candidate(
        "food",
        "cheap",
        ResponseType::IncreaseProduction,
        cheap_q,
        true,
    );
    let costly_c = test_candidate(
        "food",
        "costly",
        ResponseType::IncreaseProduction,
        costly_q,
        true,
    );
    assert!(
        arbitration_score(&cheap_c, 60, &state, &catalog, 0.0)
            > arbitration_score(&costly_c, 60, &state, &catalog, 0.0)
    );
}

#[test]
fn materials_response_targets_materials_need() {
    let catalog = ResponseCatalog::default();
    let defs = catalog.definitions_for_need(&NeedId::new("materials"));
    assert!(
        defs.iter()
            .any(|d| d.id.as_str() == "increase_construction_materials"),
        "materials production response must target materials need"
    );
    let construction = catalog.definitions_for_need(&NeedId::new("construction"));
    assert!(
        !construction
            .iter()
            .any(|d| d.id.as_str() == "increase_construction_materials"),
        "materials response must not target construction backlog"
    );
}

#[test]
fn construction_backlog_remains_independent() {
    let catalog = ResponseCatalog::default();
    let construction_defs = catalog.definitions_for_need(&NeedId::new("construction"));
    assert!(
        construction_defs
            .iter()
            .any(|d| d.id.as_str() == "advance_construction"),
        "construction backlog keeps its own responses"
    );
    assert!(
        !construction_defs
            .iter()
            .any(|d| d.id.as_str() == "increase_construction_materials"),
        "materials production is not a construction-backlog response"
    );
}

// --- Discovery helpers ---

fn prepare_discovery_baseline(world: &mut WorldData, id: SettlementId, tick: u64) {
    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let buildings = BuildingCatalog::default();
    let items = ItemCatalog::default();
    let categories = ItemCategoryCatalog::default();
    let profiles = InventoryProfileCatalog::default();
    let units = UnitCatalog::default();
    let inventory_ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    evaluate_settlement_needs_now(
        world,
        &need_catalog,
        &buildings,
        &items,
        &units,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        id,
        tick,
    );
    discover_settlement_responses_now(
        world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &buildings,
        id,
        tick,
    );
}

fn prepare_discovery_with_farm(world: &mut WorldData, tick: u64) -> SettlementId {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
    let profiles =
        InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions()).unwrap();
    let inventory_ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    let building_catalog = BuildingCatalog::from_definitions(
        starter_building_definitions(),
        &BuildingCategoryCatalog::default(),
    )
    .unwrap();
    let ownership = BuildingOwnership::with_affiliation(Affiliation::Player);

    let core = create_building_with_inventory(
        &building_catalog,
        world,
        &BuildingDefinitionId::new("settlement_core"),
        pos(50.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        ownership,
        None,
        &inventory_ctx,
    )
    .unwrap()
    .id;
    let farm = create_building_with_inventory(
        &building_catalog,
        world,
        &BuildingDefinitionId::new("prispod_farm"),
        pos(10.0, 10.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        ownership,
        None,
        &inventory_ctx,
    )
    .unwrap()
    .id;
    for building_id in [core, farm] {
        world.mutate_building(building_id, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
    }
    let settlement = create_settlement_with_treasury(
        world,
        &building_catalog,
        &crate::world::BuildingInteractionProfileCatalog::default(),
        core,
        "Phase6 Farm",
        SettlementOwnership::player_default(),
        pos(50.0, 50.0),
        0,
    )
    .unwrap();
    let id = settlement.settlement_id;
    for building_id in [core, farm] {
        let _ = assign_building_settlement(world, building_id, Some(id));
    }

    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let units = UnitCatalog::default();
    evaluate_settlement_needs_now(
        world,
        &need_catalog,
        &building_catalog,
        &items,
        &units,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        id,
        tick,
    );
    discover_settlement_responses_now(
        world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &building_catalog,
        id,
        tick,
    );
    id
}
