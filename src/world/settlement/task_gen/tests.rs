//! Strategic Task Generation tests (SA6).

use bevy::prelude::{Quat, Vec3};

use super::*;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::settlement::arbiter::{
    IntentId, IntentPersistence, SettlementIntent, SettlementIntentPlan,
};
use crate::world::settlement::needs::NeedId;
use crate::world::settlement::response::{ResponseId, ResponseType};
use crate::world::settlement::{
    create_settlement_with_treasury, reconcile_settlement_building_membership, SettlementOwnership,
};
use crate::world::task::{TaskPriority, TaskState, TaskType};
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingOwnership, BuildingSource, ChunkCoord, ChunkExtent, LocalPosition, WorldData,
    WorldPosition, create_building_with_inventory, starter_building_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions,
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

struct Sa6Fixture {
    world: WorldData,
    settlement_id: crate::world::SettlementId,
    site_id: crate::world::BuildingId,
}

impl Sa6Fixture {
    fn new() -> Self {
        let mut world = flat_world();
        let categories = BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
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

        world.mutate_building(settlement_core, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        world.mutate_building(site, |record| {
            record.lifecycle_state = BuildingLifecycleState::InProgress;
        });

        let settlement = create_settlement_with_treasury(
            &mut world,
            &building_catalog,
            &interaction_catalog,
            settlement_core,
            "SA6 Settlement",
            SettlementOwnership::player_default(),
            pos(50.0, 50.0),
            0,
        )
        .unwrap();
        reconcile_settlement_building_membership(&mut world);

        Self {
            world,
            settlement_id: settlement.settlement_id,
            site_id: site,
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
}

#[test]
fn food_intent_generates_construction_task() {
    let mut fx = Sa6Fixture::new();
    fx.insert_food_construct_intent(80.0, 10);
    let catalog = StrategicTaskTemplateCatalog::default();
    generate_strategic_tasks_now(&mut fx.world, &catalog, fx.settlement_id, 10);

    let report = fx
        .world
        .strategic_task_generation_store()
        .get(fx.settlement_id)
        .expect("report");
    assert!(
        report.emissions.iter().any(|e| {
            e.response_id == "construct_food_building"
                && e.task_type == TaskType::ConstructBuilding
                && e.building_id == fx.site_id
        }),
        "expected ConstructBuilding on site; got {:?}",
        report.emissions
    );
    let task_id = report.emissions[0].task_id;
    let task = fx.world.task_store().get(task_id).expect("task");
    assert_eq!(task.task_type, TaskType::ConstructBuilding);
    assert!(task.strategic.is_some());
    assert_eq!(
        task.strategic.as_ref().unwrap().response_id,
        "construct_food_building"
    );
    assert!(validate_strategic_task_report(&fx.world, &catalog, report).is_empty());
}

#[test]
fn duplicate_strategic_tasks_merge() {
    let mut fx = Sa6Fixture::new();
    fx.insert_food_construct_intent(80.0, 10);
    let catalog = StrategicTaskTemplateCatalog::default();
    generate_strategic_tasks_now(&mut fx.world, &catalog, fx.settlement_id, 10);
    let first_id = fx
        .world
        .strategic_task_generation_store()
        .get(fx.settlement_id)
        .unwrap()
        .emissions[0]
        .task_id;

    fx.insert_food_construct_intent(90.0, 11);
    generate_strategic_tasks_now(&mut fx.world, &catalog, fx.settlement_id, 11);
    let report = fx
        .world
        .strategic_task_generation_store()
        .get(fx.settlement_id)
        .unwrap();
    assert_eq!(report.emissions.len(), 1);
    assert_eq!(report.emissions[0].task_id, first_id);
    let strategic_construct_count = fx
        .world
        .task_store()
        .sorted_task_ids()
        .into_iter()
        .filter(|&id| {
            fx.world
                .task_store()
                .get(id)
                .map(|t| t.strategic.is_some() && t.state == TaskState::Available)
                .unwrap_or(false)
        })
        .count();
    assert_eq!(strategic_construct_count, 1);
}

#[test]
fn cancelled_intent_removes_available_strategic_tasks() {
    let mut fx = Sa6Fixture::new();
    fx.insert_food_construct_intent(80.0, 10);
    let catalog = StrategicTaskTemplateCatalog::default();
    generate_strategic_tasks_now(&mut fx.world, &catalog, fx.settlement_id, 10);
    let task_id = fx
        .world
        .strategic_task_generation_store()
        .get(fx.settlement_id)
        .unwrap()
        .emissions[0]
        .task_id;
    assert!(fx.world.task_store().get(task_id).is_some());

    // Clear intents → regenerate cancels Available strategic tasks.
    fx.world.settlement_intent_store_mut().insert(SettlementIntentPlan {
        settlement_id: fx.settlement_id,
        planned_tick: 20,
        source_response_tick: 20,
        source_need_tick: 20,
        intents: Vec::new(),
        rejected: Vec::new(),
        diagnostics: Vec::new(),
    });
    generate_strategic_tasks_now(&mut fx.world, &catalog, fx.settlement_id, 20);

    assert!(fx.world.task_store().get(task_id).is_none());
    let report = fx
        .world
        .strategic_task_generation_store()
        .get(fx.settlement_id)
        .unwrap();
    assert!(report.cancelled_task_ids.contains(&task_id));
    assert!(report.emissions.is_empty());
}

#[test]
fn task_priorities_propagate_from_intent() {
    let mut fx = Sa6Fixture::new();
    fx.insert_food_construct_intent(150.0, 10);
    let catalog = StrategicTaskTemplateCatalog::default();
    generate_strategic_tasks_now(&mut fx.world, &catalog, fx.settlement_id, 10);
    let task_id = fx
        .world
        .strategic_task_generation_store()
        .get(fx.settlement_id)
        .unwrap()
        .emissions[0]
        .task_id;
    let task = fx.world.task_store().get(task_id).unwrap();
    assert_eq!(task.priority, TaskPriority::High);
    assert_eq!(intent_to_task_priority(150.0), TaskPriority::High);
    assert_eq!(intent_to_task_priority(50.0), TaskPriority::Normal);
    assert_eq!(intent_to_task_priority(10.0), TaskPriority::Low);
}

#[test]
fn food_construct_without_site_emits_strategic_construct_on_anchor() {
    let mut fx = Sa6Fixture::new();
    // Complete the site so prefer_construction_sites falls through to anchor.
    fx.world.mutate_building(fx.site_id, |record| {
        record.lifecycle_state = BuildingLifecycleState::Complete;
    });
    fx.insert_food_construct_intent(80.0, 10);
    let catalog = StrategicTaskTemplateCatalog::default();
    generate_strategic_tasks_now(&mut fx.world, &catalog, fx.settlement_id, 10);
    let report = fx
        .world
        .strategic_task_generation_store()
        .get(fx.settlement_id)
        .unwrap();
    assert!(
        report
            .emissions
            .iter()
            .any(|e| e.task_type == TaskType::StrategicConstruct),
        "expected StrategicConstruct; got {:?}",
        report.emissions
    );
}
