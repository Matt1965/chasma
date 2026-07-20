//! EP6 extraction operation integration tests.

use crate::world::building::field_response::EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT;
use crate::world::building::inventory::attach_inventory_on_building_create;
use crate::world::building::inventory_binding::{
    BuildingInventoryBindingDefinition, BuildingInventoryBindingId, BuildingInventoryRole,
};
use crate::world::building::operation::{
    BuildingOperationParams, OperationLifecycle, PRODUCTION_PROGRESS_ONE_UNIT, ProductionProgress,
    apply_operation_ticks, step_workstation_operation,
};
use crate::world::building::operational_efficiency::OperationalLimitingFactor;
use crate::world::building::terrain_assessment::{
    BuildingTerrainAssessmentStore, TerrainAssessmentCatalogs, assessment_revision_fingerprint,
    ensure_building_terrain_assessment, rebuild_building_terrain_assessment,
};
use crate::world::inventory::{InventoryCatalogCtx, count_stack_item};
use crate::world::operation::OperationCatalog;
use crate::world::{
    BuildingCategoryCatalog, BuildingDefinition, BuildingDefinitionId, BuildingId,
    BuildingLifecycleState, BuildingOwnership, BuildingPlacement, BuildingRecord,
    BuildingRenderKey, BuildingSource, ChunkCoord, ChunkExtent, ChunkId, FootprintCatalog,
    FootprintSpec, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, ItemDefinitionId,
    LocalPosition, OperationDefinitionId, TerrainFieldCatalog, TerrainFieldId, UnitCatalog,
    UnitDefinitionId, UnitId, UnitSource, WorldData, WorldPosition, bootstrap_constant_field,
    create_unit, field_value_from_percent, sample_terrain_field_at, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions, starter_operation_definitions,
};
use bevy::prelude::{Quat, Vec3};

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

fn test_inventory_ctx() -> &'static InventoryCatalogCtx<'static> {
    static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
    CTX.get_or_init(|| {
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items =
            ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let items = Box::leak(Box::new(items));
        let categories = Box::leak(Box::new(categories));
        let profiles = Box::leak(Box::new(profiles));
        InventoryCatalogCtx::new(items, categories, profiles)
    })
}

fn operation_catalog() -> OperationCatalog {
    OperationCatalog::from_definitions(starter_operation_definitions()).unwrap()
}

fn primary_output_binding() -> BuildingInventoryBindingDefinition {
    BuildingInventoryBindingDefinition::new(
        "primary_output",
        BuildingInventoryRole::Output,
        crate::world::InventoryProfileId::new("chest_large"),
    )
    .with_default(true)
}

fn terrain_catalogs(
    building_catalog: &crate::world::BuildingCatalog,
) -> TerrainAssessmentCatalogs<'static> {
    let field_catalog = TerrainFieldCatalog::default();
    let profile_catalog = crate::world::FieldResponseProfileCatalog::default();
    let requirement_catalog = crate::world::BuildingFieldRequirementCatalog::default();
    let footprint_catalog = FootprintCatalog::default();
    TerrainAssessmentCatalogs {
        buildings: Box::leak(Box::new(building_catalog.clone())),
        requirements: Box::leak(Box::new(requirement_catalog)),
        profiles: Box::leak(Box::new(profile_catalog)),
        fields: Box::leak(Box::new(field_catalog)),
        footprints: Box::leak(Box::new(footprint_catalog)),
        requirement_revision: 0,
        profile_revision: 0,
    }
}

fn operation_params<'a>(
    catalogs: &'a TerrainAssessmentCatalogs<'a>,
    assessment_store: &'a mut BuildingTerrainAssessmentStore,
    catalog: &'a OperationCatalog,
) -> BuildingOperationParams<'a> {
    BuildingOperationParams {
        field_catalog: catalogs.fields,
        requirement_catalog: catalogs.requirements,
        profile_catalog: catalogs.profiles,
        footprint_catalog: catalogs.footprints,
        operation_catalog: catalog,
        inventory_ctx: test_inventory_ctx(),
        requirement_revision: catalogs.requirement_revision,
        profile_revision: catalogs.profile_revision,
        assessment_store,
    }
}

fn place_building(
    world: &mut WorldData,
    definition: &BuildingDefinition,
    building_id: BuildingId,
    position: WorldPosition,
) {
    let mut record = BuildingRecord::new(
        building_id,
        definition.id.clone(),
        BuildingPlacement::new(position, Quat::IDENTITY),
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        400,
        BuildingSource::Authored,
    );
    record.lifecycle_state = BuildingLifecycleState::Complete;
    record.construction.progress_0_1 = 1.0;
    attach_inventory_on_building_create(world, test_inventory_ctx(), &mut record, definition)
        .unwrap();
    world
        .insert_building(ChunkId::new(ChunkCoord::new(0, 0)), record)
        .unwrap();
}

fn worker(world: &mut WorldData, position: WorldPosition) -> UnitId {
    let unit_catalog = UnitCatalog::default();
    create_unit(
        &unit_catalog,
        world,
        &UnitDefinitionId::new("wolf"),
        position,
        UnitSource::Authored,
    )
    .unwrap()
    .id
}

fn binding_inventory(world: &WorldData, building_id: BuildingId) -> crate::world::InventoryId {
    world
        .building_inventory_binding_store()
        .resolve_inventory(
            building_id,
            &BuildingInventoryBindingId::new("primary_output"),
        )
        .expect("primary_output binding")
}

fn count_item(world: &WorldData, inventory_id: crate::world::InventoryId, item: &str) -> u32 {
    world
        .inventory_store()
        .get(inventory_id)
        .map(|record| count_stack_item(record, &ItemDefinitionId::new(item)))
        .unwrap_or(0)
}

fn stone_quarry_definition() -> BuildingDefinition {
    BuildingDefinition::new(
        BuildingDefinitionId::new("stone_quarry"),
        "Stone Quarry",
        crate::world::BuildingCategoryId::new("production"),
        BuildingRenderKey::reserved("smelter"),
        BuildingRenderKey::reserved("smelter_collision"),
        450,
        100.0,
        FootprintSpec::Rectangle {
            width_meters: 6.0,
            depth_meters: 6.0,
        },
        30.0,
        true,
    )
    .with_field_sampling_footprint_id(crate::world::FootprintId::new("quarry_excavation"))
    .with_supported_operations([OperationDefinitionId::new("mine_stone")])
    .with_default_operation_id(OperationDefinitionId::new("mine_stone"))
    .with_inventory_bindings(vec![primary_output_binding()])
    .with_default_inventory_binding_id(BuildingInventoryBindingId::new("primary_output"))
}

fn water_well_definition() -> BuildingDefinition {
    BuildingDefinition::new(
        BuildingDefinitionId::new("water_well"),
        "Water Well",
        crate::world::BuildingCategoryId::new("production"),
        BuildingRenderKey::reserved("workbench"),
        BuildingRenderKey::reserved("workbench_collision"),
        120,
        30.0,
        FootprintSpec::Circle { radius_meters: 1.0 },
        35.0,
        true,
    )
    .with_field_sampling_footprint_id(crate::world::FootprintId::new("well_extraction"))
    .with_supported_operations([OperationDefinitionId::new("pump_water")])
    .with_default_operation_id(OperationDefinitionId::new("pump_water"))
    .with_inventory_bindings(vec![primary_output_binding()])
    .with_default_inventory_binding_id(BuildingInventoryBindingId::new("primary_output"))
}

fn setup_extraction_building(
    definition: BuildingDefinition,
    field_id: &str,
    field_percent: f32,
) -> (
    WorldData,
    BuildingId,
    UnitId,
    TerrainAssessmentCatalogs<'static>,
    crate::world::BuildingCatalog,
    OperationCatalog,
) {
    let mut world = flat_world();
    bootstrap_constant_field(
        world.terrain_fields_mut(),
        TerrainFieldId::new(field_id),
        ChunkCoord::new(0, 0),
        field_value_from_percent(field_percent),
    );
    let building_id = world.allocate_building_id();
    let position = pos(64.0, 64.0);
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        crate::world::BuildingCatalog::from_definitions(vec![definition.clone()], &categories)
            .unwrap();
    place_building(&mut world, &definition, building_id, position);
    let worker_id = worker(&mut world, pos(64.0, 63.0));
    let catalogs = terrain_catalogs(&building_catalog);
    let ops = operation_catalog();
    (
        world,
        building_id,
        worker_id,
        catalogs,
        building_catalog,
        ops,
    )
}

fn complete_one_cycle(
    world: &mut WorldData,
    params: &mut BuildingOperationParams<'_>,
    building_catalog: &crate::world::BuildingCatalog,
    building_id: BuildingId,
    worker_id: UnitId,
) {
    let ticks = crate::world::building::operation::expected_ticks_to_complete(
        EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT,
    ) as u32;
    let _ = apply_operation_ticks(
        world,
        params,
        building_catalog,
        building_id,
        worker_id,
        ticks,
    )
    .unwrap();
}

#[test]
fn mine_stone_produces_stone_through_generic_runtime() {
    let (mut world, building_id, worker, catalogs, building_catalog, ops) =
        setup_extraction_building(stone_quarry_definition(), "stone", 80.0);
    let output = binding_inventory(&world, building_id);
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    complete_one_cycle(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    );
    assert_eq!(count_item(&world, output, "stone"), 1);
}

#[test]
fn pump_water_produces_water_through_generic_runtime() {
    let (mut world, building_id, worker, catalogs, building_catalog, ops) =
        setup_extraction_building(water_well_definition(), "water", 80.0);
    let output = binding_inventory(&world, building_id);
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    complete_one_cycle(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    );
    assert_eq!(count_item(&world, output, "water"), 1);
}

#[test]
fn low_terrain_blocks_extraction_without_depleting_field() {
    let (mut world, building_id, worker, catalogs, building_catalog, ops) =
        setup_extraction_building(stone_quarry_definition(), "stone", 0.0);
    let field_catalog = TerrainFieldCatalog::default();
    let sample_pos = pos(64.0, 64.0);
    let before = sample_terrain_field_at(
        &world,
        &field_catalog,
        &TerrainFieldId::new("stone"),
        sample_pos,
    )
    .value;
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    world
        .building_production_store_mut()
        .get_state_mut(building_id)
        .progress = ProductionProgress(PRODUCTION_PROGRESS_ONE_UNIT);
    let report = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    )
    .unwrap();
    assert!(!report.can_operate);
    assert!(matches!(
        report.limiting_factor,
        OperationalLimitingFactor::TerrainAverageBelowMinimum(_)
            | OperationalLimitingFactor::TerrainResponseZero(_)
    ));
    let after = sample_terrain_field_at(
        &world,
        &field_catalog,
        &TerrainFieldId::new("stone"),
        sample_pos,
    )
    .value;
    assert_eq!(before, after);
}

#[test]
fn cached_assessment_revision_is_reused_across_steps() {
    let (mut world, building_id, worker, catalogs, building_catalog, ops) =
        setup_extraction_building(water_well_definition(), "water", 80.0);
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    let _ = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    )
    .unwrap();
    let first_revision = world
        .building_production_store()
        .get_state(building_id)
        .map(|state| state.last_efficiency_revision)
        .unwrap_or(0);
    let _ = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    )
    .unwrap();
    let second_revision = world
        .building_production_store()
        .get_state(building_id)
        .map(|state| state.last_efficiency_revision)
        .unwrap_or(0);
    assert_eq!(first_revision, second_revision);
    assert!(first_revision > 0);
}

#[test]
fn moving_building_marks_assessment_dirty_and_rebuild_changes_revision() {
    let (mut world, building_id, _worker, catalogs, building_catalog, _ops) =
        setup_extraction_building(water_well_definition(), "water", 80.0);
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let record = world.get_building(building_id).unwrap().clone();
    let assessment = ensure_building_terrain_assessment(
        &world,
        &catalogs,
        &mut assessment_store,
        building_id,
        &record,
    );
    let before = assessment_revision_fingerprint(&assessment);
    assessment_store.mark_dirty(building_id);
    let mut record = world.get_building(building_id).unwrap().clone();
    record.placement = BuildingPlacement::new(pos(96.0, 96.0), Quat::IDENTITY);
    world
        .mutate_building(building_id, |stored| {
            stored.placement = record.placement;
        })
        .unwrap();
    rebuild_building_terrain_assessment(&world, &catalogs, &mut assessment_store, building_id);
    let after = assessment_store
        .get(building_id)
        .map(assessment_revision_fingerprint)
        .unwrap_or(0);
    assert_ne!(before, after);
}

#[test]
fn extraction_output_full_blocks_without_consuming_terrain() {
    let (mut world, building_id, worker, catalogs, building_catalog, ops) =
        setup_extraction_building(water_well_definition(), "water", 80.0);
    let output = binding_inventory(&world, building_id);
    let field_catalog = TerrainFieldCatalog::default();
    let sample_pos = pos(64.0, 64.0);
    let before = sample_terrain_field_at(
        &world,
        &field_catalog,
        &TerrainFieldId::new("water"),
        sample_pos,
    )
    .value;
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    for _ in 0..64 {
        let _ = crate::world::place_stack_first_fit(
            inventory_store,
            instance_store,
            test_inventory_ctx(),
            output,
            ItemDefinitionId::new("water"),
            50,
        );
    }
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &ops);
    world
        .building_production_store_mut()
        .get_state_mut(building_id)
        .progress = ProductionProgress(PRODUCTION_PROGRESS_ONE_UNIT);
    let report = step_workstation_operation(
        &mut world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
    )
    .unwrap();
    assert_eq!(report.lifecycle, OperationLifecycle::Blocked);
    assert_eq!(report.limiting_factor, OperationalLimitingFactor::OutputBlocked);
    let after = sample_terrain_field_at(
        &world,
        &field_catalog,
        &TerrainFieldId::new("water"),
        sample_pos,
    )
    .value;
    assert_eq!(before, after);
}

#[test]
fn terrain_assessment_rebuilds_after_session_cache_loss() {
    let (world, building_id, _worker, catalogs, _building_catalog, _ops) =
        setup_extraction_building(water_well_definition(), "water", 80.0);
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let record = world.get_building(building_id).unwrap().clone();
    let before = ensure_building_terrain_assessment(
        &world,
        &catalogs,
        &mut assessment_store,
        building_id,
        &record,
    );
    let before_revision = assessment_revision_fingerprint(&before);
    assessment_store.remove(building_id);
    let after = ensure_building_terrain_assessment(
        &world,
        &catalogs,
        &mut assessment_store,
        building_id,
        &record,
    );
    assert_eq!(before.can_operate, after.can_operate);
    assert_eq!(
        before.terrain_efficiency_basis_points,
        after.terrain_efficiency_basis_points
    );
    assert_eq!(before_revision, assessment_revision_fingerprint(&after));
}

#[test]
fn extraction_operations_declare_terrain_requirements() {
    let catalog = operation_catalog();
    let mine_iron = catalog.get(&OperationDefinitionId::new("mine_iron")).unwrap();
    let mine_stone = catalog.get(&OperationDefinitionId::new("mine_stone")).unwrap();
    let pump_water = catalog.get(&OperationDefinitionId::new("pump_water")).unwrap();
    assert_eq!(mine_iron.terrain_requirements[0].field_id.as_str(), "iron");
    assert_eq!(mine_stone.terrain_requirements[0].field_id.as_str(), "stone");
    assert_eq!(pump_water.terrain_requirements[0].field_id.as_str(), "water");
}
