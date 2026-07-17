//! TF5 integration tests — deterministic efficiency and workstation stepping.

use crate::world::building::field_response::EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT;
use crate::world::building::operation::{
    BuildingOperationParams, BuildingOperationStore, apply_operation_ticks,
    expected_ticks_to_complete, step_workstation_operation,
};
use crate::world::building::operational_efficiency::building_operational_efficiency;
use crate::world::building::terrain_assessment::{
    BuildingTerrainAssessmentStore, TerrainAssessmentCatalogs,
};
use crate::world::{
    BuildingCategoryCatalog, BuildingDefinition, BuildingDefinitionId, BuildingId,
    BuildingLifecycleState, BuildingOwnership, BuildingPlacement, BuildingRecord,
    BuildingRenderKey, BuildingSource, ChunkCoord, ChunkExtent, ChunkId, FootprintCatalog,
    FootprintSpec, LocalPosition, UnitCatalog, UnitDefinitionId, UnitId, UnitSource, WorldData,
    WorldPosition, create_unit,
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

fn iron_mine_record(building_id: BuildingId) -> BuildingRecord {
    let mut record = BuildingRecord::new(
        building_id,
        BuildingDefinitionId::new("iron_mine"),
        BuildingPlacement::new(pos(64.0, 64.0), Quat::IDENTITY),
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        400,
        BuildingSource::Authored,
    );
    record.lifecycle_state = BuildingLifecycleState::Complete;
    record.construction.progress_0_1 = 1.0;
    record
}

fn iron_mine_catalogs() -> (
    TerrainAssessmentCatalogs<'static>,
    crate::world::BuildingCatalog,
) {
    let field_catalog = crate::world::TerrainFieldCatalog::default();
    let profile_catalog = crate::world::FieldResponseProfileCatalog::default();
    let requirement_catalog = crate::world::BuildingFieldRequirementCatalog::default();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog = crate::world::BuildingCatalog::from_definitions(
        vec![BuildingDefinition::new(
            BuildingDefinitionId::new("iron_mine"),
            "Iron Mine",
            crate::world::BuildingCategoryId::new("production"),
            BuildingRenderKey::reserved("smelter"),
            BuildingRenderKey::reserved("smelter_collision"),
            400,
            90.0,
            FootprintSpec::Circle { radius_meters: 2.5 },
            30.0,
            true,
        )],
        &categories,
    )
    .unwrap();
    let footprint_catalog = FootprintCatalog::default();
    let catalogs = TerrainAssessmentCatalogs {
        buildings: Box::leak(Box::new(building_catalog.clone())),
        requirements: Box::leak(Box::new(requirement_catalog)),
        profiles: Box::leak(Box::new(profile_catalog)),
        fields: Box::leak(Box::new(field_catalog)),
        footprints: Box::leak(Box::new(footprint_catalog)),
        requirement_revision: 0,
        profile_revision: 0,
    };
    (catalogs, building_catalog)
}

fn setup_iron_mine_world(
    iron_percent: f32,
) -> (
    WorldData,
    BuildingId,
    UnitId,
    TerrainAssessmentCatalogs<'static>,
    crate::world::BuildingCatalog,
) {
    let mut world = flat_world();
    crate::world::bootstrap_constant_field(
        world.terrain_fields_mut(),
        crate::world::TerrainFieldId::new("iron"),
        ChunkCoord::new(0, 0),
        crate::world::field_value_from_percent(iron_percent),
    );
    let building_id = world.allocate_building_id();
    world
        .insert_building(
            ChunkId::new(ChunkCoord::new(0, 0)),
            iron_mine_record(building_id),
        )
        .unwrap();
    let unit_catalog = UnitCatalog::default();
    let worker = create_unit(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        pos(64.0, 63.0),
        UnitSource::Authored,
    )
    .unwrap()
    .id;
    let (catalogs, building_catalog) = iron_mine_catalogs();
    (world, building_id, worker, catalogs, building_catalog)
}

fn operation_params<'a>(
    catalogs: &'a TerrainAssessmentCatalogs<'a>,
    assessment_store: &'a mut BuildingTerrainAssessmentStore,
    operation_store: &'a mut BuildingOperationStore,
) -> BuildingOperationParams<'a> {
    BuildingOperationParams {
        field_catalog: catalogs.fields,
        requirement_catalog: catalogs.requirements,
        profile_catalog: catalogs.profiles,
        footprint_catalog: catalogs.footprints,
        requirement_revision: catalogs.requirement_revision,
        profile_revision: catalogs.profile_revision,
        assessment_store,
        operation_store,
    }
}

#[test]
fn progress_parity_at_half_full_and_rich_efficiency() {
    let full = expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT);
    let half = expected_ticks_to_complete(EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT / 2);
    let rich = expected_ticks_to_complete(15_000);
    assert_eq!(half, full * 2);
    assert_eq!(rich * 3 / 2, full);
}

#[test]
fn workstation_operation_reaches_completion_at_rated_efficiency() {
    let (world, building_id, worker, catalogs, building_catalog) = setup_iron_mine_world(94.0);
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut operation_store = BuildingOperationStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &mut operation_store);

    let mut ctx = params.efficiency_context(&world, &building_catalog);
    let efficiency = building_operational_efficiency(&mut ctx, building_id).unwrap();
    let ticks =
        expected_ticks_to_complete(efficiency.final_output_efficiency_basis_points.value()) as u32;
    let report = apply_operation_ticks(
        &world,
        &mut params,
        &building_catalog,
        building_id,
        worker,
        ticks,
    )
    .unwrap();
    assert!(!report.blocked);
    assert_eq!(report.completed_units, 1);
}

#[test]
fn blocked_workstation_emits_zero_progress_until_terrain_recovers() {
    let (world, building_id, worker, catalogs, building_catalog) = setup_iron_mine_world(0.0);
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut operation_store = BuildingOperationStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &mut operation_store);

    let blocked =
        step_workstation_operation(&world, &mut params, &building_catalog, building_id, worker)
            .unwrap();
    assert!(!blocked.can_operate);
    assert_eq!(blocked.scaled_progress, 0);
    assert_eq!(
        operation_store
            .get(building_id)
            .map(|s| s.progress.value())
            .unwrap_or(0),
        0
    );
}

#[test]
fn preview_efficiency_matches_runtime_query() {
    let (world, building_id, _worker, catalogs, building_catalog) = setup_iron_mine_world(94.0);
    let record = world.get_building(building_id).unwrap().clone();
    let preview = crate::world::assess_building_terrain(&world, &catalogs, &record, world.layout());
    let mut assessment_store = BuildingTerrainAssessmentStore::default();
    let mut operation_store = BuildingOperationStore::default();
    let mut params = operation_params(&catalogs, &mut assessment_store, &mut operation_store);
    let mut ctx = params.efficiency_context(&world, &building_catalog);
    let runtime = building_operational_efficiency(&mut ctx, building_id).unwrap();
    assert_eq!(
        runtime.terrain_efficiency_basis_points,
        preview.terrain_efficiency_basis_points
    );
    assert_eq!(runtime.can_operate, preview.can_operate);
}
