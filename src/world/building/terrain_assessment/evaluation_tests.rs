//! Regression tests for shared field-requirement evaluation.

use crate::world::building::field_requirement::BuildingFieldRequirementCatalog;
use crate::world::building::field_response::{
    FieldResponseProfileCatalog, field_value_from_percent, field_value_to_percent_display,
};
use crate::world::building::terrain_assessment::{
    FieldRequirementFailureReason, TerrainAssessmentCatalogs, assess_building_terrain,
    evaluate_field_requirement, evaluate_field_requirement_assessment,
    primary_failure_for_assessment,
};
use crate::world::{
    BuildingCategoryCatalog, BuildingDefinition, BuildingDefinitionId, BuildingLifecycleState,
    BuildingOwnership, BuildingPlacement, BuildingRecord, BuildingRenderKey, BuildingSource,
    ChunkCoord, ChunkExtent, FootprintCatalog, FootprintSpec, TerrainFieldCatalog, TerrainFieldId,
    WorldConfig, WorldData, WorldPosition, bootstrap_constant_field,
};
use bevy::prelude::{Quat, Vec3};

fn stone_quarry_definition() -> BuildingDefinition {
    BuildingDefinition::new(
        BuildingDefinitionId::new("stone_quarry"),
        "Stone Quarry",
        crate::world::BuildingCategoryId::new("production"),
        BuildingRenderKey::reserved("stone_mine"),
        BuildingRenderKey::reserved("stone_mine"),
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
}

fn farm_definition() -> BuildingDefinition {
    crate::world::starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "prispod_farm")
        .expect("farm")
}

fn catalogs(
    building_catalog: &crate::world::BuildingCatalog,
) -> TerrainAssessmentCatalogs<'static> {
    TerrainAssessmentCatalogs {
        buildings: Box::leak(Box::new(building_catalog.clone())),
        requirements: Box::leak(Box::new(BuildingFieldRequirementCatalog::default())),
        profiles: Box::leak(Box::new(FieldResponseProfileCatalog::default())),
        fields: Box::leak(Box::new(TerrainFieldCatalog::default())),
        footprints: Box::leak(Box::new(FootprintCatalog::default())),
        requirement_revision: 0,
        profile_revision: 0,
    }
}

fn world_with_field(field_id: &str, percent: f32) -> WorldData {
    let layout = WorldConfig::default().chunk_layout();
    let mut world = WorldData::new(layout);
    world.set_authored_extent(ChunkExtent {
        min: ChunkCoord::new(0, 0),
        max: ChunkCoord::new(1, 1),
    });
    bootstrap_constant_field(
        world.terrain_fields_mut(),
        TerrainFieldId::new(field_id),
        ChunkCoord::new(0, 0),
        field_value_from_percent(percent),
    );
    world
}

fn place_building(
    world: &mut WorldData,
    definition: &BuildingDefinition,
) -> crate::world::BuildingId {
    let building_id = world.allocate_building_id();
    let mut record = BuildingRecord::new(
        building_id,
        definition.id.clone(),
        BuildingPlacement::new(
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                crate::world::LocalPosition::new(Vec3::new(64.0, 0.0, 64.0)),
            ),
            Quat::IDENTITY,
        ),
        BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        400,
        BuildingSource::Authored,
    );
    record.lifecycle_state = BuildingLifecycleState::Complete;
    world
        .insert_building(crate::world::ChunkId::new(ChunkCoord::new(0, 0)), record)
        .unwrap();
    building_id
}

#[test]
fn stone_quarry_on_high_stone_field_can_operate() {
    let definition = stone_quarry_definition();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        crate::world::BuildingCatalog::from_definitions(vec![definition.clone()], &categories)
            .unwrap();
    let mut world = world_with_field("stone", 80.0);
    let building_id = place_building(&mut world, &definition);
    let catalogs = catalogs(&building_catalog);
    let layout = WorldConfig::default().chunk_layout();
    let record = world.get_building(building_id).unwrap();
    let assessment = assess_building_terrain(&world, &catalogs, record, layout);
    assert!(assessment.can_operate);
    let requirement = catalogs
        .requirements
        .lookup(&definition.id, &TerrainFieldId::new("stone"))
        .expect("stone requirement");
    let evaluation = evaluate_field_requirement(requirement, &assessment.per_requirement[0]);
    assert!(evaluation.can_operate);
    assert!(evaluation.primary_failure.is_none());
}

#[test]
fn stone_quarry_on_low_stone_field_is_blocked() {
    let definition = stone_quarry_definition();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        crate::world::BuildingCatalog::from_definitions(vec![definition.clone()], &categories)
            .unwrap();
    let mut world = world_with_field("stone", 22.0);
    let building_id = place_building(&mut world, &definition);
    let catalogs = catalogs(&building_catalog);
    let layout = WorldConfig::default().chunk_layout();
    let record = world.get_building(building_id).unwrap();
    let assessment = assess_building_terrain(&world, &catalogs, record, layout);
    assert!(!assessment.can_operate);
    let failure = primary_failure_for_assessment(&assessment.per_requirement[0]);
    assert_eq!(
        failure,
        Some(FieldRequirementFailureReason::AverageBelowMinimum)
    );
}

#[test]
fn display_and_authoritative_evaluation_share_sampled_average() {
    let definition = stone_quarry_definition();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        crate::world::BuildingCatalog::from_definitions(vec![definition.clone()], &categories)
            .unwrap();
    let mut world = world_with_field("stone", 82.0);
    let building_id = place_building(&mut world, &definition);
    let catalogs = catalogs(&building_catalog);
    let layout = WorldConfig::default().chunk_layout();
    let record = world.get_building(building_id).unwrap();
    let assessment = assess_building_terrain(&world, &catalogs, record, layout);
    let requirement = catalogs
        .requirements
        .lookup(&definition.id, &TerrainFieldId::new("stone"))
        .expect("stone requirement");
    let evaluation = evaluate_field_requirement(requirement, &assessment.per_requirement[0]);
    assert_eq!(
        evaluation.sampled_average,
        assessment.per_requirement[0].average_value
    );
    assert_eq!(
        evaluation.display_average_percent,
        evaluation
            .sampled_average
            .map(field_value_to_percent_display)
    );
}

#[test]
fn farm_water_field_evaluation_remains_correct() {
    let definition = farm_definition();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        crate::world::BuildingCatalog::from_definitions(vec![definition.clone()], &categories)
            .unwrap();
    let mut world = world_with_field("water", 80.0);
    let building_id = place_building(&mut world, &definition);
    let catalogs = catalogs(&building_catalog);
    let layout = WorldConfig::default().chunk_layout();
    let record = world.get_building(building_id).unwrap();
    let assessment = assess_building_terrain(&world, &catalogs, record, layout);
    assert!(assessment.can_operate);
    let evaluation = evaluate_field_requirement_assessment(&assessment.per_requirement[0]);
    assert!(evaluation.can_operate);
}

#[test]
fn missing_required_field_reports_unavailable_not_average() {
    let definition = stone_quarry_definition();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        crate::world::BuildingCatalog::from_definitions(vec![definition.clone()], &categories)
            .unwrap();
    let layout = WorldConfig::default().chunk_layout();
    let mut world = WorldData::new(layout);
    world.set_authored_extent(ChunkExtent {
        min: ChunkCoord::new(0, 0),
        max: ChunkCoord::new(1, 1),
    });
    let building_id = place_building(&mut world, &definition);
    let catalogs = catalogs(&building_catalog);
    let record = world.get_building(building_id).unwrap();
    let assessment = assess_building_terrain(&world, &catalogs, record, layout);
    assert!(!assessment.can_operate);
    let failure = primary_failure_for_assessment(&assessment.per_requirement[0]);
    assert_eq!(
        failure,
        Some(FieldRequirementFailureReason::FieldUnavailable)
    );
}

#[test]
fn evaluation_does_not_use_building_name_special_cases() {
    let definition = stone_quarry_definition();
    assert_eq!(definition.id.as_str(), "stone_quarry");
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        crate::world::BuildingCatalog::from_definitions(vec![definition.clone()], &categories)
            .unwrap();
    let mut world = world_with_field("stone", 80.0);
    let building_id = place_building(&mut world, &definition);
    let catalogs = catalogs(&building_catalog);
    let layout = WorldConfig::default().chunk_layout();
    let record = world.get_building(building_id).unwrap();
    let assessment = assess_building_terrain(&world, &catalogs, record, layout);
    assert_eq!(assessment.per_requirement[0].field_id.as_str(), "stone");
}
