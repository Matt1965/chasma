//! Response Engine tests (SA3).

use super::*;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::item::{ItemCatalog, ItemCategoryCatalog};
use crate::world::settlement::emergency::EmergencyCatalog;
use crate::world::settlement::needs::{
    evaluate_settlement_needs_now, NeedCatalog, NeedId, NEED_EVAL_CADENCE_TICKS,
};
use crate::world::settlement::state::{SettlementKind, SettlementState};
use crate::world::settlement::SettlementId;
use crate::world::{BuildingCatalog, ChunkLayout, InventoryProfileCatalog, WorldData};

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn world_with_settlement(id: SettlementId) -> WorldData {
    let mut world = WorldData::new(layout());
    world
        .settlement_state_store_mut()
        .insert(SettlementState::new(id, SettlementKind::Town, false));
    world
}

fn evaluate_needs(world: &mut WorldData, id: SettlementId, tick: u64) {
    let need_catalog = NeedCatalog::default();
    let buildings = BuildingCatalog::default();
    let items = ItemCatalog::default();
    let categories = ItemCategoryCatalog::default();
    let profiles = InventoryProfileCatalog::default();
    let inventory_ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    evaluate_settlement_needs_now(
        world,
        &need_catalog,
        &buildings,
        &items,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        id,
        tick,
    );
}

#[test]
fn starter_catalog_valid_against_needs() {
    let needs = NeedCatalog::default();
    let responses = ResponseCatalog::default();
    assert!(responses.len() >= 9);
    let errors =
        validate_response_catalog_against_needs(responses.definitions(), &needs);
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn duplicate_response_id_rejected() {
    let mut defs = starter_response_definitions();
    defs.push(defs[0].clone());
    let err = ResponseCatalog::from_definitions(defs).unwrap_err();
    assert!(matches!(err, ResponseCatalogError::DuplicateResponseId(_)));
}

#[test]
fn unknown_need_id_rejected_when_checked() {
    let defs = vec![ResponseDefinition::new(
        "bad",
        "Bad",
        "",
        [NeedId::new("not_a_real_need")],
        ResponseType::Trade,
        ExpectedEffect::new(0.5, 1.0),
        0,
        [CapabilityRequirement::Always],
    )];
    let needs = NeedCatalog::default();
    let err = validate_response_catalog_definitions_with_needs(&defs, Some(&needs)).unwrap_err();
    assert!(matches!(err, ResponseCatalogError::UnknownNeedId(_)));
}

#[test]
fn circular_prerequisites_rejected() {
    let a = ResponseDefinition::new(
        "a",
        "A",
        "",
        [NeedId::new("food")],
        ResponseType::Trade,
        ExpectedEffect::new(0.5, 1.0),
        0,
        [CapabilityRequirement::Always],
    )
    .with_prerequisites([ResponseId::new("b")]);
    let b = ResponseDefinition::new(
        "b",
        "B",
        "",
        [NeedId::new("food")],
        ResponseType::Trade,
        ExpectedEffect::new(0.5, 1.0),
        0,
        [CapabilityRequirement::Always],
    )
    .with_prerequisites([ResponseId::new("a")]);
    let err = ResponseCatalog::from_definitions(vec![a, b]).unwrap_err();
    assert!(matches!(err, ResponseCatalogError::CircularPrerequisites(_)));
}

#[test]
fn food_responses_discovered_from_catalog() {
    let id = SettlementId::new(1);
    let mut world = world_with_settlement(id);
    evaluate_needs(&mut world, id, 10);

    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let buildings = BuildingCatalog::default();
    discover_settlement_responses_now(
        &mut world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &buildings,
        id,
        10,
    );

    let result = world.response_candidate_store().get(id).expect("candidates");
    assert!(validate_settlement_response_candidates(result).is_empty());
    let food: Vec<_> = result.for_need("food").collect();
    assert!(
        food.len() >= 3,
        "catalog should offer multiple food responses, got {}",
        food.len()
    );
    let ids: Vec<&str> = food.iter().map(|c| c.response_id.as_str()).collect();
    assert!(ids.contains(&"trade_for_food"));
    assert!(ids.contains(&"construct_food_building"));
    // Catalog-driven — no hard-coded farm-only path.
    assert!(
        ids.contains(&"increase_food_production") || ids.contains(&"bake_bread_production"),
        "expected production-tagged food responses: {ids:?}"
    );
}

#[test]
fn unavailable_responses_filtered_from_available_iterator() {
    let id = SettlementId::new(2);
    let mut world = world_with_settlement(id);
    evaluate_needs(&mut world, id, 1);

    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let buildings = BuildingCatalog::default();
    discover_settlement_responses_now(
        &mut world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &buildings,
        id,
        1,
    );

    let result = world.response_candidate_store().get(id).unwrap();
    // Without food-producing buildings, production responses are unavailable.
    let production_food: Vec<_> = result
        .for_need("food")
        .filter(|c| {
            matches!(
                c.response_type,
                ResponseType::IncreaseProduction
            )
        })
        .collect();
    assert!(!production_food.is_empty());
    assert!(
        production_food.iter().all(|c| !c.is_available()),
        "production food responses should be unavailable without buildings"
    );
    let available_food: Vec<_> = result
        .for_need("food")
        .filter(|c| c.is_available())
        .collect();
    assert!(
        available_food
            .iter()
            .any(|c| c.response_id.as_str() == "trade_for_food"),
        "Always-capability trade stub should remain available"
    );
    assert_eq!(
        result.available().count(),
        result.candidates.iter().filter(|c| c.is_available()).count()
    );
}

#[test]
fn scores_are_deterministic() {
    let id = SettlementId::new(3);
    let mut world = world_with_settlement(id);
    evaluate_needs(&mut world, id, 5);
    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let buildings = BuildingCatalog::default();

    discover_settlement_responses_now(
        &mut world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &buildings,
        id,
        5,
    );
    let first = world.response_candidate_store().get(id).cloned().unwrap();

    discover_settlement_responses_now(
        &mut world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &buildings,
        id,
        5,
    );
    let second = world.response_candidate_store().get(id).cloned().unwrap();
    assert_eq!(first, second);
}

#[test]
fn catalog_drives_behavior_not_hardcoded_need_match() {
    // A response only appears for needs listed in supported_need_ids.
    let defs = vec![ResponseDefinition::new(
        "only_defense",
        "Only Defense",
        "",
        [NeedId::new("defense")],
        ResponseType::Defend,
        ExpectedEffect::new(0.5, 1.0),
        0,
        [CapabilityRequirement::Always],
    )];
    let catalog = ResponseCatalog::from_definitions(defs).unwrap();
    assert!(catalog.definitions_for_need(&NeedId::new("food")).is_empty());
    assert_eq!(catalog.definitions_for_need(&NeedId::new("defense")).len(), 1);
}

#[test]
fn dirty_and_need_change_trigger_rediscovery() {
    let id = SettlementId::new(4);
    let mut world = world_with_settlement(id);
    evaluate_needs(&mut world, id, 1);
    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let buildings = BuildingCatalog::default();

    let n = step_settlement_response_discovery(
        &mut world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &buildings,
        1,
    );
    assert_eq!(n, 1);
    assert!(!world.response_candidate_store().is_dirty(id));

    let n = step_settlement_response_discovery(
        &mut world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &buildings,
        2,
    );
    assert_eq!(n, 0, "no dirty / need change / cadence");

    world.response_candidate_store_mut().mark_dirty(id);
    let n = step_settlement_response_discovery(
        &mut world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &buildings,
        3,
    );
    assert_eq!(n, 1);

    // Need re-eval at new tick should force rediscovery via source_need_tick mismatch.
    evaluate_needs(&mut world, id, 4);
    let n = step_settlement_response_discovery(
        &mut world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &buildings,
        4,
    );
    assert_eq!(n, 1);
    let _ = NEED_EVAL_CADENCE_TICKS;
}

#[test]
fn snapshots_not_persisted_clear_rebuilds() {
    let id = SettlementId::new(5);
    let mut world = world_with_settlement(id);
    evaluate_needs(&mut world, id, 1);
    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let buildings = BuildingCatalog::default();
    discover_settlement_responses_now(
        &mut world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &buildings,
        id,
        1,
    );
    assert!(!world.response_candidate_store().is_empty());
    world.response_candidate_store_mut().clear();
    assert!(world.response_candidate_store().is_empty());
    assert!(world.response_candidate_store().is_dirty(id));
}

#[test]
fn mark_settlement_state_dirty_marks_response_store() {
    let id = SettlementId::new(6);
    let mut world = world_with_settlement(id);
    evaluate_needs(&mut world, id, 1);
    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let buildings = BuildingCatalog::default();
    discover_settlement_responses_now(
        &mut world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &buildings,
        id,
        1,
    );
    assert!(!world.response_candidate_store().is_dirty(id));
    crate::world::settlement::mark_settlement_state_dirty(&mut world, id);
    assert!(world.response_candidate_store().is_dirty(id));
}
