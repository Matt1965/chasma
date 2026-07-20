//! Settlement production planner integration tests (EP9).

use bevy::prelude::{Quat, Vec3};

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::{InventoryCatalogCtx, place_stack_first_fit};
use crate::world::operation::OperationCatalog;
use crate::world::settlement::{
    SettlementOwnership, StockGoal, create_settlement_with_treasury,
    execute_settlement_replan, reconcile_settlement_building_membership,
};
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingOwnership, BuildingSource, ChunkCoord, ChunkExtent, ItemDefinitionId,
    LocalPosition, OperationDefinitionId, UnitCatalog, WorldData, WorldPosition,
    bootstrap_constant_field, create_building_with_inventory, starter_building_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions, starter_operation_definitions, starter_unit_definitions,
};

fn flat_world() -> WorldData {
    let layout = crate::world::WorldConfig::default().chunk_layout();
    let mut world = WorldData::new(layout);
    world.set_authored_extent(ChunkExtent {
        min: ChunkCoord::new(0, 0),
        max: ChunkCoord::new(1, 1),
    });
    world
}

fn test_inventory_ctx() -> &'static InventoryCatalogCtx<'static> {
    static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
    CTX.get_or_init(|| {
        let categories =
            crate::world::ItemCategoryCatalog::from_definitions(starter_item_category_definitions())
                .unwrap();
        let items =
            crate::world::ItemCatalog::from_definitions(starter_item_definitions(), &categories)
                .unwrap();
        let profiles = crate::world::InventoryProfileCatalog::from_definitions(
            starter_inventory_profile_definitions(),
        )
        .unwrap();
        let items = Box::leak(Box::new(items));
        let categories = Box::leak(Box::new(categories));
        let profiles = Box::leak(Box::new(profiles));
        InventoryCatalogCtx::new(items, categories, profiles)
    })
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

struct PlannerFixture {
    world: WorldData,
    building_catalog: BuildingCatalog,
    operation_catalog: OperationCatalog,
    settlement_id: crate::world::SettlementId,
    chest_id: crate::world::BuildingId,
    mine_id: crate::world::BuildingId,
    smelter_id: crate::world::BuildingId,
}

impl PlannerFixture {
    fn new() -> Self {
        let mut world = flat_world();
        bootstrap_constant_field(
            world.terrain_fields_mut(),
            crate::world::TerrainFieldId::new("iron"),
            ChunkCoord::new(0, 0),
            crate::world::field_value_from_percent(100.0),
        );
        let categories = BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let operation_catalog =
            OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
        let interaction_catalog = crate::world::BuildingInteractionProfileCatalog::default();
        let ctx = test_inventory_ctx();
        let ownership = BuildingOwnership::with_affiliation(Affiliation::Player);
        let settlement_core = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("settlement_core"),
            pos(50.0, 50.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            ownership,
            None,
            ctx,
        )
        .unwrap()
        .id;
        let chest = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(50.0, 50.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            ownership,
            None,
            ctx,
        )
        .unwrap()
        .id;
        let mine = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("iron_mine"),
            pos(10.0, 10.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            ownership,
            None,
            ctx,
        )
        .unwrap()
        .id;
        let smelter = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("smelter"),
            pos(20.0, 20.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            ownership,
            None,
            ctx,
        )
        .unwrap()
        .id;
        for building_id in [settlement_core, mine, smelter] {
            world.mutate_building(building_id, |record| {
                record.lifecycle_state = BuildingLifecycleState::Complete;
            });
        }
        let settlement = create_settlement_with_treasury(
            &mut world,
            &building_catalog,
            &interaction_catalog,
            settlement_core,
            "Test Settlement",
            SettlementOwnership::player_default(),
            pos(50.0, 50.0),
            0,
        )
        .unwrap();
        reconcile_settlement_building_membership(&mut world);
        let planner = world
            .production_planner_store_mut()
            .ensure(settlement.settlement_id);
        planner.stock_goals = vec![StockGoal {
            item_id: ItemDefinitionId::new("iron_bar"),
            maintain_quantity: 10,
            export_threshold: None,
            priority_category: Default::default(),
        }];
        planner.dirty = true;
        Self {
            world,
            building_catalog,
            operation_catalog,
            settlement_id: settlement.settlement_id,
            chest_id: chest,
            mine_id: mine,
            smelter_id: smelter,
        }
    }

    fn replan(&mut self) {
        let ctx = test_inventory_ctx();
        let mut planner = self
            .world
            .production_planner_store()
            .get(self.settlement_id)
            .cloned()
            .unwrap();
        execute_settlement_replan(
            &mut self.world,
            &self.building_catalog,
            &self.operation_catalog,
            ctx,
            self.settlement_id,
            &mut planner,
            1,
        );
        self.world
            .production_planner_store_mut()
            .get_mut(self.settlement_id)
            .last_diagnostics = planner.last_diagnostics;
    }

    fn policy_enabled(&self, building_id: crate::world::BuildingId) -> bool {
        self.world
            .building_production_store()
            .get_policy(building_id)
            .map(|policy| policy.enabled)
            .unwrap_or(false)
    }

    fn selected_operation(&self, building_id: crate::world::BuildingId) -> Option<OperationDefinitionId> {
        self.world
            .building_production_store()
            .get_policy(building_id)
            .and_then(|policy| policy.selected_operation.clone())
    }
}

#[test]
fn ep9_planner_enables_mine_and_smelter_for_iron_bar_demand() {
    let mut fixture = PlannerFixture::new();
    fixture.replan();
    assert_eq!(
        fixture.selected_operation(fixture.mine_id),
        Some(OperationDefinitionId::new("mine_iron"))
    );
    assert!(fixture.policy_enabled(fixture.mine_id));
    assert_eq!(
        fixture.selected_operation(fixture.smelter_id),
        Some(OperationDefinitionId::new("smelt_iron"))
    );
    assert!(fixture.policy_enabled(fixture.smelter_id));
}

#[test]
fn ep9_planner_disables_production_when_goal_met() {
    let mut fixture = PlannerFixture::new();
    let ctx = test_inventory_ctx();
    let chest_bindings = fixture
        .world
        .building_inventory_binding_store()
        .get(fixture.chest_id)
        .unwrap();
    let chest_inventory = chest_bindings
        .bindings()
        .iter()
        .find(|binding| binding.binding_id.as_str() == "primary")
        .map(|binding| binding.inventory_id)
        .unwrap();
    let (inventory_store, instance_store) = fixture.world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        chest_inventory,
        ItemDefinitionId::new("iron_bar"),
        20,
    )
    .unwrap();
    fixture.replan();
    assert!(!fixture.policy_enabled(fixture.mine_id));
    assert!(!fixture.policy_enabled(fixture.smelter_id));
}

#[test]
fn ep9_planner_detects_shortage_when_no_producers() {
    let mut fixture = PlannerFixture::new();
    let planner = fixture
        .world
        .production_planner_store_mut()
        .ensure(fixture.settlement_id);
    planner.stock_goals = vec![StockGoal {
        item_id: ItemDefinitionId::new("nonexistent_item"),
        maintain_quantity: 5,
        export_threshold: None,
        priority_category: Default::default(),
    }];
    fixture.replan();
    let diagnostics = &fixture
        .world
        .production_planner_store()
        .get(fixture.settlement_id)
        .unwrap()
        .last_diagnostics;
    assert!(!diagnostics.validation_errors.is_empty());
}

#[test]
fn ep9_planner_computes_dependency_chain_demand() {
    let mut fixture = PlannerFixture::new();
    fixture.replan();
    let diagnostics = &fixture
        .world
        .production_planner_store()
        .get(fixture.settlement_id)
        .unwrap()
        .last_diagnostics;
    assert!(diagnostics
        .propagated_demand
        .contains_key(&ItemDefinitionId::new("iron_ore")));
}

#[test]
fn ep9_planner_cycle_detection_blocks_invalid_goals() {
    let mut fixture = PlannerFixture::new();
    let planner = fixture
        .world
        .production_planner_store_mut()
        .ensure(fixture.settlement_id);
    planner.stock_goals = vec![
        StockGoal {
            item_id: ItemDefinitionId::new("iron_bar"),
            maintain_quantity: 10,
            export_threshold: None,
            priority_category: Default::default(),
        },
        StockGoal {
            item_id: ItemDefinitionId::new("iron_bar"),
            maintain_quantity: 5,
            export_threshold: None,
            priority_category: Default::default(),
        },
    ];
    fixture.replan();
    let diagnostics = &fixture
        .world
        .production_planner_store()
        .get(fixture.settlement_id)
        .unwrap()
        .last_diagnostics;
    assert!(diagnostics
        .validation_errors
        .iter()
        .any(|msg| msg.contains("Duplicate")));
}

#[test]
fn ep9_planner_survives_save_load() {
    let mut fixture = PlannerFixture::new();
    fixture.replan();
    let exported = fixture
        .world
        .production_planner_store()
        .export_save_state();
    let mut restored = flat_world();
    restored
        .production_planner_store_mut()
        .import_save_state(exported);
    let planner = restored
        .production_planner_store()
        .get(fixture.settlement_id);
    assert!(planner.is_some());
    assert_eq!(planner.unwrap().stock_goals.len(), 1);
}

#[test]
fn ep9_planner_handles_missing_buildings_gracefully() {
    let mut fixture = PlannerFixture::new();
    fixture.world.remove_building_by_id(fixture.mine_id);
    fixture.replan();
    let diagnostics = &fixture
        .world
        .production_planner_store()
        .get(fixture.settlement_id)
        .unwrap()
        .last_diagnostics;
    assert!(diagnostics.shortages.iter().any(|(_, kind)| {
        matches!(
            kind,
            crate::world::PlannerShortageKind::NoOperationalProducers
                | crate::world::PlannerShortageKind::NoProducers
        )
    }));
}

#[test]
fn ep9_planner_multiple_producers_enable_all_candidates() {
    let mut fixture = PlannerFixture::new();
    let ctx = test_inventory_ctx();
    let ownership = BuildingOwnership::with_affiliation(Affiliation::Player);
    let mine_b = create_building_with_inventory(
        &fixture.building_catalog,
        &mut fixture.world,
        &BuildingDefinitionId::new("iron_mine"),
        pos(12.0, 12.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        ownership,
        None,
        ctx,
    )
    .unwrap()
    .id;
    fixture.world.mutate_building(mine_b, |record| {
        record.lifecycle_state = BuildingLifecycleState::Complete;
    });
    reconcile_settlement_building_membership(&mut fixture.world);
    fixture.replan();
    let diagnostics = &fixture
        .world
        .production_planner_store()
        .get(fixture.settlement_id)
        .unwrap()
        .last_diagnostics;
    let enabled_mines = diagnostics
        .chosen_producers
        .iter()
        .filter(|decision| decision.operation_id == OperationDefinitionId::new("mine_iron"))
        .count();
    assert_eq!(enabled_mines, 2);
}
