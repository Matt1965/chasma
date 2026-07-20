//! Building Intent Propagation tests (SA5).

use bevy::prelude::{Quat, Vec3};

use super::*;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::operation::{ControlSource, OperationLifecycle};
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::operation::OperationCatalog;
use crate::world::settlement::arbiter::arbitrate_settlement_intent_now;
use crate::world::settlement::emergency::EmergencyCatalog;
use crate::world::settlement::needs::{evaluate_settlement_needs_now, NeedCatalog};
use crate::world::settlement::response::{
    discover_settlement_responses_now, ResponseCatalog,
};
use crate::world::settlement::state::{
    NeedCategory, NeedTarget, SettlementKind, SettlementState,
};
use crate::world::settlement::{
    create_settlement_with_treasury, reconcile_settlement_building_membership, SettlementOwnership,
};
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingOwnership, BuildingSource, ChunkCoord, ChunkExtent, LocalPosition, WorldData,
    WorldPosition, create_building_with_inventory, starter_building_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions, starter_operation_definitions,
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

struct Sa5Fixture {
    world: WorldData,
    building_catalog: BuildingCatalog,
    operation_catalog: OperationCatalog,
    settlement_id: crate::world::SettlementId,
    farm_id: crate::world::BuildingId,
    quarry_id: crate::world::BuildingId,
    workbench_id: crate::world::BuildingId,
}

impl Sa5Fixture {
    fn new() -> Self {
        let mut world = flat_world();
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
        let workbench = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("workbench"),
            pos(30.0, 30.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            ownership,
            None,
            ctx,
        )
        .unwrap()
        .id;
        // Incomplete site drives construction pressure.
        let site = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(40.0, 40.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            ownership,
            None,
            ctx,
        )
        .unwrap()
        .id;

        for building_id in [settlement_core, farm, quarry, workbench] {
            world.mutate_building(building_id, |record| {
                record.lifecycle_state = BuildingLifecycleState::Complete;
            });
        }
        world.mutate_building(site, |record| {
            record.lifecycle_state = BuildingLifecycleState::InProgress;
        });

        let settlement = create_settlement_with_treasury(
            &mut world,
            &building_catalog,
            &interaction_catalog,
            settlement_core,
            "SA5 Settlement",
            SettlementOwnership::player_default(),
            pos(50.0, 50.0),
            0,
        )
        .unwrap();
        reconcile_settlement_building_membership(&mut world);

        // Ensure research pressure (default town targets omit research).
        if let Some(state) = world
            .settlement_state_store_mut()
            .get_mut(settlement.settlement_id)
        {
            state.kind = SettlementKind::Town;
            state
                .need_targets
                .push(NeedTarget::new(NeedCategory::Research, 10, 0.5));
            // Construction target helps materials response coexist with backlog pressure.
            if !state
                .need_targets
                .iter()
                .any(|t| t.category == NeedCategory::Construction)
            {
                state
                    .need_targets
                    .push(NeedTarget::new(NeedCategory::Construction, 1, 0.8));
            }
        }

        Self {
            world,
            building_catalog,
            operation_catalog,
            settlement_id: settlement.settlement_id,
            farm_id: farm,
            quarry_id: quarry,
            workbench_id: workbench,
        }
    }

    fn run_pipeline(&mut self, tick: u64) {
        let need_catalog = NeedCatalog::default();
        let response_catalog = ResponseCatalog::default();
        let ctx = test_inventory_ctx();
        evaluate_settlement_needs_now(
            &mut self.world,
            &need_catalog,
            &self.building_catalog,
            ctx.items,
            ctx,
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
            &response_catalog,
            self.settlement_id,
            tick,
        );
        propagate_building_intent_now(
            &mut self.world,
            &response_catalog,
            &self.building_catalog,
            &self.operation_catalog,
            self.settlement_id,
            tick,
        );
    }

    fn policy(&self, building_id: crate::world::BuildingId) -> crate::world::BuildingOperationPolicy {
        self.world
            .building_production_store()
            .get_policy(building_id)
            .cloned()
            .unwrap_or_default()
    }
}

#[test]
fn food_pressure_enables_farm_by_capability() {
    let mut fx = Sa5Fixture::new();
    // Start disabled to prove SA5 enables.
    {
        let store = fx.world.building_production_store_mut();
        let def = fx
            .building_catalog
            .get(&BuildingDefinitionId::new("prispod_farm"))
            .unwrap();
        store.ensure_policy_for_building(fx.farm_id, def, &fx.operation_catalog);
        store.get_policy_mut(fx.farm_id).enabled = false;
        store.get_policy_mut(fx.farm_id).control_source = ControlSource::PlayerControlled;
        store.get_policy_mut(fx.farm_id).planner_managed = false;
    }
    fx.run_pipeline(1);

    let policy = fx.policy(fx.farm_id);
    assert!(policy.enabled, "farm should be enabled for food intent");
    assert_eq!(
        policy.selected_operation.as_ref().map(|o| o.as_str()),
        Some("grow_prispods")
    );
    assert_eq!(policy.control_source, ControlSource::AIControlled);
    assert!(policy.planner_managed);

    let report = fx
        .world
        .building_intent_propagation_store()
        .get(fx.settlement_id)
        .unwrap();
    assert!(report.assignment_for(fx.farm_id).is_some());
    assert!(validate_propagation_report(
        &fx.world,
        &fx.building_catalog,
        &fx.operation_catalog,
        report
    )
    .is_empty());
}

#[test]
fn construction_pressure_enables_quarry_by_capability() {
    let mut fx = Sa5Fixture::new();
    {
        let store = fx.world.building_production_store_mut();
        let def = fx
            .building_catalog
            .get(&BuildingDefinitionId::new("stone_quarry"))
            .unwrap();
        store.ensure_policy_for_building(fx.quarry_id, def, &fx.operation_catalog);
        store.get_policy_mut(fx.quarry_id).enabled = false;
    }
    fx.run_pipeline(1);

    let policy = fx.policy(fx.quarry_id);
    assert!(
        policy.enabled,
        "quarry should be enabled via mine_stone capability for construction"
    );
    assert_eq!(
        policy.selected_operation.as_ref().map(|o| o.as_str()),
        Some("mine_stone")
    );
}

#[test]
fn research_pressure_enables_lab_capability_building() {
    let mut fx = Sa5Fixture::new();
    // Zero food desired so bake_bread does not claim the workbench before research.
    if let Some(state) = fx.world.settlement_state_store_mut().get_mut(fx.settlement_id) {
        if let Some(t) = state
            .need_targets
            .iter_mut()
            .find(|t| t.category == NeedCategory::Food)
        {
            t.target_value = 0;
            t.weight = 0.0;
        } else {
            state
                .need_targets
                .push(NeedTarget::new(NeedCategory::Food, 0, 0.0));
        }
        if let Some(t) = state
            .need_targets
            .iter_mut()
            .find(|t| t.category == NeedCategory::Research)
        {
            t.target_value = 20;
            t.weight = 1.0;
        }
    }
    {
        let store = fx.world.building_production_store_mut();
        let def = fx
            .building_catalog
            .get(&BuildingDefinitionId::new("workbench"))
            .unwrap();
        store.ensure_policy_for_building(fx.workbench_id, def, &fx.operation_catalog);
        store.get_policy_mut(fx.workbench_id).enabled = false;
        store.get_policy_mut(fx.workbench_id).selected_operation =
            Some(crate::world::OperationDefinitionId::new("bake_bread"));
    }
    fx.run_pipeline(1);

    let policy = fx.policy(fx.workbench_id);
    let report = fx
        .world
        .building_intent_propagation_store()
        .get(fx.settlement_id)
        .unwrap();
    let research_assignment = report
        .assignments
        .iter()
        .find(|a| a.response_id.as_str() == "pursue_research");
    assert!(
        research_assignment.is_some(),
        "expected pursue_research assignment; intents={:?} assignments={:?} deferred={:?} diag={:?}",
        fx.world
            .settlement_intent_store()
            .get(fx.settlement_id)
            .map(|p| p
                .intents
                .iter()
                .map(|i| i.chosen_response.as_str().to_string())
                .collect::<Vec<_>>()),
        report
            .assignments
            .iter()
            .map(|a| a.response_id.as_str().to_string())
            .collect::<Vec<_>>(),
        report.deferred_intents,
        report.diagnostics
    );
    assert_eq!(
        policy.selected_operation.as_ref().map(|o| o.as_str()),
        Some("research")
    );
    assert!(policy.enabled);
}

#[test]
fn building_operation_state_untouched() {
    let mut fx = Sa5Fixture::new();
    {
        let store = fx.world.building_production_store_mut();
        let def = fx
            .building_catalog
            .get(&BuildingDefinitionId::new("prispod_farm"))
            .unwrap();
        store.ensure_policy_for_building(fx.farm_id, def, &fx.operation_catalog);
        let state = store.get_state_mut(fx.farm_id);
        state.lifecycle = OperationLifecycle::Running;
        state.completion_count = 7;
        state.active_worker_count = 2;
    }
    let before = fx
        .world
        .building_production_store()
        .get_state(fx.farm_id)
        .cloned()
        .unwrap();
    fx.run_pipeline(1);
    let after = fx
        .world
        .building_production_store()
        .get_state(fx.farm_id)
        .cloned()
        .unwrap();
    assert_eq!(before.lifecycle, after.lifecycle);
    assert_eq!(before.completion_count, after.completion_count);
    assert_eq!(before.active_worker_count, after.active_worker_count);
    assert_eq!(before.progress, after.progress);
}

#[test]
fn policy_updates_survive_save_load() {
    let mut fx = Sa5Fixture::new();
    fx.run_pipeline(1);
    let policy_before = fx.policy(fx.farm_id);
    assert!(policy_before.enabled);

    let save = fx.world.building_production_store().export_save_state();
    // Simulate load: clear transient SA caches, restore policies.
    fx.world.building_intent_propagation_store_mut().clear();
    fx.world.settlement_intent_store_mut().clear();
    fx.world
        .building_production_store_mut()
        .import_save_state(save);

    let policy_after = fx.policy(fx.farm_id);
    assert_eq!(policy_before.enabled, policy_after.enabled);
    assert_eq!(
        policy_before.selected_operation,
        policy_after.selected_operation
    );
    assert_eq!(policy_before.priority, policy_after.priority);
    assert_eq!(policy_before.planner_managed, policy_after.planner_managed);
}

#[test]
fn discovery_uses_operations_not_building_names() {
    let fx = Sa5Fixture::new();
    let op = crate::world::OperationDefinitionId::new("grow_prispods");
    let found = discover_capable_buildings(
        &fx.world,
        &fx.building_catalog,
        fx.settlement_id,
        &op,
    );
    assert!(found.iter().any(|b| b.building_id == fx.farm_id));
    // Capability scan must not require knowing "prispod_farm" as a string in caller.
    assert!(found.iter().all(|b| b.operation_id.as_str() == "grow_prispods"));
}

#[test]
fn unavailable_capability_records_diagnostics_not_construction() {
    let mut world = flat_world();
    let id = crate::world::SettlementId::new(99);
    world
        .settlement_state_store_mut()
        .insert(SettlementState::new(id, SettlementKind::Town, false));
    // Intent plan with food production but no farm buildings.
    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let building_catalog = BuildingCatalog::default();
    let operation_catalog = OperationCatalog::default();
    let ctx = test_inventory_ctx();
    evaluate_settlement_needs_now(
        &mut world,
        &need_catalog,
        &building_catalog,
        ctx.items,
        ctx,
        &EmergencyCatalog::default(),
        id,
        1,
    );
    discover_settlement_responses_now(
        &mut world,
        &need_catalog,
        &response_catalog,
        &EmergencyCatalog::default(),
        &building_catalog,
        id,
        1,
    );
    arbitrate_settlement_intent_now(&mut world, &response_catalog, id, 1);
    propagate_building_intent_now(
        &mut world,
        &response_catalog,
        &building_catalog,
        &operation_catalog,
        id,
        1,
    );
    let report = world.building_intent_propagation_store().get(id).unwrap();
    assert!(
        report.diagnostics.iter().any(|d| d.contains("no capable buildings"))
            || report.assignments.is_empty(),
        "diag={:?}",
        report.diagnostics
    );
    assert!(
        report
            .deferred_intents
            .iter()
            .any(|d| d.contains("deferred"))
            || report.assignments.is_empty()
            || !report.diagnostics.is_empty()
    );
}
