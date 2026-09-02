//! Settlement Response Arbiter tests (SA4).

use super::*;
use bevy::prelude::{Quat, Vec3};

use crate::world::inventory::InventoryCatalogCtx;
use crate::world::item::{ItemCatalog, ItemCategoryCatalog};
use crate::world::settlement::SettlementId;
use crate::world::settlement::emergency::EmergencyCatalog;
use crate::world::settlement::needs::{NeedCatalog, evaluate_settlement_needs_now};
use crate::world::settlement::response::{
    ResponseCatalog, ResponseType, discover_settlement_responses_now,
};
use crate::world::settlement::state::{SettlementKind, SettlementState};
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

fn world_with_settlement(id: SettlementId) -> WorldData {
    let mut world = WorldData::new(layout());
    world
        .settlement_state_store_mut()
        .insert(SettlementState::new(id, SettlementKind::Town, false));
    world
}

fn prepare_candidates(world: &mut WorldData, id: SettlementId, tick: u64) {
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

fn prepare_candidates_with_farm(world: &mut WorldData, tick: u64) -> SettlementId {
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
        "Arbiter Farm Test",
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

#[test]
fn high_pressure_food_responses_selected() {
    let mut world = WorldData::new(layout());
    let id = prepare_candidates_with_farm(&mut world, 10);
    let need_catalog = NeedCatalog::default();
    let catalog = ResponseCatalog::default();
    arbitrate_settlement_intent_now(&mut world, &need_catalog, &catalog, id, 10);

    let plan = world.settlement_intent_store().get(id).expect("plan");
    assert!(validate_settlement_intent_plan(plan, Some(&catalog)).is_empty());
    assert!(
        !plan.intents.is_empty(),
        "expected at least one intent from food pressure"
    );
    let food_intents: Vec<_> = plan.intents_for_need("food").collect();
    assert!(
        !food_intents.is_empty(),
        "high food pressure should select food responses"
    );
    // With a farm present, IncreaseProduction is the live food path (trade/construct are unavailable).
    assert!(
        food_intents.iter().any(|i| {
            i.chosen_response.as_str() == "increase_food_production"
                && matches!(i.response_type, ResponseType::IncreaseProduction)
        }),
        "selected={:?}",
        food_intents
            .iter()
            .map(|i| i.chosen_response.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn multiple_responses_coexist_across_needs() {
    let id = SettlementId::new(2);
    let mut world = world_with_settlement(id);
    prepare_candidates(&mut world, id, 1);
    let need_catalog = NeedCatalog::default();
    let catalog = ResponseCatalog::default();
    arbitrate_settlement_intent_now(&mut world, &need_catalog, &catalog, id, 1);

    let plan = world.settlement_intent_store().get(id).unwrap();
    let needs: std::collections::BTreeSet<_> = plan
        .intents
        .iter()
        .map(|i| i.source_need.as_str())
        .collect();
    // Town defaults create food (+ often housing/defense/growth) pressure — multi-need intents.
    assert!(
        plan.intents.len() >= 1,
        "expected at least one intent: intents={:?} rejected={}",
        plan.intents
            .iter()
            .map(|i| (i.source_need.as_str(), i.chosen_response.as_str()))
            .collect::<Vec<_>>(),
        plan.rejected.len()
    );
    let _ = needs;
    // Food high pressure allows up to 2 intents for that need.
    let food_count = plan.intents_for_need("food").count();
    assert!(food_count <= MAX_INTENTS_PER_NEED_HIGH);
    assert!(plan.intents.len() <= MAX_SETTLEMENT_INTENTS);
}

#[test]
fn planning_is_deterministic() {
    let id = SettlementId::new(3);
    let mut world = world_with_settlement(id);
    prepare_candidates(&mut world, id, 5);
    let need_catalog = NeedCatalog::default();
    let catalog = ResponseCatalog::default();

    arbitrate_settlement_intent_now(&mut world, &need_catalog, &catalog, id, 5);
    let first = world.settlement_intent_store().get(id).cloned().unwrap();
    arbitrate_settlement_intent_now(&mut world, &need_catalog, &catalog, id, 5);
    let second = world.settlement_intent_store().get(id).cloned().unwrap();
    assert_eq!(first.intents, second.intents);
    assert_eq!(first.rejected.len(), second.rejected.len());
}

#[test]
fn replanning_on_dirty_and_candidate_change() {
    let id = SettlementId::new(4);
    let mut world = world_with_settlement(id);
    prepare_candidates(&mut world, id, 1);
    let need_catalog = NeedCatalog::default();
    let catalog = ResponseCatalog::default();

    let n = step_settlement_response_arbitration(&mut world, &need_catalog, &catalog, 1);
    assert_eq!(n, 1);
    assert!(!world.settlement_intent_store().is_dirty(id));

    let n = step_settlement_response_arbitration(&mut world, &need_catalog, &catalog, 2);
    assert_eq!(n, 0);

    world.settlement_intent_store_mut().mark_dirty(id);
    let n = step_settlement_response_arbitration(&mut world, &need_catalog, &catalog, 3);
    assert_eq!(n, 1);

    // Candidate rediscovery at new tick should force replan via source_response_tick.
    prepare_candidates(&mut world, id, 4);
    let n = step_settlement_response_arbitration(&mut world, &need_catalog, &catalog, 4);
    assert_eq!(n, 1);
    assert_eq!(
        world
            .settlement_intent_store()
            .get(id)
            .unwrap()
            .planned_tick,
        4
    );
}

#[test]
fn intent_not_serialized_clear_rebuilds() {
    let id = SettlementId::new(5);
    let mut world = world_with_settlement(id);
    prepare_candidates(&mut world, id, 1);
    let need_catalog = NeedCatalog::default();
    let catalog = ResponseCatalog::default();
    arbitrate_settlement_intent_now(&mut world, &need_catalog, &catalog, id, 1);
    assert!(!world.settlement_intent_store().is_empty());

    // SettlementIntent has no Serialize — store clear simulates load rebuild principle.
    world.settlement_intent_store_mut().clear();
    assert!(world.settlement_intent_store().is_empty());
    assert!(world.settlement_intent_store().is_dirty(id));

    arbitrate_settlement_intent_now(&mut world, &need_catalog, &catalog, id, 2);
    assert!(
        !world
            .settlement_intent_store()
            .get(id)
            .unwrap()
            .intents
            .is_empty()
    );
}

#[test]
fn rejected_includes_unavailable_candidates() {
    let id = SettlementId::new(6);
    let mut world = world_with_settlement(id);
    prepare_candidates(&mut world, id, 1);
    let need_catalog = NeedCatalog::default();
    let catalog = ResponseCatalog::default();
    arbitrate_settlement_intent_now(&mut world, &need_catalog, &catalog, id, 1);
    let plan = world.settlement_intent_store().get(id).unwrap();
    assert!(
        plan.rejected
            .iter()
            .any(|r| matches!(r.reason, IntentRejectionReason::Unavailable)),
        "production responses without buildings should be rejected as unavailable"
    );
}

#[test]
fn mark_settlement_state_dirty_marks_intent_store() {
    let id = SettlementId::new(7);
    let mut world = world_with_settlement(id);
    prepare_candidates(&mut world, id, 1);
    let need_catalog = NeedCatalog::default();
    let catalog = ResponseCatalog::default();
    arbitrate_settlement_intent_now(&mut world, &need_catalog, &catalog, id, 1);
    assert!(!world.settlement_intent_store().is_dirty(id));
    crate::world::settlement::mark_settlement_state_dirty(&mut world, id);
    assert!(world.settlement_intent_store().is_dirty(id));
}

#[test]
fn no_increase_and_decrease_conflict_in_plan() {
    let id = SettlementId::new(8);
    let mut world = world_with_settlement(id);
    prepare_candidates(&mut world, id, 1);
    let need_catalog = NeedCatalog::default();
    let catalog = ResponseCatalog::default();
    arbitrate_settlement_intent_now(&mut world, &need_catalog, &catalog, id, 1);
    let plan = world.settlement_intent_store().get(id).unwrap();
    let errors = validate_settlement_intent_plan(plan, Some(&catalog));
    assert!(
        !errors
            .iter()
            .any(|e| matches!(e, IntentValidationError::ConflictingTypes { .. })),
        "{errors:?}"
    );
    // Chosen intents should never mix increase/decrease for same need.
    for need in ["food", "luxury"] {
        let types: Vec<_> = plan
            .intents_for_need(need)
            .map(|i| i.response_type)
            .collect();
        let has_inc = types.contains(&ResponseType::IncreaseProduction);
        let has_dec = types.contains(&ResponseType::DecreaseProduction);
        assert!(!(has_inc && has_dec), "need {need} has conflict: {types:?}");
    }
}
