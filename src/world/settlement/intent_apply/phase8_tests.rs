//! Phase 8 policy ownership tests (ADR-120).

use bevy::prelude::{Quat, Vec3};

use super::*;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::operation::ControlSource;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::operation::OperationCatalog;
use crate::world::settlement::arbiter::{
    IntentId, IntentPersistence, SettlementIntent, SettlementIntentPlan,
    arbitrate_settlement_intent_now,
};
use crate::world::settlement::emergency::EmergencyCatalog;
use crate::world::settlement::needs::{NeedCatalog, evaluate_settlement_needs_now};
use crate::world::settlement::planner::{
    ProductionIntentRequest, StockGoal, execute_settlement_replan, recommend_production_for_intent,
    replan_settlement_production, step_settlement_production_planners,
};
use crate::world::settlement::response::{
    ResponseCatalog, ResponseId, ResponseType, discover_settlement_responses_now,
};
use crate::world::settlement::state::{NeedCategory, NeedTarget, SettlementKind};
use crate::world::settlement::{
    SettlementOwnership, assign_building_settlement, create_settlement_with_treasury,
    priority_category_for_need,
};
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingOwnership, BuildingSource, ChunkCoord, ChunkExtent, ItemDefinitionId, LocalPosition,
    NeedId, OperationDefinitionId, UnitCatalog, WorldData, WorldPosition,
    create_building_with_inventory, starter_building_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions, starter_operation_definitions,
};

struct Phase8Fixture {
    world: WorldData,
    building_catalog: BuildingCatalog,
    operation_catalog: OperationCatalog,
    inventory_ctx: InventoryCatalogCtx<'static>,
    settlement_id: crate::world::SettlementId,
    farm_id: crate::world::BuildingId,
    quarry_id: crate::world::BuildingId,
}

impl Phase8Fixture {
    fn new() -> Self {
        let mut world = flat_world();
        let categories = BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let operation_catalog =
            OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
        let item_categories =
            Box::leak(Box::new(
                crate::world::ItemCategoryCatalog::from_definitions(
                    starter_item_category_definitions(),
                )
                .unwrap(),
            ));
        let items = Box::leak(Box::new(
            crate::world::ItemCatalog::from_definitions(
                starter_item_definitions(),
                item_categories,
            )
            .unwrap(),
        ));
        let profiles = Box::leak(Box::new(
            crate::world::InventoryProfileCatalog::from_definitions(
                starter_inventory_profile_definitions(),
            )
            .unwrap(),
        ));
        let inventory_ctx = InventoryCatalogCtx::new(items, item_categories, profiles);
        let ctx = &inventory_ctx;
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
        let farm = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("prispod_farm"),
            pos(10.0, 10.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            ownership,
            None,
            ctx,
        )
        .unwrap()
        .id;
        let quarry = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("stone_quarry"),
            pos(20.0, 20.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            ownership,
            None,
            ctx,
        )
        .unwrap()
        .id;

        for building_id in [settlement_core, farm, quarry] {
            world.mutate_building(building_id, |record| {
                record.lifecycle_state = BuildingLifecycleState::Complete;
            });
        }

        let interaction_catalog = crate::world::BuildingInteractionProfileCatalog::default();
        let settlement = create_settlement_with_treasury(
            &mut world,
            &building_catalog,
            &interaction_catalog,
            settlement_core,
            "P8",
            SettlementOwnership::player_default(),
            pos(50.0, 50.0),
            0,
        )
        .unwrap();
        for building_id in world.sorted_building_ids() {
            let _ =
                assign_building_settlement(&mut world, building_id, Some(settlement.settlement_id));
        }
        if let Some(state) = world
            .settlement_state_store_mut()
            .get_mut(settlement.settlement_id)
        {
            state.kind = SettlementKind::Town;
            state
                .need_targets
                .push(NeedTarget::new(NeedCategory::Construction, 1, 0.8));
        }

        Self {
            world,
            building_catalog,
            operation_catalog,
            settlement_id: settlement.settlement_id,
            farm_id: farm,
            quarry_id: quarry,
            inventory_ctx,
        }
    }

    fn run_sa_pipeline(&mut self, tick: u64) {
        let need_catalog = NeedCatalog::default();
        let response_catalog = ResponseCatalog::default();
        evaluate_settlement_needs_now(
            &mut self.world,
            &need_catalog,
            &self.building_catalog,
            self.inventory_ctx.items,
            &UnitCatalog::default(),
            &self.inventory_ctx,
            &EmergencyCatalog::default(),
            self.settlement_id,
            tick,
        );
        discover_settlement_responses_now(
            &mut self.world,
            &need_catalog,
            &response_catalog,
            &EmergencyCatalog::default(),
            &self.building_catalog,
            self.settlement_id,
            tick,
        );
        arbitrate_settlement_intent_now(
            &mut self.world,
            &need_catalog,
            &response_catalog,
            self.settlement_id,
            tick,
        );
        propagate_building_intent_now(
            &mut self.world,
            &response_catalog,
            &self.building_catalog,
            &self.operation_catalog,
            &self.inventory_ctx,
            self.settlement_id,
            tick,
        );
    }

    fn policy(
        &self,
        building_id: crate::world::BuildingId,
    ) -> crate::world::BuildingOperationPolicy {
        self.world
            .building_production_store()
            .get_policy(building_id)
            .cloned()
            .unwrap_or_default()
    }

    fn ai_control_building(&mut self, building_id: crate::world::BuildingId, definition_id: &str) {
        let def = self
            .building_catalog
            .get(&BuildingDefinitionId::new(definition_id))
            .unwrap();
        let store = self.world.building_production_store_mut();
        store.ensure_policy_for_building(building_id, def, &self.operation_catalog);
        let policy = store.get_policy_mut(building_id);
        policy.enabled = false;
        policy.control_source = ControlSource::AIControlled;
    }

    fn insert_production_intent(
        &mut self,
        intent_id: &str,
        need_id: &str,
        response_id: &str,
        priority: f32,
        tick: u64,
    ) {
        let plan = SettlementIntentPlan {
            settlement_id: self.settlement_id,
            planned_tick: tick,
            source_response_tick: tick,
            source_need_tick: tick,
            intents: vec![SettlementIntent {
                intent_id: IntentId::new(intent_id),
                source_need: NeedId::new(need_id),
                chosen_response: ResponseId::new(response_id),
                response_type: ResponseType::IncreaseProduction,
                priority,
                desired_persistence: IntentPersistence::UntilPressureLow,
                reasoning: "phase8 test intent".into(),
                quality_explanation: String::new(),
                arbitration: Default::default(),
                diagnostics: Vec::new(),
                ai_seams: Vec::new(),
            }],
            rejected: Vec::new(),
            diagnostics: Vec::new(),
        };
        self.world.settlement_intent_store_mut().insert(plan);
    }

    fn propagate(&mut self, tick: u64) {
        propagate_building_intent_now(
            &mut self.world,
            &ResponseCatalog::default(),
            &self.building_catalog,
            &self.operation_catalog,
            &self.inventory_ctx,
            self.settlement_id,
            tick,
        );
    }
}

fn flat_world() -> WorldData {
    let layout = crate::world::WorldConfig::default().chunk_layout();
    let mut world = WorldData::new(layout);
    world.set_authored_extent(ChunkExtent {
        min: ChunkCoord::new(0, 0),
        max: ChunkCoord::new(1, 1),
    });
    world
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

#[test]
fn ep9_tick_stage_does_not_write_policy() {
    let mut fx = Phase8Fixture::new();
    fx.world
        .production_planner_store_mut()
        .get_mut(fx.settlement_id)
        .stock_goals = vec![StockGoal {
        item_id: ItemDefinitionId::new("prispod"),
        maintain_quantity: 100,
        export_threshold: None,
        priority_category: Default::default(),
    }];
    fx.world
        .production_planner_store_mut()
        .get_mut(fx.settlement_id)
        .dirty = true;

    let before_farm = fx.policy(fx.farm_id).enabled;
    let replanned = step_settlement_production_planners(
        &mut fx.world,
        &fx.building_catalog,
        &fx.operation_catalog,
        &fx.inventory_ctx,
        1,
    );
    assert_eq!(replanned, 0);
    assert_eq!(fx.policy(fx.farm_id).enabled, before_farm);
}

#[test]
fn food_intent_recommendation_uses_graph_not_building_name() {
    let fx = Phase8Fixture::new();
    let planner = fx
        .world
        .production_planner_store()
        .get(fx.settlement_id)
        .cloned()
        .unwrap_or_default();
    let request = ProductionIntentRequest {
        settlement_id: fx.settlement_id,
        need_id: NeedId::new("food"),
        operation_hint: OperationDefinitionId::new("grow_prispods"),
        demand_quantity: 10,
        enable: true,
        priority_category: priority_category_for_need(&NeedId::new("food")),
        reason: "test food intent".into(),
    };
    let (decisions, _) = recommend_production_for_intent(
        &fx.world,
        &fx.building_catalog,
        &fx.operation_catalog,
        &fx.inventory_ctx,
        &planner,
        &request,
        1,
    );
    assert!(decisions.iter().any(|d| d.building_id == fx.farm_id));
    assert!(
        decisions
            .iter()
            .all(|d| d.operation_id.as_str() == "grow_prispods")
    );
}

#[test]
fn materials_intent_enables_quarry_through_sa5() {
    let mut fx = Phase8Fixture::new();
    {
        let store = fx.world.building_production_store_mut();
        let def = fx
            .building_catalog
            .get(&BuildingDefinitionId::new("stone_quarry"))
            .unwrap();
        store.ensure_policy_for_building(fx.quarry_id, def, &fx.operation_catalog);
        store.get_policy_mut(fx.quarry_id).enabled = false;
        store.get_policy_mut(fx.quarry_id).control_source = ControlSource::AIControlled;
    }
    fx.run_sa_pipeline(1);
    let policy = fx.policy(fx.quarry_id);
    assert!(policy.enabled);
    assert_eq!(
        policy.selected_operation.as_ref().map(|o| o.as_str()),
        Some("mine_stone")
    );
}

#[test]
fn stock_goal_replan_alone_does_not_mutate_policy_without_sa5() {
    let fx = Phase8Fixture::new();
    let mut planner = fx
        .world
        .production_planner_store()
        .get(fx.settlement_id)
        .cloned()
        .unwrap_or_default();
    planner.stock_goals = vec![StockGoal {
        item_id: ItemDefinitionId::new("prispod"),
        maintain_quantity: 50,
        export_threshold: None,
        priority_category: Default::default(),
    }];
    let before = fx.policy(fx.farm_id);
    let (decisions, _) = replan_settlement_production(
        &fx.world,
        &fx.building_catalog,
        &fx.operation_catalog,
        &fx.inventory_ctx,
        fx.settlement_id,
        &planner,
        1,
    );
    assert!(!decisions.is_empty());
    assert_eq!(fx.policy(fx.farm_id), before);
}

#[test]
fn food_policy_releases_when_no_food_intent_selected() {
    let mut fx = Phase8Fixture::new();
    {
        let store = fx.world.building_production_store_mut();
        let def = fx
            .building_catalog
            .get(&BuildingDefinitionId::new("prispod_farm"))
            .unwrap();
        store.ensure_policy_for_building(fx.farm_id, def, &fx.operation_catalog);
        store.get_policy_mut(fx.farm_id).enabled = false;
        store.get_policy_mut(fx.farm_id).control_source = ControlSource::AIControlled;
    }
    fx.run_sa_pipeline(1);
    assert!(fx.policy(fx.farm_id).enabled);
    assert_eq!(
        fx.policy(fx.farm_id).control_source,
        ControlSource::AIControlled
    );

    if let Some(mut plan) = fx
        .world
        .settlement_intent_store()
        .get(fx.settlement_id)
        .cloned()
    {
        plan.intents.clear();
        fx.world.settlement_intent_store_mut().insert(plan);
    }
    propagate_building_intent_now(
        &mut fx.world,
        &ResponseCatalog::default(),
        &fx.building_catalog,
        &fx.operation_catalog,
        &fx.inventory_ctx,
        fx.settlement_id,
        2,
    );
    assert!(!fx.policy(fx.farm_id).enabled);
}

#[test]
fn player_controlled_producer_skipped_by_sa5_not_ep9() {
    let mut fx = Phase8Fixture::new();
    let pinned_operation = OperationDefinitionId::new("grow_prispods");
    {
        let store = fx.world.building_production_store_mut();
        let def = fx
            .building_catalog
            .get(&BuildingDefinitionId::new("prispod_farm"))
            .unwrap();
        store.ensure_policy_for_building(fx.farm_id, def, &fx.operation_catalog);
        let policy = store.get_policy_mut(fx.farm_id);
        policy.control_source = ControlSource::PlayerControlled;
        policy.enabled = false;
        policy.selected_operation = Some(pinned_operation.clone());
    }
    let planner = fx
        .world
        .production_planner_store()
        .get(fx.settlement_id)
        .cloned()
        .unwrap_or_default();
    let request = ProductionIntentRequest {
        settlement_id: fx.settlement_id,
        need_id: NeedId::new("food"),
        operation_hint: pinned_operation.clone(),
        demand_quantity: 10,
        enable: true,
        priority_category: priority_category_for_need(&NeedId::new("food")),
        reason: "test".into(),
    };
    let (decisions, _) = recommend_production_for_intent(
        &fx.world,
        &fx.building_catalog,
        &fx.operation_catalog,
        &fx.inventory_ctx,
        &planner,
        &request,
        1,
    );
    assert!(decisions.iter().any(|d| d.building_id == fx.farm_id));

    fx.run_sa_pipeline(1);
    let policy = fx.policy(fx.farm_id);
    assert_eq!(policy.control_source, ControlSource::PlayerControlled);
    assert!(!policy.enabled);
    assert_eq!(policy.selected_operation.as_ref(), Some(&pinned_operation));
}

#[test]
fn intent_shift_releases_food_and_enables_materials() {
    let mut fx = Phase8Fixture::new();
    fx.ai_control_building(fx.farm_id, "prispod_farm");
    fx.ai_control_building(fx.quarry_id, "stone_quarry");

    fx.insert_production_intent("food-1", "food", "increase_food_production", 200.0, 1);
    fx.propagate(1);
    assert!(fx.policy(fx.farm_id).enabled);
    assert!(!fx.policy(fx.quarry_id).enabled);

    fx.insert_production_intent(
        "materials-1",
        "materials",
        "increase_construction_materials",
        200.0,
        2,
    );
    fx.propagate(2);
    assert!(!fx.policy(fx.farm_id).enabled);
    assert!(fx.policy(fx.quarry_id).enabled);
    assert_eq!(
        fx.policy(fx.quarry_id)
            .selected_operation
            .as_ref()
            .map(|o| o.as_str()),
        Some("mine_stone")
    );
}

#[test]
fn execute_settlement_replan_does_not_mutate_policy_without_sa5() {
    let mut fx = Phase8Fixture::new();
    fx.ai_control_building(fx.farm_id, "prispod_farm");
    let before = fx.policy(fx.farm_id);
    let mut planner = fx
        .world
        .production_planner_store()
        .get(fx.settlement_id)
        .cloned()
        .unwrap_or_default();
    planner.stock_goals = vec![StockGoal {
        item_id: ItemDefinitionId::new("prispod"),
        maintain_quantity: 100,
        export_threshold: None,
        priority_category: Default::default(),
    }];
    execute_settlement_replan(
        &mut fx.world,
        &fx.building_catalog,
        &fx.operation_catalog,
        &fx.inventory_ctx,
        fx.settlement_id,
        &mut planner,
        1,
    );
    assert_eq!(fx.policy(fx.farm_id), before);
    assert!(!planner.last_diagnostics.chosen_producers.is_empty());
}
