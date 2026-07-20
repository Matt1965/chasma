//! Need Evaluation Runtime tests (SA2).

use super::*;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::settlement::emergency::EmergencyCatalog;
use crate::world::item::{ItemCatalog, ItemCategoryCatalog};
use crate::world::settlement::state::{
    NeedCategory, NeedTarget, SettlementKind, SettlementModifier, SettlementModifierSource,
    SettlementState,
};
use crate::world::settlement::SettlementId;
use crate::world::{
    BuildingCatalog, ChunkLayout, InventoryProfileCatalog, WorldData,
};

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn catalogs() -> (
    NeedCatalog,
    BuildingCatalog,
    ItemCatalog,
    ItemCategoryCatalog,
    InventoryProfileCatalog,
) {
    (
        NeedCatalog::default(),
        BuildingCatalog::default(),
        ItemCatalog::default(),
        ItemCategoryCatalog::default(),
        InventoryProfileCatalog::default(),
    )
}

fn world_with_settlement(id: SettlementId) -> WorldData {
    let mut world = WorldData::new(layout());
    world
        .settlement_state_store_mut()
        .insert(SettlementState::new(id, SettlementKind::Town, false));
    world
}

#[test]
fn starter_catalog_has_seven_unique_needs() {
    let catalog = NeedCatalog::default();
    assert_eq!(catalog.len(), 7);
    assert!(validate_need_catalog(&catalog).is_empty());
    for id in [
        "food",
        "construction",
        "housing",
        "defense",
        "research",
        "expansion",
        "luxury",
    ] {
        assert!(catalog.get_str(id).is_some(), "missing {id}");
    }
}

#[test]
fn duplicate_need_id_rejected() {
    let mut defs = starter_need_definitions();
    defs.push(defs[0].clone());
    let err = NeedCatalog::from_definitions(defs).unwrap_err();
    assert!(matches!(err, NeedCatalogError::DuplicateNeedId(_)));
}

#[test]
fn food_pressure_from_target_with_empty_stock() {
    let id = SettlementId::new(1);
    let mut world = world_with_settlement(id);
    let (need_catalog, buildings, items, categories, profiles) = catalogs();
    let inventory_ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);

    evaluate_settlement_needs_now(
        &mut world,
        &need_catalog,
        &buildings,
        &items,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        id,
        10,
    );

    let eval = world.need_evaluation_store().get(id).expect("evaluation");
    assert!(validate_settlement_need_evaluation(eval).is_empty());
    let food = eval.snapshot_str("food").expect("food");
    assert_eq!(food.desired_value, 100.0);
    assert_eq!(food.current_value, 0.0);
    assert_eq!(food.pressure, 100);
    assert_eq!(food.evaluated_tick, 10);
}

#[test]
fn evaluation_is_deterministic() {
    let id = SettlementId::new(2);
    let mut world = world_with_settlement(id);
    let (need_catalog, buildings, items, categories, profiles) = catalogs();
    let inventory_ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);

    evaluate_settlement_needs_now(
        &mut world,
        &need_catalog,
        &buildings,
        &items,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        id,
        5,
    );
    let first = world.need_evaluation_store().get(id).cloned().unwrap();

    evaluate_settlement_needs_now(
        &mut world,
        &need_catalog,
        &buildings,
        &items,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        id,
        5,
    );
    let second = world.need_evaluation_store().get(id).cloned().unwrap();
    assert_eq!(first, second);
}

#[test]
fn dirty_evaluation_reruns_and_clears_need_dirty() {
    let id = SettlementId::new(3);
    let mut world = world_with_settlement(id);
    let (need_catalog, buildings, items, categories, profiles) = catalogs();
    let inventory_ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);

    let n = step_settlement_need_evaluation(
        &mut world,
        &need_catalog,
        &buildings,
        &items,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        1,
    );
    assert_eq!(n, 1);
    assert!(!world.need_evaluation_store().is_dirty(id));

    // Cadence not elapsed and not dirty → skip.
    let n = step_settlement_need_evaluation(
        &mut world,
        &need_catalog,
        &buildings,
        &items,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        2,
    );
    assert_eq!(n, 0);

    world.need_evaluation_store_mut().mark_dirty(id);
    let n = step_settlement_need_evaluation(
        &mut world,
        &need_catalog,
        &buildings,
        &items,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        3,
    );
    assert_eq!(n, 1);
    assert!(!world.need_evaluation_store().is_dirty(id));
    assert_eq!(
        world
            .need_evaluation_store()
            .get(id)
            .unwrap()
            .evaluated_tick,
        3
    );
}

#[test]
fn cadence_triggers_without_dirty() {
    let id = SettlementId::new(4);
    let mut world = world_with_settlement(id);
    let (need_catalog, buildings, items, categories, profiles) = catalogs();
    let inventory_ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);

    step_settlement_need_evaluation(
        &mut world,
        &need_catalog,
        &buildings,
        &items,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        0,
    );
    let n = step_settlement_need_evaluation(
        &mut world,
        &need_catalog,
        &buildings,
        &items,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        NEED_EVAL_CADENCE_TICKS,
    );
    assert_eq!(n, 1);
}

#[test]
fn snapshots_rebuild_after_save_load_clear() {
    let id = SettlementId::new(5);
    let mut world = world_with_settlement(id);
    let (need_catalog, buildings, items, categories, profiles) = catalogs();
    let inventory_ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);

    evaluate_settlement_needs_now(
        &mut world,
        &need_catalog,
        &buildings,
        &items,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        id,
        20,
    );
    assert!(world.need_evaluation_store().get(id).is_some());

    // Simulate load: discard transient need store; SettlementState remains.
    world.need_evaluation_store_mut().clear();
    assert!(world.need_evaluation_store().get(id).is_none());
    assert!(world.need_evaluation_store().is_dirty(id));

    evaluate_settlement_needs_now(
        &mut world,
        &need_catalog,
        &buildings,
        &items,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        id,
        21,
    );
    let food = world
        .need_evaluation_store()
        .get(id)
        .unwrap()
        .snapshot_str("food")
        .unwrap();
    assert_eq!(food.pressure, 100);
    assert_eq!(food.evaluated_tick, 21);
}

#[test]
fn pressure_normalization_stable_with_modifiers() {
    assert_eq!(normalize_pressure(25.0, 100.0), 75);
    assert_eq!(normalize_pressure(0.0, 0.0), 0);

    let mods = [SettlementModifier {
        source: SettlementModifierSource::Scenario,
        key: "food".into(),
        magnitude: 10.0,
        expires_tick: None,
    }];
    assert_eq!(apply_pressure_modifiers(75, "food", &mods, 0), 85);
    assert_eq!(apply_pressure_modifiers(100, "food", &mods, 0), 100);
}

#[test]
fn research_stub_desired_zero_has_zero_pressure() {
    let id = SettlementId::new(6);
    let mut world = world_with_settlement(id);
    // Ensure research target 0.
    if let Some(state) = world.settlement_state_store_mut().get_mut(id) {
        state.need_targets.push(NeedTarget::new(NeedCategory::Research, 0, 0.1));
    }
    let (need_catalog, buildings, items, categories, profiles) = catalogs();
    let inventory_ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    evaluate_settlement_needs_now(
        &mut world,
        &need_catalog,
        &buildings,
        &items,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        id,
        1,
    );
    let research = world
        .need_evaluation_store()
        .get(id)
        .unwrap()
        .snapshot_str("research")
        .unwrap();
    assert_eq!(research.pressure, 0);
}

#[test]
fn mark_settlement_state_dirty_marks_need_store() {
    let id = SettlementId::new(7);
    let mut world = world_with_settlement(id);
    let (need_catalog, buildings, items, categories, profiles) = catalogs();
    let inventory_ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    evaluate_settlement_needs_now(
        &mut world,
        &need_catalog,
        &buildings,
        &items,
        &inventory_ctx,
        &EmergencyCatalog::default(),
        id,
        1,
    );
    assert!(!world.need_evaluation_store().is_dirty(id));
    crate::world::settlement::mark_settlement_state_dirty(&mut world, id);
    assert!(world.need_evaluation_store().is_dirty(id));
}
