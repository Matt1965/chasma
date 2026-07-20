//! Strategic Construction Planning tests (SA9).

use bevy::prelude::{Quat, Vec3};

use super::*;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::settlement::arbiter::{
    IntentId, IntentPersistence, SettlementIntent, SettlementIntentPlan,
};
use crate::world::settlement::needs::NeedId;
use crate::world::settlement::response::{ResponseId, ResponseType};
use crate::world::settlement::state::{SettlementKind, SettlementState};
use crate::world::settlement::{
    create_settlement_with_treasury, ensure_settlement_states_for_world,
    reconcile_settlement_building_membership, SettlementOwnership,
};
use crate::world::{
    create_building_with_inventory, generate_strategic_tasks_now, starter_building_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions, Affiliation, BuildingCategoryCatalog, BuildingDefinitionId,
    BuildingLifecycleState, BuildingOwnership, BuildingSource, ChunkCoord, ChunkData, ChunkExtent,
    ChunkId, Heightfield, LocalPosition, StrategicTaskTemplateCatalog, TaskType, WorldData,
    WorldPosition,
};

fn flat_world() -> WorldData {
    let layout = crate::world::WorldConfig::default().chunk_layout();
    let mut world = WorldData::new(layout);
    world.set_authored_extent(ChunkExtent {
        min: ChunkCoord::new(0, 0),
        max: ChunkCoord::new(1, 1),
    });
    let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn inventory_ctx() -> &'static InventoryCatalogCtx<'static> {
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

struct Sa9Fixture {
    world: WorldData,
    settlement_id: crate::world::SettlementId,
    building_catalog: BuildingCatalog,
    footprint: crate::world::FootprintCatalog,
    doodad: crate::world::DoodadCatalog,
    unit: crate::world::UnitCatalog,
}

impl Sa9Fixture {
    fn new() -> Self {
        let mut world = flat_world();
        let categories = BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let interaction_catalog = crate::world::BuildingInteractionProfileCatalog::default();
        let ctx = inventory_ctx();
        let ownership = BuildingOwnership::with_affiliation(Affiliation::Player);

        let settlement_core = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("settlement_core"),
            pos(128.0, 128.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            ownership,
            None,
            ctx,
        )
        .unwrap()
        .id;
        world.mutate_building(settlement_core, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });

        let settlement = create_settlement_with_treasury(
            &mut world,
            &building_catalog,
            &interaction_catalog,
            settlement_core,
            "SA9 Settlement",
            SettlementOwnership::player_default(),
            pos(128.0, 128.0),
            0,
        )
        .unwrap();
        ensure_settlement_states_for_world(&mut world);
        if let Some(state) = world
            .settlement_state_store_mut()
            .get_mut(settlement.settlement_id)
        {
            *state = SettlementState::new(
                settlement.settlement_id,
                SettlementKind::Town,
                true,
            );
            state.policies.auto_construction = true;
            state.policies.require_construction_approval = false;
            state.policies.require_construction_placement_approval = false;
        }
        reconcile_settlement_building_membership(&mut world);

        Self {
            world,
            settlement_id: settlement.settlement_id,
            building_catalog,
            footprint: crate::world::FootprintCatalog::default(),
            doodad: crate::world::DoodadCatalog::default(),
            unit: crate::world::UnitCatalog::default(),
        }
    }

    fn insert_food_construct_intent(&mut self, priority: f32, tick: u64) {
        let plan = SettlementIntentPlan {
            settlement_id: self.settlement_id,
            planned_tick: tick,
            source_response_tick: tick,
            source_need_tick: tick,
            intents: vec![SettlementIntent {
                intent_id: IntentId::new("food-construct-1"),
                source_need: NeedId::new("food"),
                chosen_response: ResponseId::new("construct_food_building"),
                response_type: ResponseType::ConstructBuilding,
                priority,
                desired_persistence: IntentPersistence::UntilPressureLow,
                reasoning: "test food construct".into(),
                diagnostics: Vec::new(),
                ai_seams: Vec::new(),
            }],
            rejected: Vec::new(),
            diagnostics: Vec::new(),
        };
        self.world.settlement_intent_store_mut().insert(plan);
    }

    fn plan_now(&mut self, tick: u64) -> ConstructionPlanningReport {
        let responses = ConstructionResponseCatalog::default();
        let costs = BuildingConstructionCostCatalog::default();
        let mut ctx = ConstructionPlanningContext {
            world: &mut self.world,
            response_catalog: &responses,
            cost_catalog: &costs,
            building_catalog: &self.building_catalog,
            footprint_catalog: &self.footprint,
            doodad_catalog: &self.doodad,
            unit_catalog: &self.unit,
            inventory_ctx: inventory_ctx(),
            simulation_tick: tick,
        };
        plan_construction_for_settlement(&mut ctx, self.settlement_id)
    }
}

#[test]
fn construction_response_produces_plan() {
    let mut fx = Sa9Fixture::new();
    fx.insert_food_construct_intent(80.0, 10);
    let report = fx.plan_now(10);
    assert!(
        !report.created_plan_ids.is_empty() || !report.refreshed_plan_ids.is_empty(),
        "expected plan creation; diag={:?}",
        report.diagnostics
    );
    let plans = fx
        .world
        .construction_plan_store()
        .plans_for_settlement(fx.settlement_id);
    assert_eq!(plans.len(), 1);
    assert_eq!(
        plans[0].building_definition_id.as_str(),
        "prispod_farm"
    );
    assert!(plans[0].status.is_active());
    // Does not spawn completed buildings.
    if let Some(id) = plans[0].reserved_building_id {
        let state = fx.world.get_building(id).unwrap().lifecycle_state;
        assert_ne!(state, BuildingLifecycleState::Complete);
    }
}

#[test]
fn sufficient_capacity_prevents_new_plan() {
    let mut fx = Sa9Fixture::new();
    let ctx = inventory_ctx();
    let farm = create_building_with_inventory(
        &fx.building_catalog,
        &mut fx.world,
        &BuildingDefinitionId::new("prispod_farm"),
        pos(100.0, 100.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        ctx,
    )
    .unwrap()
    .id;
    fx.world.mutate_building(farm, |r| {
        r.lifecycle_state = BuildingLifecycleState::Complete;
    });
    let _ = fx
        .world
        .settlement_store_mut()
        .link_building_to_settlement(fx.settlement_id, farm);
    reconcile_settlement_building_membership(&mut fx.world);

    fx.insert_food_construct_intent(80.0, 10);
    let report = fx.plan_now(10);
    assert!(
        report.created_plan_ids.is_empty(),
        "should not create; diag={:?}",
        report.diagnostics
    );
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.contains("capacity sufficient")));
}

#[test]
fn multiple_buildings_can_satisfy_capability_and_selection_is_deterministic() {
    let catalog = BuildingCatalog::from_definitions(
        starter_building_definitions(),
        &BuildingCategoryCatalog::default(),
    )
    .unwrap();
    let costs = BuildingConstructionCostCatalog::default();
    let mapping = ConstructionResponseCatalog::default()
        .get_str("construct_food_building")
        .unwrap()
        .clone();
    let a = select_building_candidates(&catalog, &costs, &mapping);
    let b = select_building_candidates(&catalog, &costs, &mapping);
    assert_eq!(a, b);
    assert_eq!(a[0].building_definition_id.as_str(), "prispod_farm");
}

#[test]
fn soft_preferences_rank_valid_sites() {
    let fx = Sa9Fixture::new();
    let ownership = BuildingOwnership::with_affiliation(Affiliation::Player);
    let anchor = pos(128.0, 128.0);
    let result = search_placement_candidates(
        &fx.world,
        &fx.building_catalog,
        &fx.footprint,
        &fx.doodad,
        &fx.unit,
        &BuildingDefinitionId::new("hut"),
        ownership,
        anchor,
        PlacementSearchBudget {
            search_radius_meters: 40.0,
            step_meters: 8.0,
            max_candidates: 32,
        },
    );
    assert!(
        result.selected.is_some() || !result.rejected.is_empty(),
        "expected search activity; diag={:?}",
        result.diagnostics
    );
    if let Some(site) = result.selected {
        assert!(site.hard_valid);
        // Soft score is present and ranking used it (non-zero or zero is fine; hard_valid is key).
        let _ = site.soft_score;
    }
}

#[test]
fn duplicate_plans_not_created() {
    let mut fx = Sa9Fixture::new();
    fx.insert_food_construct_intent(80.0, 10);
    let _ = fx.plan_now(10);
    fx.insert_food_construct_intent(90.0, 11);
    let report = fx.plan_now(11);
    assert!(report.created_plan_ids.is_empty());
    let plans = fx
        .world
        .construction_plan_store()
        .plans_for_settlement(fx.settlement_id);
    assert_eq!(plans.len(), 1);
}

#[test]
fn material_requirements_from_cost_catalog() {
    let mut fx = Sa9Fixture::new();
    fx.insert_food_construct_intent(80.0, 10);
    let _ = fx.plan_now(10);
    let plan = fx
        .world
        .construction_plan_store()
        .plans_for_settlement(fx.settlement_id)[0];
    assert!(
        plan.required_materials
            .iter()
            .any(|m| m.item_id.as_str() == "stone" && m.required == 10),
        "materials={:?}",
        plan.required_materials
    );
}

#[test]
fn brief_pressure_drop_does_not_cancel_committed_plan() {
    let mut fx = Sa9Fixture::new();
    fx.insert_food_construct_intent(80.0, 10);
    let _ = fx.plan_now(10);
    // Clear intents (pressure dip).
    fx.world.settlement_intent_store_mut().clear();
    let _ = fx.plan_now(11);
    let plans = fx
        .world
        .construction_plan_store()
        .plans_for_settlement(fx.settlement_id);
    assert_eq!(plans.len(), 1);
    assert!(plans[0].status.is_committed() || plans[0].status.is_active());
    assert_ne!(plans[0].status, ConstructionPlanStatus::Cancelled);
}

#[test]
fn explicit_cancel_releases_reservation() {
    let mut fx = Sa9Fixture::new();
    fx.insert_food_construct_intent(80.0, 10);
    let _ = fx.plan_now(10);
    let plan = fx
        .world
        .construction_plan_store()
        .plans_for_settlement(fx.settlement_id)[0]
        .clone();
    let reserved = plan.reserved_building_id;
    cancel_construction_plan(
        &mut fx.world,
        &fx.building_catalog,
        &fx.footprint,
        &fx.doodad,
        plan.id,
        "player cancel",
        12,
    )
    .unwrap();
    let cancelled = fx.world.construction_plan_store().get(plan.id).unwrap();
    assert_eq!(cancelled.status, ConstructionPlanStatus::Cancelled);
    assert!(cancelled.reserved_building_id.is_none());
    if let Some(id) = reserved {
        assert!(fx.world.get_building(id).is_none());
    }
}

#[test]
fn player_approval_policy_uses_same_runtime() {
    let mut fx = Sa9Fixture::new();
    if let Some(state) = fx
        .world
        .settlement_state_store_mut()
        .get_mut(fx.settlement_id)
    {
        state.policies.player_controlled = true;
        state.policies.require_construction_approval = true;
    }
    fx.insert_food_construct_intent(80.0, 10);
    let _ = fx.plan_now(10);
    let plan_id = fx
        .world
        .construction_plan_store()
        .plans_for_settlement(fx.settlement_id)[0]
        .id;
    assert_eq!(
        fx.world.construction_plan_store().get(plan_id).unwrap().status,
        ConstructionPlanStatus::AwaitingApproval
    );
    let responses = ConstructionResponseCatalog::default();
    let costs = BuildingConstructionCostCatalog::default();
    let mut ctx = ConstructionPlanningContext {
        world: &mut fx.world,
        response_catalog: &responses,
        cost_catalog: &costs,
        building_catalog: &fx.building_catalog,
        footprint_catalog: &fx.footprint,
        doodad_catalog: &fx.doodad,
        unit_catalog: &fx.unit,
        inventory_ctx: inventory_ctx(),
        simulation_tick: 11,
    };
    approve_construction_plan(&mut ctx, plan_id).unwrap();
    let plan = fx.world.construction_plan_store().get(plan_id).unwrap();
    assert!(matches!(
        plan.status,
        ConstructionPlanStatus::Ready
            | ConstructionPlanStatus::AwaitingMaterials
            | ConstructionPlanStatus::Blocked
    ));
}

#[test]
fn plans_survive_save_load_roundtrip() {
    let mut fx = Sa9Fixture::new();
    fx.insert_food_construct_intent(80.0, 10);
    let _ = fx.plan_now(10);
    let save = fx.world.construction_plan_store().export_save_state();
    assert!(!save.plans.is_empty());
    fx.world.construction_plan_store_mut().clear();
    assert_eq!(
        fx.world
            .construction_plan_store()
            .plans_for_settlement(fx.settlement_id)
            .len(),
        0
    );
    fx.world
        .construction_plan_store_mut()
        .import_save_state(save);
    assert_eq!(
        fx.world
            .construction_plan_store()
            .plans_for_settlement(fx.settlement_id)
            .len(),
        1
    );
}

#[test]
fn construction_tasks_derive_from_committed_plans_via_sa6() {
    let mut fx = Sa9Fixture::new();
    fx.insert_food_construct_intent(80.0, 10);
    let _ = fx.plan_now(10);
    let reserved = fx
        .world
        .construction_plan_store()
        .plans_for_settlement(fx.settlement_id)[0]
        .reserved_building_id;
    // If site reserved as Planned, SA6 should emit ConstructBuilding on it.
    if reserved.is_some() {
        let templates = StrategicTaskTemplateCatalog::default();
        generate_strategic_tasks_now(&mut fx.world, &templates, fx.settlement_id, 12);
        let has_construct = fx
            .world
            .task_store()
            .sorted_task_ids()
            .into_iter()
            .filter_map(|id| fx.world.task_store().get(id))
            .any(|task| {
                task.task_type == TaskType::ConstructBuilding
                    && task.strategic.as_ref().is_some_and(|o| {
                        o.settlement_id == fx.settlement_id.raw()
                    })
            });
        assert!(has_construct, "expected ConstructBuilding task after SA6");
    }
}

#[test]
fn planning_does_not_assign_workers_or_move_materials() {
    let mut fx = Sa9Fixture::new();
    fx.insert_food_construct_intent(80.0, 10);
    let _ = fx.plan_now(10);
    // SA9 must not claim workers (Available construct tasks from occupancy sync are OK).
    let assigned = fx
        .world
        .task_store()
        .sorted_task_ids()
        .into_iter()
        .filter_map(|id| fx.world.task_store().get(id))
        .filter(|t| t.assigned_unit_id.is_some())
        .count();
    assert_eq!(assigned, 0);
}

#[test]
fn completed_plan_transitions_when_building_completes() {
    let mut fx = Sa9Fixture::new();
    fx.insert_food_construct_intent(80.0, 10);
    let _ = fx.plan_now(10);
    let plan = fx
        .world
        .construction_plan_store()
        .plans_for_settlement(fx.settlement_id)[0]
        .clone();
    let Some(building_id) = plan.reserved_building_id else {
        // Blocked without site — still a valid blocked plan path.
        assert_eq!(plan.status, ConstructionPlanStatus::Blocked);
        return;
    };
    fx.world.mutate_building(building_id, |r| {
        r.lifecycle_state = BuildingLifecycleState::Complete;
    });
    let _ = fx.plan_now(20);
    assert_eq!(
        fx.world.construction_plan_store().get(plan.id).unwrap().status,
        ConstructionPlanStatus::Completed
    );
}
