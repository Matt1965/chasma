//! Phase 5 settlement need sensing tests — food nutrition + materials count (ADR-117).

use bevy::prelude::{Quat, Vec3};

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::item::{ItemCatalog, ItemCategoryCatalog, ItemDefinition, ItemDefinitionId};
use crate::world::settlement::needs::{
    DEFAULT_FOOD_PLANNING_HORIZON_TICKS, NeedCatalog, evaluate_settlement_needs_now,
    normalize_pressure,
};
use crate::world::settlement::response::ResponseCatalog;
use crate::world::settlement::state::{NeedCategory, NeedTarget};
use crate::world::settlement::{
    SettlementOwnership, assign_building_settlement, assign_unit_settlement,
    create_settlement_with_treasury,
};
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingOwnership, BuildingSource, ChunkCoord, ChunkData, ChunkLayout, Heightfield,
    InventoryProfileCatalog, ItemCategoryId, LocalPosition, SettlementId, UnitCatalog,
    UnitDefinitionId, UnitOwnership, UnitSource, UnitState, WorldData, WorldPosition,
    create_building_with_inventory, create_unit_with_inventory, place_stack_first_fit,
    starter_building_definitions, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions, starter_unit_definitions,
};

struct Phase5Fixture {
    world: WorldData,
    inventory_ctx: InventoryCatalogCtx<'static>,
    building_catalog: BuildingCatalog,
    unit_catalog: UnitCatalog,
    settlement_a: SettlementId,
    settlement_b: SettlementId,
}

impl Phase5Fixture {
    fn new() -> Self {
        let mut world = test_world();
        let categories = Box::leak(Box::new(
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap(),
        ));
        let mut item_defs = starter_item_definitions();
        item_defs.push(
            ItemDefinition::new(
                ItemDefinitionId::new("prepared_meal"),
                "Prepared Meal",
                "High-nutrition prepared food.",
                ItemCategoryId::new("food"),
                1,
                1,
                true,
                50,
                200,
                1,
                true,
            )
            .with_nutrition(200),
        );
        item_defs.push(
            ItemDefinition::new(
                ItemDefinitionId::new("stale_bread"),
                "Stale Bread",
                "Food with no nutrition value.",
                ItemCategoryId::new("food"),
                1,
                1,
                true,
                50,
                100,
                1,
                true,
            )
            .with_nutrition(0),
        );
        item_defs.push(
            ItemDefinition::new(
                ItemDefinitionId::new("nutrient_ore"),
                "Nutrient Ore",
                "Non-food item with nutrition field set.",
                ItemCategoryId::new("raw_material"),
                1,
                1,
                true,
                50,
                500,
                1,
                true,
            )
            .with_nutrition(100),
        );
        let items = Box::leak(Box::new(
            ItemCatalog::from_definitions(item_defs, categories).unwrap(),
        ));
        let profiles = Box::leak(Box::new(
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap(),
        ));
        let inventory_ctx = InventoryCatalogCtx::new(items, categories, profiles);
        let building_catalog = BuildingCatalog::from_definitions(
            starter_building_definitions(),
            &BuildingCategoryCatalog::default(),
        )
        .unwrap();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();

        let settlement_a = spawn_settlement(&mut world, 30.0, 30.0, "A");
        let settlement_b = spawn_settlement(&mut world, 200.0, 200.0, "B");

        Self {
            world,
            inventory_ctx,
            building_catalog,
            unit_catalog,
            settlement_a,
            settlement_b,
        }
    }

    fn evaluate(&mut self, settlement_id: SettlementId, tick: u64) {
        let need_catalog = NeedCatalog::default();
        evaluate_settlement_needs_now(
            &mut self.world,
            &need_catalog,
            &self.building_catalog,
            self.inventory_ctx.items,
            &self.unit_catalog,
            &self.inventory_ctx,
            &crate::world::settlement::emergency::EmergencyCatalog::default(),
            settlement_id,
            tick,
        );
    }

    fn food_snapshot(
        &self,
        settlement_id: SettlementId,
    ) -> crate::world::settlement::needs::NeedSnapshot {
        self.world
            .need_evaluation_store()
            .get(settlement_id)
            .unwrap()
            .snapshot_str("food")
            .unwrap()
            .clone()
    }

    fn materials_snapshot(
        &self,
        settlement_id: SettlementId,
    ) -> crate::world::settlement::needs::NeedSnapshot {
        self.world
            .need_evaluation_store()
            .get(settlement_id)
            .unwrap()
            .snapshot_str("materials")
            .unwrap()
            .clone()
    }

    fn construction_snapshot(
        &self,
        settlement_id: SettlementId,
    ) -> crate::world::settlement::needs::NeedSnapshot {
        self.world
            .need_evaluation_store()
            .get(settlement_id)
            .unwrap()
            .snapshot_str("construction")
            .unwrap()
            .clone()
    }

    fn spawn_member_chest(
        &mut self,
        settlement_id: SettlementId,
        x: f32,
        z: f32,
    ) -> crate::world::BuildingId {
        let chest = create_building_with_inventory(
            &self.building_catalog,
            &mut self.world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(x, z),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            &self.inventory_ctx,
        )
        .unwrap()
        .id;
        assign_building_settlement(&mut self.world, chest, Some(settlement_id)).unwrap();
        chest
    }

    fn stock_chest(&mut self, chest_id: crate::world::BuildingId, item_id: &str, quantity: u32) {
        let binding_store = self.world.building_inventory_binding_store();
        let bindings = binding_store.get(chest_id).unwrap();
        let inventory_id = bindings.bindings()[0].inventory_id;
        let (inventory_store, instance_store) = self.world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            &self.inventory_ctx,
            inventory_id,
            ItemDefinitionId::new(item_id),
            quantity,
        )
        .unwrap();
    }

    fn spawn_member(
        &mut self,
        settlement_id: SettlementId,
        x: f32,
        z: f32,
    ) -> crate::world::UnitId {
        let unit = create_unit_with_inventory(
            &self.unit_catalog,
            &mut self.world,
            &UnitDefinitionId::new("bandit"),
            pos(x, z),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            &self.inventory_ctx,
        )
        .unwrap();
        assign_unit_settlement(&mut self.world, unit.id, Some(settlement_id)).unwrap();
        unit.id
    }
}

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn test_world() -> WorldData {
    let mut world = WorldData::new(layout());
    let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
    world.insert(
        crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn spawn_settlement(world: &mut WorldData, x: f32, z: f32, name: &str) -> SettlementId {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
    let profiles =
        InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions()).unwrap();
    let building_catalog = BuildingCatalog::from_definitions(
        starter_building_definitions(),
        &BuildingCategoryCatalog::default(),
    )
    .unwrap();
    let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    let core = create_building_with_inventory(
        &building_catalog,
        world,
        &BuildingDefinitionId::new("settlement_core"),
        pos(x, z),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        &ctx,
    )
    .unwrap()
    .id;
    world.mutate_building(core, |record| {
        record.lifecycle_state = BuildingLifecycleState::Complete;
    });
    create_settlement_with_treasury(
        world,
        &building_catalog,
        &crate::world::BuildingInteractionProfileCatalog::default(),
        core,
        name,
        SettlementOwnership::player_default(),
        pos(x, z),
        0,
    )
    .unwrap()
    .settlement_id
}

#[test]
fn food_current_sums_quantity_times_nutrition() {
    let mut fx = Phase5Fixture::new();
    let chest = fx.spawn_member_chest(fx.settlement_a, 10.0, 10.0);
    fx.stock_chest(chest, "prispod", 10);
    fx.evaluate(fx.settlement_a, 1);
    let food = fx.food_snapshot(fx.settlement_a);
    assert_eq!(food.current_value, 250.0);
}

#[test]
fn food_current_differs_by_nutrition_not_quantity_alone() {
    let mut fx = Phase5Fixture::new();
    let chest_a = fx.spawn_member_chest(fx.settlement_a, 10.0, 10.0);
    let chest_b = fx.spawn_member_chest(fx.settlement_a, 12.0, 12.0);
    fx.stock_chest(chest_a, "prispod", 10);
    fx.stock_chest(chest_b, "prepared_meal", 10);
    fx.evaluate(fx.settlement_a, 1);
    let food = fx.food_snapshot(fx.settlement_a);
    assert_eq!(food.current_value, 10.0 * 25.0 + 10.0 * 200.0);
    assert_ne!(250.0, 2000.0);
}

#[test]
fn zero_nutrition_food_contributes_zero() {
    let mut fx = Phase5Fixture::new();
    let chest = fx.spawn_member_chest(fx.settlement_a, 10.0, 10.0);
    fx.stock_chest(chest, "stale_bread", 20);
    fx.evaluate(fx.settlement_a, 1);
    assert_eq!(fx.food_snapshot(fx.settlement_a).current_value, 0.0);
}

#[test]
fn non_food_nutrition_does_not_count_toward_food_supply() {
    let mut fx = Phase5Fixture::new();
    let chest = fx.spawn_member_chest(fx.settlement_a, 10.0, 10.0);
    fx.stock_chest(chest, "nutrient_ore", 10);
    fx.evaluate(fx.settlement_a, 1);
    assert_eq!(fx.food_snapshot(fx.settlement_a).current_value, 0.0);
}

#[test]
fn non_member_building_stock_does_not_contribute() {
    let mut fx = Phase5Fixture::new();
    let chest = create_building_with_inventory(
        &fx.building_catalog,
        &mut fx.world,
        &BuildingDefinitionId::new("storage_chest"),
        pos(500.0, 500.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        &fx.inventory_ctx,
    )
    .unwrap()
    .id;
    fx.stock_chest(chest, "prispod", 10);
    fx.evaluate(fx.settlement_a, 1);
    assert_eq!(fx.food_snapshot(fx.settlement_a).current_value, 0.0);
}

#[test]
fn other_settlement_stock_does_not_leak() {
    let mut fx = Phase5Fixture::new();
    let chest = fx.spawn_member_chest(fx.settlement_b, 10.0, 10.0);
    fx.stock_chest(chest, "prispod", 10);
    fx.evaluate(fx.settlement_a, 1);
    assert_eq!(fx.food_snapshot(fx.settlement_a).current_value, 0.0);
}

#[test]
fn one_live_member_creates_projected_food_demand() {
    let mut fx = Phase5Fixture::new();
    fx.spawn_member(fx.settlement_a, 5.0, 5.0);
    fx.evaluate(fx.settlement_a, 1);
    let food = fx.food_snapshot(fx.settlement_a);
    let projected = DEFAULT_FOOD_PLANNING_HORIZON_TICKS as f32;
    assert_eq!(food.desired_value, projected + 100.0);
}

#[test]
fn more_members_increase_food_desired() {
    let mut fx = Phase5Fixture::new();
    fx.spawn_member(fx.settlement_a, 5.0, 5.0);
    fx.evaluate(fx.settlement_a, 1);
    let one = fx.food_snapshot(fx.settlement_a).desired_value;
    fx.spawn_member(fx.settlement_a, 6.0, 6.0);
    fx.evaluate(fx.settlement_a, 2);
    let two = fx.food_snapshot(fx.settlement_a).desired_value;
    assert!(two > one);
    assert_eq!(two - one, DEFAULT_FOOD_PLANNING_HORIZON_TICKS as f32);
}

#[test]
fn non_member_does_not_increase_food_desired() {
    let mut fx = Phase5Fixture::new();
    let unit = create_unit_with_inventory(
        &fx.unit_catalog,
        &mut fx.world,
        &UnitDefinitionId::new("bandit"),
        pos(5.0, 5.0),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
        &fx.inventory_ctx,
    )
    .unwrap();
    fx.evaluate(fx.settlement_a, 1);
    let before = fx.food_snapshot(fx.settlement_a).desired_value;
    let _ = unit;
    fx.evaluate(fx.settlement_a, 2);
    assert_eq!(fx.food_snapshot(fx.settlement_a).desired_value, before);
}

#[test]
fn other_settlement_member_does_not_increase_food_desired() {
    let mut fx = Phase5Fixture::new();
    fx.spawn_member(fx.settlement_b, 5.0, 5.0);
    fx.evaluate(fx.settlement_a, 1);
    assert_eq!(fx.food_snapshot(fx.settlement_a).desired_value, 100.0);
}

#[test]
fn dead_member_does_not_increase_food_desired() {
    let mut fx = Phase5Fixture::new();
    let unit = fx.spawn_member(fx.settlement_a, 5.0, 5.0);
    fx.world.set_unit_state(unit, UnitState::Dead).unwrap();
    fx.evaluate(fx.settlement_a, 1);
    assert_eq!(fx.food_snapshot(fx.settlement_a).desired_value, 100.0);
}

#[test]
fn different_unit_consumption_rates_change_demand() {
    let mut fx = Phase5Fixture::new();
    let mut defs = starter_unit_definitions();
    defs[1] = defs[1].clone().with_nutrition_consumption_per_tick(2.0);
    fx.unit_catalog = UnitCatalog::from_definitions(defs).unwrap();
    fx.spawn_member(fx.settlement_a, 5.0, 5.0);
    fx.evaluate(fx.settlement_a, 1);
    let fast = fx.food_snapshot(fx.settlement_a).desired_value;

    let mut fx2 = Phase5Fixture::new();
    fx2.spawn_member(fx2.settlement_a, 5.0, 5.0);
    fx2.evaluate(fx2.settlement_a, 1);
    let slow = fx2.food_snapshot(fx2.settlement_a).desired_value;
    assert!(fast > slow);
}

#[test]
fn food_desired_is_projected_consumption_plus_reserve() {
    let mut fx = Phase5Fixture::new();
    if let Some(state) = fx
        .world
        .settlement_state_store_mut()
        .get_mut(fx.settlement_a)
    {
        state.need_targets = vec![NeedTarget::new(NeedCategory::Food, 250, 1.0)];
    }
    fx.spawn_member(fx.settlement_a, 5.0, 5.0);
    fx.evaluate(fx.settlement_a, 1);
    let food = fx.food_snapshot(fx.settlement_a);
    assert_eq!(
        food.desired_value,
        DEFAULT_FOOD_PLANNING_HORIZON_TICKS as f32 + 250.0
    );
}

#[test]
fn reserve_shortage_keeps_low_nonzero_pressure() {
    let mut fx = Phase5Fixture::new();
    fx.spawn_member(fx.settlement_a, 5.0, 5.0);
    let desired = DEFAULT_FOOD_PLANNING_HORIZON_TICKS as f32 + 100.0;
    let chest = fx.spawn_member_chest(fx.settlement_a, 10.0, 10.0);
    fx.stock_chest(chest, "prepared_meal", 4);
    fx.stock_chest(chest, "prispod", 6);
    fx.evaluate(fx.settlement_a, 1);
    let food = fx.food_snapshot(fx.settlement_a);
    assert_eq!(food.current_value, 950.0);
    assert_eq!(food.desired_value, desired);
    assert!(food.pressure > 0);
    assert!(food.pressure < 20);
}

#[test]
fn food_pressure_zero_when_fully_satisfied() {
    let mut fx = Phase5Fixture::new();
    let chest = fx.spawn_member_chest(fx.settlement_a, 10.0, 10.0);
    fx.stock_chest(chest, "prepared_meal", 1);
    fx.evaluate(fx.settlement_a, 1);
    let food = fx.food_snapshot(fx.settlement_a);
    assert_eq!(food.desired_value, 100.0);
    assert_eq!(food.current_value, 200.0);
    assert_eq!(food.pressure, 0);
}

#[test]
fn materials_current_counts_construction_material_category() {
    let mut fx = Phase5Fixture::new();
    let chest = fx.spawn_member_chest(fx.settlement_a, 10.0, 10.0);
    fx.stock_chest(chest, "stone", 15);
    fx.evaluate(fx.settlement_a, 1);
    assert_eq!(fx.materials_snapshot(fx.settlement_a).current_value, 15.0);
}

#[test]
fn raw_materials_do_not_count_toward_materials_need() {
    let mut fx = Phase5Fixture::new();
    let chest = fx.spawn_member_chest(fx.settlement_a, 10.0, 10.0);
    fx.stock_chest(chest, "iron_ore", 20);
    fx.stock_chest(chest, "coal", 30);
    fx.evaluate(fx.settlement_a, 1);
    assert_eq!(fx.materials_snapshot(fx.settlement_a).current_value, 0.0);
}

#[test]
fn materials_desired_from_authored_target() {
    let mut fx = Phase5Fixture::new();
    if let Some(state) = fx
        .world
        .settlement_state_store_mut()
        .get_mut(fx.settlement_a)
    {
        state.need_targets = vec![
            NeedTarget::new(NeedCategory::Food, 100, 1.0),
            NeedTarget::new(NeedCategory::Materials, 80, 0.5),
        ];
    }
    fx.evaluate(fx.settlement_a, 1);
    assert_eq!(fx.materials_snapshot(fx.settlement_a).desired_value, 80.0);
}

#[test]
fn construction_need_remains_backlog_not_material_stock() {
    let mut fx = Phase5Fixture::new();
    let chest = fx.spawn_member_chest(fx.settlement_a, 40.0, 40.0);
    fx.stock_chest(chest, "stone", 50);
    fx.evaluate(fx.settlement_a, 1);
    let construction = fx.construction_snapshot(fx.settlement_a);
    assert_eq!(
        construction.evaluation_source,
        "construction_sites incomplete=0"
    );
    assert_eq!(fx.materials_snapshot(fx.settlement_a).current_value, 50.0);
}

#[test]
fn increase_construction_materials_targets_materials_need() {
    let responses = ResponseCatalog::default();
    let response = responses
        .get_str("increase_construction_materials")
        .unwrap();
    assert!(response.supports_need(&crate::world::settlement::needs::NeedId::new("materials")));
    assert!(
        !response.supports_need(&crate::world::settlement::needs::NeedId::new(
            "construction"
        ))
    );
}

#[test]
fn need_pressure_remains_unweighted_objective() {
    let mut fx = Phase5Fixture::new();
    if let Some(state) = fx
        .world
        .settlement_state_store_mut()
        .get_mut(fx.settlement_a)
    {
        state.need_targets = vec![NeedTarget::new(NeedCategory::Food, 100, 0.01)];
    }
    fx.evaluate(fx.settlement_a, 1);
    let low_weight = fx.food_snapshot(fx.settlement_a).pressure;

    let mut fx2 = Phase5Fixture::new();
    if let Some(state) = fx2
        .world
        .settlement_state_store_mut()
        .get_mut(fx2.settlement_a)
    {
        state.need_targets = vec![NeedTarget::new(NeedCategory::Food, 100, 99.0)];
    }
    fx2.evaluate(fx2.settlement_a, 1);
    let high_weight = fx2.food_snapshot(fx2.settlement_a).pressure;
    assert_eq!(low_weight, high_weight);
    assert_eq!(low_weight, normalize_pressure(0.0, 100.0));
}

#[test]
fn no_population_need_in_catalog() {
    let catalog = NeedCatalog::default();
    assert!(catalog.get_str("population").is_none());
}

#[test]
fn no_item_id_special_cases_required() {
    let mut fx = Phase5Fixture::new();
    let chest = fx.spawn_member_chest(fx.settlement_a, 10.0, 10.0);
    fx.stock_chest(chest, "stone", 5);
    fx.stock_chest(chest, "prispod", 4);
    fx.evaluate(fx.settlement_a, 1);
    assert_eq!(fx.materials_snapshot(fx.settlement_a).current_value, 5.0);
    assert_eq!(fx.food_snapshot(fx.settlement_a).current_value, 100.0);
}

#[test]
fn item_nutrition_import_unchanged() {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
    assert_eq!(
        items
            .get(&ItemDefinitionId::new("prispod"))
            .unwrap()
            .nutrition,
        25
    );
}
