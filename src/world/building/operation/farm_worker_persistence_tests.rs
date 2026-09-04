//! Farm harvest worker assignment persistence (SA7 + labor integration).

use bevy::prelude::{Quat, Vec3};

use crate::world::building::field_response::EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT;
use crate::world::building::inventory::attach_inventory_on_building_create;
use crate::world::building::operation::{
    BuildingOperationParams, FarmProductionPhase, expected_ticks_to_complete,
    step_all_farm_passive_growth,
};
use crate::world::building::terrain_assessment::TerrainAssessmentCatalogs;
use crate::world::inventory::{InventoryCatalogCtx, count_stack_item};
use crate::world::operation::OperationCatalog;
use crate::world::task::{
    TaskCancelReason, TaskId, TaskState, TaskType, WorkerAssignmentContext, cancel_unit_task,
    step_all_worker_tasks, step_worker_assignment, sync_operate_workstation_tasks,
};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingCategoryCatalog, BuildingDefinition, BuildingId,
    BuildingInteractionProfileCatalog, BuildingLifecycleState, BuildingOwnership,
    BuildingPlacement, BuildingRecord, BuildingSource, ChunkCoord, ChunkData, ChunkExtent, ChunkId,
    ChunkLayout, DoodadCatalog, FootprintCatalog, INTERACTION_WORK_RANGE_METERS,
    InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, ItemDefinitionId, LocalPosition,
    NavigationConfig, OccupancyCatalogs, PassabilityCatalogs, SettlementKind, SettlementOwnership,
    TerrainFieldCatalog, TerrainFieldId, UnitCatalog, UnitDefinitionId, UnitId, UnitOwnership,
    UnitSource, UnitState, WeaponCatalog, WorkPermissionDomain, WorldData, WorldPosition,
    assign_building_settlement, assign_unit_settlement, bootstrap_constant_field,
    create_settlement, create_unit_with_ownership, field_value_from_percent,
    interaction_point_world_position, resolve_all_pending_unit_orders, set_unit_work_permission,
    starter_building_definitions, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions, starter_operation_definitions,
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

fn flat_world() -> WorldData {
    let mut world = WorldData::new(layout());
    let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
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
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let items = Box::leak(Box::new(items));
        let categories = Box::leak(Box::new(categories));
        let profiles = Box::leak(Box::new(profiles));
        InventoryCatalogCtx::new(items, categories, profiles)
    })
}

fn prispod_farm_definition() -> BuildingDefinition {
    starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "prispod_farm")
        .expect("starter prispod_farm")
}

fn operation_catalog() -> OperationCatalog {
    OperationCatalog::from_definitions(starter_operation_definitions()).unwrap()
}

fn building_catalog() -> BuildingCatalog {
    let categories = BuildingCategoryCatalog::default();
    BuildingCatalog::from_definitions(vec![prispod_farm_definition()], &categories).unwrap()
}

fn terrain_catalogs(building_catalog: &BuildingCatalog) -> TerrainAssessmentCatalogs<'static> {
    TerrainAssessmentCatalogs {
        buildings: Box::leak(Box::new(building_catalog.clone())),
        requirements: Box::leak(Box::new(
            crate::world::BuildingFieldRequirementCatalog::default(),
        )),
        profiles: Box::leak(Box::new(
            crate::world::FieldResponseProfileCatalog::default(),
        )),
        fields: Box::leak(Box::new(TerrainFieldCatalog::default())),
        footprints: Box::leak(Box::new(FootprintCatalog::default())),
        requirement_revision: 0,
        profile_revision: 0,
    }
}

fn grow_farm_to_ready(
    world: &mut WorldData,
    assessment_store: &mut crate::world::BuildingTerrainAssessmentStore,
    terrain_catalogs: &TerrainAssessmentCatalogs<'static>,
    operation_catalog: &OperationCatalog,
    building_catalog: &BuildingCatalog,
    building_id: BuildingId,
) {
    let mut params = BuildingOperationParams {
        field_catalog: terrain_catalogs.fields,
        requirement_catalog: terrain_catalogs.requirements,
        profile_catalog: terrain_catalogs.profiles,
        footprint_catalog: terrain_catalogs.footprints,
        operation_catalog,
        inventory_ctx: test_inventory_ctx(),
        requirement_revision: terrain_catalogs.requirement_revision,
        profile_revision: terrain_catalogs.profile_revision,
        assessment_store,
    };
    let ticks = expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT) as u32;
    for _ in 0..ticks {
        step_all_farm_passive_growth(world, building_catalog, &mut params);
    }
    assert_eq!(
        world
            .building_production_store()
            .farm_state(building_id)
            .unwrap()
            .phase,
        FarmProductionPhase::ReadyToHarvest
    );
}

fn step_farm_labor(
    world: &mut WorldData,
    assessment_store: &mut crate::world::BuildingTerrainAssessmentStore,
    terrain_catalogs: &TerrainAssessmentCatalogs<'static>,
    operation_catalog: &OperationCatalog,
    unit_catalog: &UnitCatalog,
    building_catalog: &BuildingCatalog,
    interaction: &BuildingInteractionProfileCatalog,
    doodad: &DoodadCatalog,
    footprint: &FootprintCatalog,
    delta_seconds: f32,
) {
    let occupancy = OccupancyCatalogs {
        building: building_catalog,
        doodad,
        footprint,
    };
    let mut params = BuildingOperationParams {
        field_catalog: terrain_catalogs.fields,
        requirement_catalog: terrain_catalogs.requirements,
        profile_catalog: terrain_catalogs.profiles,
        footprint_catalog: terrain_catalogs.footprints,
        operation_catalog,
        inventory_ctx: test_inventory_ctx(),
        requirement_revision: terrain_catalogs.requirement_revision,
        profile_revision: terrain_catalogs.profile_revision,
        assessment_store,
    };
    let _ = step_all_worker_tasks(
        world,
        unit_catalog,
        building_catalog,
        interaction,
        &crate::world::InteriorProfileCatalog::default(),
        doodad,
        occupancy,
        None,
        delta_seconds,
        Some(&mut params),
    );
}

struct FarmHarness {
    world: WorldData,
    settlement_id: crate::world::SettlementId,
    building_id: BuildingId,
    worker_id: UnitId,
    second_worker_id: UnitId,
    building_catalog: BuildingCatalog,
    operation_catalog: OperationCatalog,
    terrain_catalogs: TerrainAssessmentCatalogs<'static>,
    assessment_store: crate::world::BuildingTerrainAssessmentStore,
    interaction: BuildingInteractionProfileCatalog,
    unit_catalog: UnitCatalog,
    weapons: WeaponCatalog,
    doodad: DoodadCatalog,
    footprint: FootprintCatalog,
    nav: NavigationConfig,
}

impl FarmHarness {
    fn new() -> Self {
        let mut world = flat_world();
        bootstrap_constant_field(
            world.terrain_fields_mut(),
            TerrainFieldId::new("water"),
            ChunkCoord::new(0, 0),
            field_value_from_percent(50.0),
        );
        let settlement_id = create_settlement(
            &mut world,
            pos(64.0, 64.0),
            "Farm Town",
            SettlementOwnership::player_default(),
            SettlementKind::Town,
            None,
            None,
            0,
        )
        .unwrap()
        .settlement_id;

        let definition = prispod_farm_definition();
        let building_id = world.allocate_building_id();
        let mut record = BuildingRecord::new(
            building_id,
            definition.id.clone(),
            BuildingPlacement::new(pos(64.0, 64.0), Quat::IDENTITY),
            BuildingOwnership::with_affiliation(Affiliation::Player),
            definition.max_hp,
            BuildingSource::Authored,
        );
        record.lifecycle_state = BuildingLifecycleState::Complete;
        record.construction.progress_0_1 = 1.0;
        attach_inventory_on_building_create(
            &mut world,
            test_inventory_ctx(),
            &mut record,
            &definition,
        )
        .unwrap();
        world
            .insert_building(ChunkId::new(ChunkCoord::new(0, 0)), record)
            .unwrap();
        assign_building_settlement(&mut world, building_id, Some(settlement_id)).unwrap();

        let unit_catalog = UnitCatalog::default();
        let worker_id = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(60.0, 64.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let second_worker_id = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(58.0, 64.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        assign_unit_settlement(&mut world, worker_id, Some(settlement_id)).unwrap();
        assign_unit_settlement(&mut world, second_worker_id, Some(settlement_id)).unwrap();

        let building_catalog = building_catalog();
        let operation_catalog = operation_catalog();
        let terrain_catalogs = terrain_catalogs(&building_catalog);
        let store = world.building_production_store_mut();
        store.ensure_policy_for_building(building_id, &definition, &operation_catalog);
        store.get_policy_mut(building_id).enabled = true;
        store.farm_state_mut(building_id);

        Self {
            world,
            settlement_id,
            building_id,
            worker_id,
            second_worker_id,
            building_catalog,
            operation_catalog,
            terrain_catalogs,
            assessment_store: crate::world::BuildingTerrainAssessmentStore::default(),
            interaction: BuildingInteractionProfileCatalog::default(),
            unit_catalog,
            weapons: WeaponCatalog::default(),
            doodad: DoodadCatalog::default(),
            footprint: FootprintCatalog::default(),
            nav: NavigationConfig::default(),
        }
    }

    fn grow_to_ready(&mut self) {
        grow_farm_to_ready(
            &mut self.world,
            &mut self.assessment_store,
            &self.terrain_catalogs,
            &self.operation_catalog,
            &self.building_catalog,
            self.building_id,
        );
    }

    fn work_target(&self) -> WorldPosition {
        let building = self.world.get_building(self.building_id).unwrap();
        let definition = self.building_catalog.get(&building.definition_id).unwrap();
        let profile = self.interaction.profile_for_definition(definition).unwrap();
        let point = profile
            .points
            .iter()
            .find(|p| p.task_type == TaskType::OperateWorkstation)
            .expect("harvest point");
        interaction_point_world_position(building, self.world.layout(), point)
    }

    fn place_worker_at_work_point(&mut self, worker_id: UnitId) {
        let target = self.work_target();
        let _ = self.world.update_unit_position(worker_id, target);
        let _ = self.world.set_unit_state(worker_id, UnitState::Idle);
    }

    fn run_assign(&mut self, tick: u64) -> crate::world::WorkerAssignmentReport {
        let inventory_ctx = test_inventory_ctx();
        let mut ctx = WorkerAssignmentContext {
            world: &mut self.world,
            unit_catalog: &self.unit_catalog,
            weapon_catalog: &self.weapons,
            doodad_catalog: &self.doodad,
            building_catalog: &self.building_catalog,
            operation_catalog: &self.operation_catalog,
            interaction_catalog: &self.interaction,
            nav_config: &self.nav,
            inventory_ctx,
            simulation_tick: tick,
        };
        step_worker_assignment(&mut ctx)
    }

    fn run_labor_tick(&mut self, delta_seconds: f32) {
        step_farm_labor(
            &mut self.world,
            &mut self.assessment_store,
            &self.terrain_catalogs,
            &self.operation_catalog,
            &self.unit_catalog,
            &self.building_catalog,
            &self.interaction,
            &self.doodad,
            &self.footprint,
            delta_seconds,
        );
    }

    fn run_movement_tick(&mut self, delta_seconds: f32) {
        let passability = PassabilityCatalogs {
            building: &self.building_catalog,
            doodad: &self.doodad,
            footprint: &self.footprint,
        };
        let _ = crate::world::step_all_unit_movement(
            &mut self.world,
            &self.unit_catalog,
            passability,
            delta_seconds,
        );
    }

    fn resolve_orders(&mut self) {
        let passability = PassabilityCatalogs {
            building: &self.building_catalog,
            doodad: &self.doodad,
            footprint: &self.footprint,
        };
        let _ = resolve_all_pending_unit_orders(
            &mut self.world,
            &self.unit_catalog,
            passability,
            &self.nav,
        );
    }

    fn claim_harvest(&mut self) -> TaskId {
        let report = self.run_assign(1);
        assert!(
            !report.assignments.is_empty(),
            "expected harvest claim; diag={:?}",
            report.diagnostics
        );
        let task_id = self
            .world
            .task_store()
            .unit_task_id(self.worker_id)
            .expect("worker assigned");
        assert_eq!(report.assignments[0].task_id, Some(task_id));
        task_id
    }
}

fn binding_inventory(world: &WorldData, building_id: BuildingId) -> crate::world::InventoryId {
    use crate::world::building::inventory_binding::BuildingInventoryBindingId;
    world
        .building_inventory_binding_store()
        .resolve_inventory(
            building_id,
            &BuildingInventoryBindingId::new("primary_output"),
        )
        .expect("primary_output binding")
}

#[test]
fn ready_farm_maintains_harvest_work_listing() {
    let mut harness = FarmHarness::new();
    harness.grow_to_ready();
    sync_operate_workstation_tasks(&mut harness.world, &harness.building_catalog, 1);
    let has_available = harness
        .world
        .task_store()
        .building_task_ids(harness.building_id)
        .iter()
        .any(|task_id| {
            harness
                .world
                .task_store()
                .get(*task_id)
                .is_some_and(|task| {
                    task.task_type == TaskType::OperateWorkstation
                        && task.state == TaskState::Available
                })
        });
    assert!(has_available);
}

#[test]
fn eligible_settlement_worker_claims_harvest() {
    let mut harness = FarmHarness::new();
    harness.grow_to_ready();
    let task_id = harness.claim_harvest();
    let task = harness.world.task_store().get(task_id).unwrap();
    assert_eq!(task.task_type, TaskType::OperateWorkstation);
    assert!(matches!(
        task.state,
        TaskState::Assigned | TaskState::InProgress
    ));
}

#[test]
fn claimed_harvest_persists_across_sa7_reevaluation() {
    let mut harness = FarmHarness::new();
    harness.grow_to_ready();
    let task_id = harness.claim_harvest();
    for tick in 2..=20 {
        harness.run_assign(tick);
        assert_eq!(
            harness.world.task_store().unit_task_id(harness.worker_id),
            Some(task_id),
            "tick {tick}"
        );
    }
}

#[test]
fn traveling_worker_retains_harvest_claim() {
    let mut harness = FarmHarness::new();
    harness.grow_to_ready();
    let task_id = harness.claim_harvest();
    harness.resolve_orders();
    for tick in 2..=10 {
        harness.run_assign(tick);
        harness.run_labor_tick(1.0);
        harness.run_movement_tick(1.0);
        assert_eq!(
            harness.world.task_store().unit_task_id(harness.worker_id),
            Some(task_id),
            "tick {tick}"
        );
    }
}

#[test]
fn arrival_transitions_into_harvest_execution() {
    let mut harness = FarmHarness::new();
    harness.grow_to_ready();
    let _task_id = harness.claim_harvest();
    harness.place_worker_at_work_point(harness.worker_id);
    harness.run_labor_tick(1.0);
    let phase = harness
        .world
        .building_production_store()
        .farm_state(harness.building_id)
        .unwrap()
        .phase;
    assert_eq!(phase, FarmProductionPhase::Harvesting);
    assert!(matches!(
        harness.world.get_unit(harness.worker_id).unwrap().state,
        UnitState::Working { .. }
    ));
    assert_eq!(
        harness.world.task_store().unit_task_id(harness.worker_id),
        Some(_task_id)
    );
}

#[test]
fn harvest_progresses_while_worker_remains_assigned() {
    let mut harness = FarmHarness::new();
    harness.grow_to_ready();
    let task_id = harness.claim_harvest();
    harness.place_worker_at_work_point(harness.worker_id);
    let before = harness
        .world
        .building_production_store()
        .farm_state(harness.building_id)
        .unwrap()
        .harvest_progress
        .value();
    harness.run_labor_tick(1.0);
    let after = harness
        .world
        .building_production_store()
        .farm_state(harness.building_id)
        .unwrap()
        .harvest_progress
        .value();
    assert!(after > before);
    assert_eq!(
        harness.world.task_store().unit_task_id(harness.worker_id),
        Some(task_id)
    );
}

#[test]
fn successful_harvest_commits_output_releases_worker_and_resets_farm() {
    let mut harness = FarmHarness::new();
    harness.grow_to_ready();
    let task_id = harness.claim_harvest();
    harness.place_worker_at_work_point(harness.worker_id);
    let ticks = expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT) as u32;
    for _ in 0..ticks {
        harness.run_labor_tick(1.0);
    }
    let inventory_id = binding_inventory(&harness.world, harness.building_id);
    assert_eq!(
        count_stack_item(
            harness.world.inventory_store().get(inventory_id).unwrap(),
            &ItemDefinitionId::new("prispod"),
        ),
        1
    );
    let farm = harness
        .world
        .building_production_store()
        .farm_state(harness.building_id)
        .unwrap();
    assert_eq!(farm.phase, FarmProductionPhase::Growing);
    assert!(
        harness
            .world
            .task_store()
            .unit_task_id(harness.worker_id)
            .is_none()
    );
    assert_eq!(
        harness.world.task_store().get(task_id).unwrap().state,
        TaskState::Completed
    );
}

#[test]
fn harvest_worker_does_not_bounce_claim_during_execution() {
    let mut harness = FarmHarness::new();
    harness.grow_to_ready();
    let task_id = harness.claim_harvest();
    harness.place_worker_at_work_point(harness.worker_id);
    let mut claim_events = 0u32;
    for tick in 2..=30 {
        let report = harness.run_assign(tick);
        claim_events += report.assignments.len() as u32;
        harness.run_labor_tick(1.0);
        assert_eq!(
            harness.world.task_store().unit_task_id(harness.worker_id),
            Some(task_id),
            "tick {tick}"
        );
    }
    assert_eq!(
        claim_events, 0,
        "worker should not be re-claimed mid-harvest"
    );
}

#[test]
fn second_worker_does_not_steal_claimed_harvest() {
    let mut harness = FarmHarness::new();
    harness.grow_to_ready();
    let task_id = harness.claim_harvest();
    harness.place_worker_at_work_point(harness.worker_id);
    harness.run_assign(2);
    assert_eq!(
        harness
            .world
            .task_store()
            .unit_task_id(harness.second_worker_id),
        None
    );
    assert_eq!(
        harness
            .world
            .task_store()
            .get(task_id)
            .unwrap()
            .assigned_unit_id,
        Some(harness.worker_id)
    );
}

#[test]
fn farming_permission_denied_prevents_initial_harvest_claim() {
    let mut harness = FarmHarness::new();
    harness.grow_to_ready();
    set_unit_work_permission(
        &mut harness.world,
        harness.settlement_id,
        harness.worker_id,
        WorkPermissionDomain::Farming,
        false,
    )
    .unwrap();
    set_unit_work_permission(
        &mut harness.world,
        harness.settlement_id,
        harness.second_worker_id,
        WorkPermissionDomain::Farming,
        false,
    )
    .unwrap();
    let report = harness.run_assign(1);
    assert!(report.assignments.is_empty());
    assert!(
        harness
            .world
            .task_store()
            .unit_task_id(harness.worker_id)
            .is_none()
    );
    assert!(
        harness
            .world
            .task_store()
            .unit_task_id(harness.second_worker_id)
            .is_none()
    );
}

#[test]
fn farm_removal_cancels_worker_assignment() {
    let mut harness = FarmHarness::new();
    harness.grow_to_ready();
    let _task_id = harness.claim_harvest();
    harness.world.remove_building_by_id(harness.building_id);
    let mut events = Vec::new();
    cancel_unit_task(
        &mut harness.world,
        harness.worker_id,
        TaskCancelReason::BuildingDestroyed,
        &mut events,
    );
    assert!(
        harness
            .world
            .task_store()
            .unit_task_id(harness.worker_id)
            .is_none()
    );
}

#[test]
fn idle_worker_in_work_range_does_not_get_repathed_by_resume_stalled() {
    let mut harness = FarmHarness::new();
    harness.grow_to_ready();
    let _task_id = harness.claim_harvest();
    harness.place_worker_at_work_point(harness.worker_id);
    let target = harness.work_target();
    let unit = harness.world.get_unit(harness.worker_id).unwrap();
    let layout = harness.world.layout();
    let unit_global = unit.placement.position.to_global(layout);
    let work_global = target.to_global(layout);
    let dx = unit_global.x - work_global.x;
    let dz = unit_global.z - work_global.z;
    let distance = (dx * dx + dz * dz).sqrt();
    assert!(distance <= INTERACTION_WORK_RANGE_METERS);
    harness.run_assign(2);
    harness.resolve_orders();
    harness.run_labor_tick(1.0);
    assert!(matches!(
        harness.world.get_unit(harness.worker_id).unwrap().state,
        UnitState::Working { .. }
    ));
}

mod schedule_level {
    use super::*;
    use crate::simulation::{SIMULATION_TICK_SECONDS, run_simulation_tick};
    use crate::world::building::operation::{
        BASE_OPERATION_PROGRESS_PER_TICK, PRODUCTION_PROGRESS_ONE_UNIT, ProductionProgress,
    };
    use crate::world::{
        AuthoredRelationshipCatalog, BuildingConstructionSettings,
        BuildingNavigationBlueprintCatalog, CombatAiScanState, CombatAiSettings, CorpseSettings,
        InteriorProfileCatalog, WeaponCatalog, ensure_settlement_states_for_world,
        seed_building_settlement_at_creation, starter_weapon_definitions,
    };

    struct ScheduleHarness {
        world: WorldData,
        settlement_id: crate::world::SettlementId,
        building_id: BuildingId,
        worker_id: UnitId,
        building_catalog: BuildingCatalog,
        operation_catalog: OperationCatalog,
        terrain_catalogs: TerrainAssessmentCatalogs<'static>,
        assessment_store: crate::world::BuildingTerrainAssessmentStore,
        unit_catalog: UnitCatalog,
        weapons: WeaponCatalog,
        doodad: DoodadCatalog,
        footprint: FootprintCatalog,
        nav: NavigationConfig,
        interaction: BuildingInteractionProfileCatalog,
        interior: InteriorProfileCatalog,
        nav_blueprint: BuildingNavigationBlueprintCatalog,
        combat_scan: CombatAiScanState,
    }

    impl ScheduleHarness {
        fn new(automation_enabled: bool) -> Self {
            Self::with_farm_growth_seed(automation_enabled, true)
        }

        fn with_farm_growth_seed(automation_enabled: bool, one_tick_before_ready: bool) -> Self {
            let mut world = flat_world();
            bootstrap_constant_field(
                world.terrain_fields_mut(),
                TerrainFieldId::new("water"),
                ChunkCoord::new(0, 0),
                field_value_from_percent(50.0),
            );
            let settlement_id = create_settlement(
                &mut world,
                pos(64.0, 64.0),
                "Schedule Farm Town",
                SettlementOwnership::player_default(),
                SettlementKind::Town,
                None,
                None,
                0,
            )
            .unwrap()
            .settlement_id;
            ensure_settlement_states_for_world(&mut world);
            if let Some(state) = world.settlement_state_store_mut().get_mut(settlement_id) {
                state.policies.automation_enabled = automation_enabled;
            }

            let definition = prispod_farm_definition();
            let building_id = world.allocate_building_id();
            let farm_position = pos(64.0, 64.0);
            let mut record = BuildingRecord::new(
                building_id,
                definition.id.clone(),
                BuildingPlacement::new(farm_position, Quat::IDENTITY),
                BuildingOwnership::with_affiliation(Affiliation::Player),
                definition.max_hp,
                BuildingSource::Authored,
            );
            record.lifecycle_state = BuildingLifecycleState::Complete;
            record.construction.progress_0_1 = 1.0;
            attach_inventory_on_building_create(
                &mut world,
                test_inventory_ctx(),
                &mut record,
                &definition,
            )
            .unwrap();
            world
                .insert_building(ChunkId::new(ChunkCoord::new(0, 0)), record)
                .unwrap();
            seed_building_settlement_at_creation(&mut world, building_id, farm_position);

            let interaction = BuildingInteractionProfileCatalog::default();
            let profile = interaction.profile_for_definition(&definition).unwrap();
            let point = profile
                .points
                .iter()
                .find(|p| p.task_type == TaskType::OperateWorkstation)
                .expect("harvest point");
            let building = world.get_building(building_id).unwrap();
            let work_target = interaction_point_world_position(building, world.layout(), point);
            let worker_spawn = WorldPosition::new(
                work_target.chunk,
                LocalPosition::new(Vec3::new(
                    work_target.local.0.x - 1.2,
                    work_target.local.0.y,
                    work_target.local.0.z,
                )),
            );

            let unit_catalog = UnitCatalog::default();
            let worker_id = create_unit_with_ownership(
                &unit_catalog,
                &mut world,
                &UnitDefinitionId::new("bandit"),
                worker_spawn,
                UnitSource::Authored,
                UnitOwnership::player_default(),
            )
            .unwrap()
            .id;

            let building_catalog = building_catalog();
            let operation_catalog = operation_catalog();
            let terrain_catalogs = terrain_catalogs(&building_catalog);
            {
                let store = world.building_production_store_mut();
                store.ensure_policy_for_building(building_id, &definition, &operation_catalog);
                store.get_policy_mut(building_id).enabled = true;
                let farm = store.farm_state_mut(building_id);
                farm.phase = FarmProductionPhase::Growing;
                farm.growth_progress = if one_tick_before_ready {
                    ProductionProgress(
                        PRODUCTION_PROGRESS_ONE_UNIT - BASE_OPERATION_PROGRESS_PER_TICK,
                    )
                } else {
                    ProductionProgress::ZERO
                };
                farm.harvest_progress = ProductionProgress::ZERO;
            }

            Self {
                world,
                settlement_id,
                building_id,
                worker_id,
                building_catalog,
                operation_catalog,
                terrain_catalogs,
                assessment_store: crate::world::BuildingTerrainAssessmentStore::default(),
                unit_catalog,
                weapons: WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap(),
                doodad: DoodadCatalog::default(),
                footprint: FootprintCatalog::default(),
                nav: NavigationConfig::default(),
                interaction,
                interior: InteriorProfileCatalog::default(),
                nav_blueprint: BuildingNavigationBlueprintCatalog::default(),
                combat_scan: CombatAiScanState::default(),
            }
        }

        fn run_tick(&mut self, tick: u64) {
            let inventory_ctx = test_inventory_ctx();
            let mut operation = BuildingOperationParams {
                field_catalog: self.terrain_catalogs.fields,
                requirement_catalog: self.terrain_catalogs.requirements,
                profile_catalog: self.terrain_catalogs.profiles,
                footprint_catalog: self.terrain_catalogs.footprints,
                operation_catalog: &self.operation_catalog,
                inventory_ctx,
                requirement_revision: self.terrain_catalogs.requirement_revision,
                profile_revision: self.terrain_catalogs.profile_revision,
                assessment_store: &mut self.assessment_store,
            };
            let _ = run_simulation_tick(
                &mut self.world,
                &self.unit_catalog,
                &self.weapons,
                &self.doodad,
                &self.building_catalog,
                &self.footprint,
                &self.interaction,
                &self.nav,
                crate::world::AttackTargetingPolicy::default(),
                &AuthoredRelationshipCatalog::default(),
                &CombatAiSettings::default(),
                &mut self.combat_scan,
                BuildingConstructionSettings::default(),
                &self.interior,
                Some(&self.nav_blueprint),
                inventory_ctx.items,
                inventory_ctx.categories,
                inventory_ctx.profiles,
                &CorpseSettings::default(),
                SIMULATION_TICK_SECONDS,
                tick,
                Some(&mut operation),
            );
        }

        fn output_prispod_count(&self) -> u32 {
            let inventory_id = binding_inventory(&self.world, self.building_id);
            count_stack_item(
                self.world.inventory_store().get(inventory_id).unwrap(),
                &ItemDefinitionId::new("prispod"),
            )
        }

        fn clear_farm_output_for_next_cycle(&mut self) {
            let inventory_id = binding_inventory(&self.world, self.building_id);
            let Some(record) = self.world.inventory_store().get(inventory_id) else {
                return;
            };
            if record.placed_entries().is_empty() {
                return;
            }
            let inventory_ctx = test_inventory_ctx();
            let (inventory_store, instance_store) = self.world.inventory_runtime_mut();
            let _ = crate::world::inventory::remove_entry(
                inventory_store,
                instance_store,
                inventory_ctx,
                inventory_id,
                0,
            );
        }
    }

    #[test]
    fn scheduled_runtime_natural_ready_attracts_and_completes_harvest() {
        let mut harness = ScheduleHarness::new(false);
        assert_eq!(
            harness
                .world
                .get_unit(harness.worker_id)
                .unwrap()
                .settlement_id,
            Some(harness.settlement_id),
            "unit created inside settlement must inherit membership"
        );

        let harvest_ticks =
            expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT) as u32;
        let max_ticks = 100 + harvest_ticks + 250;

        let mut ready_seen = false;
        let mut assigned = false;
        let mut harvest_progress_increases = 0u32;
        let mut last_harvest = 0u64;

        for tick in 1..=max_ticks {
            harness.run_tick(tick as u64);

            let phase = harness
                .world
                .building_production_store()
                .farm_state(harness.building_id)
                .map(|farm| farm.phase);
            if phase == Some(FarmProductionPhase::ReadyToHarvest)
                || phase == Some(FarmProductionPhase::Harvesting)
            {
                ready_seen = true;
            }
            if harness
                .world
                .task_store()
                .unit_task_id(harness.worker_id)
                .is_some()
            {
                assigned = true;
            }
            if let Some(farm) = harness
                .world
                .building_production_store()
                .farm_state(harness.building_id)
            {
                let harvest = farm.harvest_progress.value();
                if harvest > last_harvest {
                    harvest_progress_increases += 1;
                    last_harvest = harvest;
                }
            }
            if harness.output_prispod_count() >= 1 {
                break;
            }
        }

        assert!(
            ready_seen,
            "farm must reach harvest-ready via scheduled passive growth"
        );
        assert!(
            assigned,
            "idle settlement worker must autonomously claim harvest work"
        );
        assert!(
            harvest_progress_increases >= 5,
            "harvest must advance across many ticks, got {harvest_progress_increases} increases"
        );
        assert_eq!(harness.output_prispod_count(), 1);
        assert!(
            harness
                .world
                .task_store()
                .unit_task_id(harness.worker_id)
                .is_none(),
            "worker must release after harvest completes"
        );
    }

    #[test]
    fn scheduled_harvest_executes_with_planning_automation_disabled() {
        let mut harness = ScheduleHarness::new(false);
        assert!(
            !harness
                .world
                .settlement_state_store()
                .get(harness.settlement_id)
                .unwrap()
                .policies
                .automation_enabled
        );

        let harvest_ticks =
            expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT) as u32;
        let max_ticks = 100 + harvest_ticks + 250;
        let mut assigned = false;

        for tick in 1..=max_ticks {
            harness.run_tick(tick as u64);
            if harness
                .world
                .task_store()
                .unit_task_id(harness.worker_id)
                .is_some()
            {
                assigned = true;
            }
            if harness.output_prispod_count() >= 1 {
                break;
            }
        }

        assert!(
            assigned,
            "SA7 execution must not require planning automation"
        );
        assert_eq!(harness.output_prispod_count(), 1);
    }

    #[test]
    fn scheduled_harvest_claims_after_full_natural_growth_when_worker_is_critically_hungry() {
        use crate::world::{HungerStage, NutritionProfile, evaluate_hunger_stage};

        let mut harness = ScheduleHarness::with_farm_growth_seed(false, false);
        let profile = NutritionProfile::from_definition(
            harness
                .unit_catalog
                .get(&UnitDefinitionId::new("bandit"))
                .unwrap(),
        )
        .unwrap();
        let starting_nutrition = harness
            .world
            .get_unit(harness.worker_id)
            .unwrap()
            .nutrition
            .current;
        assert_eq!(
            starting_nutrition, profile.max,
            "runtime unit creation must start at authored fullness"
        );

        let harvest_ticks =
            expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT) as u32;
        let max_ticks = 500 + harvest_ticks + 250;
        let mut saw_critical_before_claim = false;
        let mut assigned = false;

        for tick in 1..=max_ticks {
            harness.run_tick(tick as u64);
            if !assigned {
                let unit = harness.world.get_unit(harness.worker_id).unwrap();
                if evaluate_hunger_stage(unit.nutrition.current, &profile) == HungerStage::Critical
                {
                    saw_critical_before_claim = true;
                }
                if harness
                    .world
                    .task_store()
                    .unit_task_id(harness.worker_id)
                    .is_some()
                {
                    assigned = true;
                }
            }
            if harness.output_prispod_count() >= 1 {
                break;
            }
        }

        assert!(
            saw_critical_before_claim,
            "passive farm growth should outlast starting nutrition and reach critical hunger \
             before harvest work is claimed"
        );
        assert!(
            assigned,
            "critically hungry worker with no accessible food must still claim harvest work"
        );
        assert_eq!(harness.output_prispod_count(), 1);
    }

    #[test]
    fn scheduled_runtime_completes_two_natural_harvest_cycles() {
        let mut harness = ScheduleHarness::new(false);
        let harvest_ticks =
            expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT) as u32;
        let max_ticks = (100 + harvest_ticks) * 3 + 600;
        let mut harvest_completions = 0u32;
        let mut cleared_after_first = false;

        for tick in 1..=max_ticks {
            harness.run_tick(tick as u64);
            let output = harness.output_prispod_count();
            if harvest_completions == 0 && output >= 1 {
                harvest_completions = 1;
                harness.clear_farm_output_for_next_cycle();
                cleared_after_first = true;
            } else if cleared_after_first && output >= 1 {
                harvest_completions = 2;
                break;
            }
        }

        assert_eq!(
            harvest_completions, 2,
            "two full natural grow→harvest cycles should complete without Force Cycle"
        );
    }
}
