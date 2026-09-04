//! Phase 7 hunger and self-maintenance tests (ADR-134).

use bevy::prelude::{Quat, Vec3};

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::{InventoryCatalogCtx, InventoryEntryContents};
use crate::world::item::{ItemCatalog, ItemCategoryCatalog, ItemDefinition, ItemDefinitionId};
use crate::world::settlement::{
    SettlementOwnership, assign_building_settlement, assign_unit_settlement,
    create_settlement_with_treasury,
};
use crate::world::task::{
    TaskPriority, TaskState, TaskType, assign_construct_building_task, ensure_building_task,
    step_worker_assignment,
};
use crate::world::unit::self_maintenance::{
    FoodSourceRef, HungerStage, NutritionProfile, SelfMaintenanceActivity, SelfMaintenanceContext,
    UnitNutritionState, apply_nutrition_decay, eat_one_from_inventory, evaluate_hunger_stage,
    find_nearest_settlement_edible, hunger_prevents_work_claim, initialize_unit_nutrition,
    is_edible_food, restore_nutrition, select_food_source, step_unit_self_maintenance_pre_work,
    unit_in_active_combat,
};
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingInteractionProfileCatalog,
    BuildingLifecycleState, BuildingOwnership, BuildingSource, ChunkCoord, ChunkData, ChunkId,
    ChunkLayout, CombatState, DoodadCatalog, FootprintCatalog, InventoryProfileCatalog,
    ItemCategoryId, LocalPosition, NavigationConfig, OccupancyCatalogs, PassabilityCatalogs,
    SettlementId, UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource, UnitState,
    WeaponCatalog, WorldData, WorldPosition, create_building_with_inventory,
    create_unit_with_inventory, place_stack_first_fit, starter_building_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions, starter_unit_definitions,
};
use crate::world::{NeedCatalog, evaluate_settlement_needs_now};

struct Phase7Fixture {
    world: WorldData,
    inventory_ctx: InventoryCatalogCtx<'static>,
    items: &'static ItemCatalog,
    building_catalog: BuildingCatalog,
    unit_catalog: UnitCatalog,
    interaction_catalog: BuildingInteractionProfileCatalog,
    weapon_catalog: WeaponCatalog,
    doodad_catalog: DoodadCatalog,
    footprint_catalog: FootprintCatalog,
    nav_config: NavigationConfig,
}

impl Phase7Fixture {
    fn new() -> Self {
        let mut world = flat_world();
        let categories = Box::leak(Box::new(
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap(),
        ));
        let items = Box::leak(Box::new(
            ItemCatalog::from_definitions(starter_item_definitions(), categories).unwrap(),
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
        Self {
            world,
            inventory_ctx,
            items,
            building_catalog,
            unit_catalog,
            interaction_catalog: BuildingInteractionProfileCatalog::default(),
            weapon_catalog: WeaponCatalog::default(),
            doodad_catalog: DoodadCatalog::default(),
            footprint_catalog: FootprintCatalog::default(),
            nav_config: NavigationConfig::default(),
        }
    }

    fn bandit_profile(&self) -> NutritionProfile {
        NutritionProfile::from_definition(
            self.unit_catalog
                .get(&UnitDefinitionId::new("bandit"))
                .unwrap(),
        )
        .unwrap()
    }

    fn hunger_blocks_claim(&self, unit_id: crate::world::UnitId) -> bool {
        hunger_prevents_work_claim(
            &self.world,
            &self.unit_catalog,
            &self.building_catalog,
            &self.interaction_catalog,
            self.items,
            unit_id,
        )
    }

    fn maintenance_ctx(&mut self) -> SelfMaintenanceContext<'_> {
        SelfMaintenanceContext {
            world: &mut self.world,
            unit_catalog: &self.unit_catalog,
            building_catalog: &self.building_catalog,
            interaction_catalog: &self.interaction_catalog,
            item_catalog: self.items,
            inventory_ctx: &self.inventory_ctx,
            passability: PassabilityCatalogs {
                doodad: &self.doodad_catalog,
                building: &self.building_catalog,
                footprint: &self.footprint_catalog,
            },
            nav_config: &self.nav_config,
        }
    }

    fn spawn_settlement(&mut self, x: f32, z: f32) -> (SettlementId, crate::world::BuildingId) {
        let core = create_building_with_inventory(
            &self.building_catalog,
            &mut self.world,
            &BuildingDefinitionId::new("settlement_core"),
            pos(x, z),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            &self.inventory_ctx,
        )
        .unwrap()
        .id;
        self.world.mutate_building(core, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        let report = create_settlement_with_treasury(
            &mut self.world,
            &self.building_catalog,
            &self.interaction_catalog,
            core,
            "P7",
            SettlementOwnership::player_default(),
            pos(x, z),
            0,
        )
        .unwrap();
        (report.settlement_id, core)
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
        let bindings = self
            .world
            .building_inventory_binding_store()
            .get(chest_id)
            .unwrap();
        let inventory_id = bindings.bindings()[0].inventory_id;
        let (store, instances) = self.world.inventory_runtime_mut();
        place_stack_first_fit(
            store,
            instances,
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

    fn stack_quantity(&self, inventory_id: crate::world::InventoryId, item_id: &str) -> u32 {
        let inventory = self.world.inventory_store().get(inventory_id).unwrap();
        inventory
            .placed_entries()
            .iter()
            .filter_map(|entry| {
                if let InventoryEntryContents::Stack {
                    item_definition_id,
                    quantity,
                } = &entry.contents
                {
                    if item_definition_id.as_str() == item_id {
                        Some(*quantity)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .sum()
    }
}

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

fn flat_world() -> WorldData {
    let mut world = WorldData::new(layout());
    let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn set_nutrition(unit_id: crate::world::UnitId, world: &mut WorldData, current: f32) {
    world
        .mutate_unit(unit_id, |record| record.nutrition.current = current)
        .unwrap();
}

fn inventory_quantity_in_unit(
    fx: &Phase7Fixture,
    unit_id: crate::world::UnitId,
    item_id: &str,
) -> u32 {
    let inventory_id = fx.world.get_unit(unit_id).unwrap().inventory_id.unwrap();
    fx.stack_quantity(inventory_id, item_id)
}

// --- STATE / DECAY ---

#[test]
fn new_unit_with_consumption_starts_full() {
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let def = catalog.get(&UnitDefinitionId::new("bandit")).unwrap();
    let mut nutrition = UnitNutritionState::default();
    initialize_unit_nutrition(&mut nutrition, def);
    let profile = NutritionProfile::from_definition(def).unwrap();
    assert_eq!(nutrition.current, profile.max);
}

#[test]
fn created_unit_with_inventory_starts_non_critical() {
    let mut fx = Phase7Fixture::new();
    let unit_id = create_unit_with_inventory(
        &fx.unit_catalog,
        &mut fx.world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
        &fx.inventory_ctx,
    )
    .unwrap()
    .id;
    let profile = fx.bandit_profile();
    let nutrition = fx.world.get_unit(unit_id).unwrap().nutrition.current;
    assert_eq!(nutrition, profile.max);
    assert_eq!(evaluate_hunger_stage(nutrition, &profile), HungerStage::Fed);
    assert!(!fx.hunger_blocks_claim(unit_id));
}

#[test]
fn zero_consumption_rate_does_not_decay() {
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let mut wolf = catalog.get(&UnitDefinitionId::new("wolf")).unwrap().clone();
    wolf.nutrition_consumption_per_tick = 0.0;
    assert!(NutritionProfile::from_definition(&wolf).is_none());
}

#[test]
fn decay_uses_authored_consumption_per_tick() {
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let def = catalog.get(&UnitDefinitionId::new("bandit")).unwrap();
    let profile = NutritionProfile::from_definition(def).unwrap();
    let mut nutrition = UnitNutritionState::full(profile.max);
    apply_nutrition_decay(&mut nutrition, &profile);
    assert_eq!(
        nutrition.current,
        profile.max - def.nutrition_consumption_per_tick
    );
}

#[test]
fn nutrition_never_drops_below_zero() {
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let def = catalog.get(&UnitDefinitionId::new("bandit")).unwrap();
    let profile = NutritionProfile::from_definition(def).unwrap();
    let mut nutrition = UnitNutritionState { current: 0.5 };
    for _ in 0..10 {
        apply_nutrition_decay(&mut nutrition, &profile);
    }
    assert_eq!(nutrition.current, 0.0);
}

#[test]
fn decay_field_matches_phase5_projected_demand_source() {
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let def = catalog.get(&UnitDefinitionId::new("bandit")).unwrap();
    let profile = NutritionProfile::from_definition(def).unwrap();
    assert_eq!(
        profile.consumption_per_tick,
        def.nutrition_consumption_per_tick
    );
}

// --- EATING: OWN INVENTORY ---

#[test]
fn hungry_unit_eats_from_own_inventory() {
    let mut fx = Phase7Fixture::new();
    let unit_id = create_unit_with_inventory(
        &fx.unit_catalog,
        &mut fx.world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
        &fx.inventory_ctx,
    )
    .unwrap()
    .id;
    let inventory_id = fx.world.get_unit(unit_id).unwrap().inventory_id.unwrap();
    let (store, instances) = fx.world.inventory_runtime_mut();
    place_stack_first_fit(
        store,
        instances,
        &fx.inventory_ctx,
        inventory_id,
        ItemDefinitionId::new("prispod"),
        2,
    )
    .unwrap();
    set_nutrition(unit_id, &mut fx.world, 10.0);
    let profile = fx.bandit_profile();
    let mut nutrition = fx.world.get_unit(unit_id).unwrap().nutrition;
    assert!(eat_one_from_inventory(
        &mut fx.world,
        &fx.inventory_ctx,
        unit_id,
        &mut nutrition,
        &profile,
        inventory_id,
        &ItemDefinitionId::new("prispod"),
        fx.items,
    ));
    assert_eq!(inventory_quantity_in_unit(&fx, unit_id, "prispod"), 1);
    assert_eq!(nutrition.current, 35.0);
}

#[test]
fn nutrition_restores_by_item_definition_and_clamps() {
    let mut fx = Phase7Fixture::new();
    let profile = fx.bandit_profile();
    let mut nutrition = UnitNutritionState::full(profile.max - 5.0);
    restore_nutrition(&mut nutrition, 25.0, &profile);
    assert_eq!(nutrition.current, profile.max);
    let mut low = UnitNutritionState { current: 1.0 };
    restore_nutrition(&mut low, 1_000.0, &profile);
    assert_eq!(low.current, profile.max);
}

#[test]
fn zero_nutrition_food_cannot_restore_hunger() {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let zero = ItemDefinition::new(
        ItemDefinitionId::new("stale"),
        "Stale",
        "No nutrition",
        ItemCategoryId::new("food"),
        1,
        1,
        true,
        50,
        100,
        1,
        true,
    );
    let items = ItemCatalog::from_definitions(vec![zero], &categories).unwrap();
    assert!(!is_edible_food(&items, &ItemDefinitionId::new("stale")));
}

#[test]
fn non_food_with_nutrition_field_is_not_edible() {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let ore = ItemDefinition::new(
        ItemDefinitionId::new("nutrient_ore"),
        "Nutrient Ore",
        "Not food.",
        ItemCategoryId::new("raw_material"),
        1,
        1,
        true,
        50,
        500,
        1,
        true,
    )
    .with_nutrition(100);
    let items = ItemCatalog::from_definitions(vec![ore], &categories).unwrap();
    assert!(!is_edible_food(
        &items,
        &ItemDefinitionId::new("nutrient_ore")
    ));
}

// --- EATING: SOURCE ---

#[test]
fn member_locates_own_settlement_edible_source() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let chest = fx.spawn_member_chest(settlement, 32.0, 32.0);
    fx.stock_chest(chest, "prispod", 2);
    let unit_id = fx.spawn_member(settlement, 5.0, 5.0);
    let unit_pos = fx.world.get_unit(unit_id).unwrap().placement.position;
    let edible = find_nearest_settlement_edible(
        &fx.world,
        &fx.building_catalog,
        &fx.interaction_catalog,
        fx.items,
        settlement,
        unit_pos,
        fx.world.layout(),
    )
    .unwrap();
    assert_eq!(edible.item_definition_id.as_str(), "prispod");
}

#[test]
fn other_settlement_storage_not_selected_for_member() {
    let mut fx = Phase7Fixture::new();
    let (home, _) = fx.spawn_settlement(30.0, 30.0);
    let (other, _) = fx.spawn_settlement(300.0, 300.0);
    let other_chest = fx.spawn_member_chest(other, 302.0, 302.0);
    fx.stock_chest(other_chest, "prispod", 5);
    let unit_id = fx.spawn_member(home, 5.0, 5.0);
    let found = select_food_source(
        &fx.world,
        &fx.building_catalog,
        &fx.interaction_catalog,
        fx.items,
        unit_id,
        Some(home),
    );
    assert!(found.is_none());
}

#[test]
fn non_member_does_not_select_settlement_storage() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let chest = fx.spawn_member_chest(settlement, 32.0, 32.0);
    fx.stock_chest(chest, "prispod", 3);
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
    assign_unit_settlement(&mut fx.world, unit.id, None).unwrap();
    let found = select_food_source(
        &fx.world,
        &fx.building_catalog,
        &fx.interaction_catalog,
        fx.items,
        unit.id,
        None,
    );
    assert!(found.is_none());
}

#[test]
fn eat_at_source_without_personal_inventory_transfer() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let chest = fx.spawn_member_chest(settlement, 32.0, 32.0);
    fx.stock_chest(chest, "prispod", 1);
    let unit_id = fx.spawn_member(settlement, 5.0, 5.0);
    let bindings = fx
        .world
        .building_inventory_binding_store()
        .get(chest)
        .unwrap();
    let storage_inventory = bindings.bindings()[0].inventory_id;
    let before_storage = fx.stack_quantity(storage_inventory, "prispod");
    let profile = fx.bandit_profile();
    set_nutrition(unit_id, &mut fx.world, 10.0);
    let mut nutrition = fx.world.get_unit(unit_id).unwrap().nutrition;
    assert!(eat_one_from_inventory(
        &mut fx.world,
        &fx.inventory_ctx,
        unit_id,
        &mut nutrition,
        &profile,
        storage_inventory,
        &ItemDefinitionId::new("prispod"),
        fx.items,
    ));
    assert_eq!(
        fx.stack_quantity(storage_inventory, "prispod"),
        before_storage - 1
    );
    assert_eq!(inventory_quantity_in_unit(&fx, unit_id, "prispod"), 0);
    assert_eq!(nutrition.current, 35.0);
}

#[test]
fn normal_hungry_idle_unit_seeks_food() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let chest = fx.spawn_member_chest(settlement, 32.0, 32.0);
    fx.stock_chest(chest, "prispod", 2);
    let unit_id = fx.spawn_member(settlement, 5.0, 5.0);
    let profile = fx.bandit_profile();
    set_nutrition(unit_id, &mut fx.world, profile.normal_threshold);
    let mut ctx = fx.maintenance_ctx();
    step_unit_self_maintenance_pre_work(&mut ctx);
    let record = fx.world.get_unit(unit_id).unwrap();
    assert!(record.self_maintenance.is_seeking_or_eating());
}

#[test]
fn empty_food_source_clears_activity_for_reseek() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let chest = fx.spawn_member_chest(settlement, 32.0, 32.0);
    let unit_id = fx.spawn_member(settlement, 31.0, 31.0);
    let profile = fx.bandit_profile();
    set_nutrition(unit_id, &mut fx.world, profile.critical_threshold);
    let bindings = fx
        .world
        .building_inventory_binding_store()
        .get(chest)
        .unwrap();
    let inventory_id = bindings.bindings()[0].inventory_id;
    fx.world.mutate_unit(unit_id, |record| {
        record.self_maintenance.activity = SelfMaintenanceActivity::Eating {
            source: FoodSourceRef::SettlementStorage {
                inventory_id,
                building_id: chest,
            },
            stage: HungerStage::Critical,
        };
    });
    let mut ctx = fx.maintenance_ctx();
    step_unit_self_maintenance_pre_work(&mut ctx);
    assert_eq!(
        fx.world
            .get_unit(unit_id)
            .unwrap()
            .self_maintenance
            .activity,
        SelfMaintenanceActivity::None
    );
}

#[test]
fn phase8_policy_propagation_untouched_by_hunger() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let unit_id = fx.spawn_member(settlement, 5.0, 5.0);
    let profile = fx.bandit_profile();
    set_nutrition(unit_id, &mut fx.world, profile.critical_threshold);
    let before = fx.world.building_intent_propagation_store().len();
    let mut ctx = fx.maintenance_ctx();
    step_unit_self_maintenance_pre_work(&mut ctx);
    let after = fx.world.building_intent_propagation_store().len();
    assert_eq!(before, after);
}

#[test]
fn critical_hunger_begins_seeking_distant_food() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let chest = fx.spawn_member_chest(settlement, 80.0, 80.0);
    fx.stock_chest(chest, "prispod", 2);
    let unit_id = fx.spawn_member(settlement, 5.0, 5.0);
    let profile = fx.bandit_profile();
    set_nutrition(unit_id, &mut fx.world, profile.critical_threshold);
    let mut ctx = fx.maintenance_ctx();
    step_unit_self_maintenance_pre_work(&mut ctx);
    let record = fx.world.get_unit(unit_id).unwrap();
    assert!(matches!(
        record.self_maintenance.activity,
        SelfMaintenanceActivity::SeekingFood { .. } | SelfMaintenanceActivity::Eating { .. }
    ));
}

// --- CONTENTION ---

#[test]
fn two_hungry_units_contend_for_last_item_without_duplication() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let chest = fx.spawn_member_chest(settlement, 32.0, 32.0);
    fx.stock_chest(chest, "prispod", 1);
    let unit_a = fx.spawn_member(settlement, 31.0, 31.0);
    let unit_b = fx.spawn_member(settlement, 33.0, 33.0);
    let profile = fx.bandit_profile();
    set_nutrition(unit_a, &mut fx.world, profile.critical_threshold);
    set_nutrition(unit_b, &mut fx.world, profile.critical_threshold);
    let bindings = fx
        .world
        .building_inventory_binding_store()
        .get(chest)
        .unwrap();
    let storage_inventory = bindings.bindings()[0].inventory_id;
    let mut nutrition_a = fx.world.get_unit(unit_a).unwrap().nutrition;
    assert!(eat_one_from_inventory(
        &mut fx.world,
        &fx.inventory_ctx,
        unit_a,
        &mut nutrition_a,
        &profile,
        storage_inventory,
        &ItemDefinitionId::new("prispod"),
        fx.items,
    ));
    let mut nutrition_b = fx.world.get_unit(unit_b).unwrap().nutrition;
    assert!(!eat_one_from_inventory(
        &mut fx.world,
        &fx.inventory_ctx,
        unit_b,
        &mut nutrition_b,
        &profile,
        storage_inventory,
        &ItemDefinitionId::new("prispod"),
        fx.items,
    ));
    assert_eq!(fx.stack_quantity(storage_inventory, "prispod"), 0);
}

// --- URGENCY ---

#[test]
fn normal_hunger_does_not_interrupt_ongoing_work() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let chest = fx.spawn_member_chest(settlement, 32.0, 32.0);
    fx.stock_chest(chest, "prispod", 5);
    let unit_id = fx.spawn_member(settlement, 5.0, 5.0);
    let hut = place_hut(&mut fx);
    assign_construct_building_task(
        &mut fx.world,
        &fx.unit_catalog,
        &fx.weapon_catalog,
        &fx.doodad_catalog,
        &fx.building_catalog,
        &fx.interaction_catalog,
        &fx.nav_config,
        unit_id,
        hut,
        1,
    )
    .unwrap();
    let task_id = fx.world.task_store().unit_task_id(unit_id).unwrap();
    fx.world
        .set_unit_state(unit_id, UnitState::Working { task_id })
        .unwrap();
    let profile = fx.bandit_profile();
    set_nutrition(unit_id, &mut fx.world, profile.normal_threshold);
    let mut ctx = fx.maintenance_ctx();
    step_unit_self_maintenance_pre_work(&mut ctx);
    assert!(matches!(
        fx.world.get_unit(unit_id).unwrap().state,
        UnitState::Working { .. }
    ));
    assert_eq!(fx.world.task_store().unit_task_id(unit_id), Some(task_id));
}

#[test]
fn normal_hungry_idle_blocks_new_work_claim_when_food_available() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let chest = fx.spawn_member_chest(settlement, 32.0, 32.0);
    fx.stock_chest(chest, "prispod", 5);
    let unit_id = fx.spawn_member(settlement, 5.0, 5.0);
    let profile = fx.bandit_profile();
    set_nutrition(unit_id, &mut fx.world, profile.normal_threshold);
    assert!(fx.hunger_blocks_claim(unit_id));
}

#[test]
fn critical_hungry_idle_allows_work_claim_when_no_food_available() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let unit_id = fx.spawn_member(settlement, 5.0, 5.0);
    let profile = fx.bandit_profile();
    set_nutrition(unit_id, &mut fx.world, profile.critical_threshold);
    assert!(!fx.hunger_blocks_claim(unit_id));
}

#[test]
fn critical_hunger_releases_ordinary_work_to_marketplace() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let chest = fx.spawn_member_chest(settlement, 32.0, 32.0);
    fx.stock_chest(chest, "prispod", 5);
    let unit_id = fx.spawn_member(settlement, 5.0, 5.0);
    let hut = place_hut(&mut fx);
    assign_construct_building_task(
        &mut fx.world,
        &fx.unit_catalog,
        &fx.weapon_catalog,
        &fx.doodad_catalog,
        &fx.building_catalog,
        &fx.interaction_catalog,
        &fx.nav_config,
        unit_id,
        hut,
        1,
    )
    .unwrap();
    let task_id = fx.world.task_store().unit_task_id(unit_id).unwrap();
    fx.world
        .set_unit_state(unit_id, UnitState::Working { task_id })
        .unwrap();
    let profile = fx.bandit_profile();
    set_nutrition(unit_id, &mut fx.world, profile.critical_threshold);
    let mut ctx = fx.maintenance_ctx();
    step_unit_self_maintenance_pre_work(&mut ctx);
    assert!(fx.world.task_store().unit_task_id(unit_id).is_none());
    assert_eq!(
        fx.world.task_store().get(task_id).unwrap().state,
        TaskState::Available
    );
}

#[test]
fn critical_hunger_does_not_interrupt_active_combat() {
    let mut fx = Phase7Fixture::new();
    let unit_id = create_unit_with_inventory(
        &fx.unit_catalog,
        &mut fx.world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
        &fx.inventory_ctx,
    )
    .unwrap()
    .id;
    let profile = fx.bandit_profile();
    set_nutrition(unit_id, &mut fx.world, profile.critical_threshold);
    fx.world.mutate_unit(unit_id, |record| {
        record.combat_state = CombatState::Attacking {
            target: crate::world::UnitId::new(99),
        };
    });
    let mut ctx = fx.maintenance_ctx();
    step_unit_self_maintenance_pre_work(&mut ctx);
    let record = fx.world.get_unit(unit_id).unwrap();
    assert!(unit_in_active_combat(&record.combat_state));
    assert!(matches!(
        record.self_maintenance.activity,
        SelfMaintenanceActivity::None
    ));
}

#[test]
fn after_combat_ends_critical_unit_seeks_food() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let chest = fx.spawn_member_chest(settlement, 32.0, 32.0);
    fx.stock_chest(chest, "prispod", 2);
    let unit_id = fx.spawn_member(settlement, 5.0, 5.0);
    let profile = fx.bandit_profile();
    set_nutrition(unit_id, &mut fx.world, profile.critical_threshold);
    fx.world.mutate_unit(unit_id, |record| {
        record.combat_state = CombatState::Peaceful;
    });
    let mut ctx = fx.maintenance_ctx();
    step_unit_self_maintenance_pre_work(&mut ctx);
    assert!(
        fx.world
            .get_unit(unit_id)
            .unwrap()
            .self_maintenance
            .is_seeking_or_eating()
    );
}

#[test]
fn sa7_does_not_trap_critically_hungry_worker_in_reclaim_churn() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let chest = fx.spawn_member_chest(settlement, 32.0, 32.0);
    fx.stock_chest(chest, "prispod", 5);
    let unit_id = fx.spawn_member(settlement, 5.0, 5.0);
    let hut = place_hut(&mut fx);
    ensure_building_task(
        &mut fx.world,
        hut,
        TaskType::ConstructBuilding,
        TaskPriority::Normal,
        1,
    )
    .unwrap();
    let profile = fx.bandit_profile();
    set_nutrition(unit_id, &mut fx.world, profile.critical_threshold);
    let mut ctx = fx.maintenance_ctx();
    step_unit_self_maintenance_pre_work(&mut ctx);
    assert!(fx.hunger_blocks_claim(unit_id));
    let operation_catalog = crate::world::OperationCatalog::default();
    let mut assign_ctx = crate::world::WorkerAssignmentContext {
        world: &mut fx.world,
        unit_catalog: &fx.unit_catalog,
        weapon_catalog: &fx.weapon_catalog,
        doodad_catalog: &fx.doodad_catalog,
        building_catalog: &fx.building_catalog,
        operation_catalog: &operation_catalog,
        interaction_catalog: &fx.interaction_catalog,
        nav_config: &fx.nav_config,
        inventory_ctx: &fx.inventory_ctx,
        simulation_tick: 2,
    };
    let report = step_worker_assignment(&mut assign_ctx);
    assert!(report.assignments.is_empty());
    assert!(fx.world.task_store().unit_task_id(unit_id).is_none());
}

// --- STABILITY ---

#[test]
fn eating_multiple_small_items_restores_fullness() {
    let mut fx = Phase7Fixture::new();
    let unit_id = create_unit_with_inventory(
        &fx.unit_catalog,
        &mut fx.world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
        &fx.inventory_ctx,
    )
    .unwrap()
    .id;
    let inventory_id = fx.world.get_unit(unit_id).unwrap().inventory_id.unwrap();
    let (store, instances) = fx.world.inventory_runtime_mut();
    place_stack_first_fit(
        store,
        instances,
        &fx.inventory_ctx,
        inventory_id,
        ItemDefinitionId::new("prispod"),
        4,
    )
    .unwrap();
    let profile = fx.bandit_profile();
    let mut nutrition = UnitNutritionState {
        current: profile.max - 60.0,
    };
    for _ in 0..3 {
        assert!(eat_one_from_inventory(
            &mut fx.world,
            &fx.inventory_ctx,
            unit_id,
            &mut nutrition,
            &profile,
            inventory_id,
            &ItemDefinitionId::new("prispod"),
            fx.items,
        ));
    }
    assert_eq!(
        evaluate_hunger_stage(nutrition.current, &profile),
        HungerStage::Fed
    );
}

#[test]
fn no_food_reservation_objects_exist_in_hunger_module() {
    let source = include_str!("food.rs");
    assert!(!source.contains("reservation"));
    assert!(!source.contains("Reservation"));
}

// --- BOUNDARY ---

#[test]
fn hunger_does_not_modify_phase5_food_desired_formula() {
    let mut fx = Phase7Fixture::new();
    let (settlement, _) = fx.spawn_settlement(30.0, 30.0);
    let chest = fx.spawn_member_chest(settlement, 32.0, 32.0);
    fx.stock_chest(chest, "prispod", 10);
    let unit_id = fx.spawn_member(settlement, 5.0, 5.0);
    let profile = fx.bandit_profile();
    set_nutrition(unit_id, &mut fx.world, profile.critical_threshold);
    fx.evaluate_food_need(settlement, 1);
    let before = fx.food_desired(settlement);
    let mut ctx = fx.maintenance_ctx();
    step_unit_self_maintenance_pre_work(&mut ctx);
    fx.evaluate_food_need(settlement, 2);
    let after = fx.food_desired(settlement);
    assert_eq!(before, after);
}

#[test]
fn zero_nutrition_does_not_reduce_hp() {
    let mut fx = Phase7Fixture::new();
    let unit_id = create_unit_with_inventory(
        &fx.unit_catalog,
        &mut fx.world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
        &fx.inventory_ctx,
    )
    .unwrap()
    .id;
    let max_hp = fx.world.get_unit(unit_id).unwrap().vitals.max_hp;
    set_nutrition(unit_id, &mut fx.world, 0.0);
    for _ in 0..5 {
        let mut ctx = fx.maintenance_ctx();
        step_unit_self_maintenance_pre_work(&mut ctx);
        crate::world::step_unit_nutrition_decay(&mut ctx);
    }
    let record = fx.world.get_unit(unit_id).unwrap();
    assert_eq!(record.vitals.current_hp, max_hp);
    assert_eq!(record.nutrition.current, 0.0);
}

#[test]
fn edible_food_requires_food_category_and_positive_nutrition() {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
    assert!(is_edible_food(&items, &ItemDefinitionId::new("prispod")));
}

#[test]
fn hunger_stage_thresholds_follow_authored_fractions() {
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let def = catalog.get(&UnitDefinitionId::new("bandit")).unwrap();
    let profile = NutritionProfile::from_definition(def).unwrap();
    assert_eq!(
        evaluate_hunger_stage(profile.max, &profile),
        HungerStage::Fed
    );
    assert_eq!(
        evaluate_hunger_stage(profile.normal_threshold, &profile),
        HungerStage::Normal
    );
    assert_eq!(
        evaluate_hunger_stage(profile.critical_threshold, &profile),
        HungerStage::Critical
    );
}

impl Phase7Fixture {
    fn evaluate_food_need(&mut self, settlement_id: SettlementId, tick: u64) {
        let need_catalog = NeedCatalog::default();
        evaluate_settlement_needs_now(
            &mut self.world,
            &need_catalog,
            &self.building_catalog,
            self.items,
            &self.unit_catalog,
            &self.inventory_ctx,
            &crate::world::EmergencyCatalog::default(),
            settlement_id,
            tick,
        );
    }

    fn food_desired(&self, settlement_id: SettlementId) -> f32 {
        self.world
            .need_evaluation_store()
            .get(settlement_id)
            .unwrap()
            .snapshot_str("food")
            .unwrap()
            .desired_value
    }
}

fn place_hut(fx: &mut Phase7Fixture) -> crate::world::BuildingId {
    let occ = OccupancyCatalogs {
        building: &fx.building_catalog,
        doodad: &fx.doodad_catalog,
        footprint: &fx.footprint_catalog,
    };
    crate::world::place_player_building(
        &fx.building_catalog,
        &mut fx.world,
        &BuildingDefinitionId::new("hut"),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occ,
    )
    .unwrap()
    .id
}
