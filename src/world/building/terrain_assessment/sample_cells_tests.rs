//! Regression tests for terrain-assessment sampling footprint authority.

use bevy::prelude::{Quat, Vec2, Vec3};

use crate::world::BuildingCategoryId;
use crate::world::building::catalog::{
    BuildingDefinition, BuildingDefinitionId, BuildingRenderKey,
};
use crate::world::building::field_requirement::{
    BuildingFieldRequirementCatalog, BuildingFieldRequirementDefinition,
    BuildingFieldRequirementKind,
};
use crate::world::building::field_response::{
    FieldResponseProfileId, field_value_from_percent, field_value_to_percent_display,
};
use crate::world::building::footprint::FootprintSpec;
use crate::world::building::placement::BuildingPlacement;
use crate::world::building::terrain_assessment::{
    TerrainAssessmentCatalogs, assess_building_terrain_at_placement,
    resolve_building_field_sample_cells,
};
use crate::world::terrain_field::{
    TerrainFieldCatalog, bootstrap_constant_field, sample_terrain_field_at,
};
use crate::world::{
    BuildingCatalog, BuildingCategoryCatalog, ChunkCoord, ChunkExtent, FixedScale,
    FootprintCatalog, LocalPosition, TerrainFieldId, WorldConfig, WorldData, WorldPosition,
};

const QUARRY_EXCAVATION_WIDTH_M: f32 = 12.0;
const QUARRY_EXCAVATION_DEPTH_M: f32 = 12.0;
/// Stone-mine visual baseline X scale from baked catalog sizing (not gameplay instance scale).
const STONE_MINE_VISUAL_BASELINE_SCALE: f32 = 3.58;

fn stone_quarry_definition() -> BuildingDefinition {
    crate::world::starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "stone_quarry")
        .expect("starter stone_quarry")
}

fn farm_definition() -> BuildingDefinition {
    crate::world::starter_building_definitions()
        .into_iter()
        .find(|def| def.id.as_str() == "prispod_farm")
        .expect("starter prispod_farm")
}

fn stone_quarry_requirement() -> BuildingFieldRequirementDefinition {
    BuildingFieldRequirementCatalog::default()
        .lookup(
            &BuildingDefinitionId::new("stone_quarry"),
            &TerrainFieldId::new("stone"),
        )
        .expect("stone quarry requirement")
        .clone()
}

fn farm_requirement() -> BuildingFieldRequirementDefinition {
    BuildingFieldRequirementCatalog::default()
        .lookup(
            &BuildingDefinitionId::new("prispod_farm"),
            &TerrainFieldId::new("water"),
        )
        .expect("farm requirement")
        .clone()
}

fn placement_at(center: Vec2, uniform_scale: f32) -> BuildingPlacement {
    BuildingPlacement::new(
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(center.x, 0.0, center.y)),
        ),
        Quat::IDENTITY,
    )
    .with_uniform_scale(FixedScale::from_f32(uniform_scale).unwrap())
}

fn cell_extent_meters(cells: &[crate::world::OccupancyCellCoord]) -> (f32, f32) {
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for cell in cells {
        let center = cell.center_global();
        min = min.min(center);
        max = max.max(center);
    }
    (max.x - min.x, max.y - min.y)
}

fn catalogs(building_catalog: &BuildingCatalog) -> TerrainAssessmentCatalogs<'static> {
    TerrainAssessmentCatalogs {
        buildings: Box::leak(Box::new(building_catalog.clone())),
        requirements: Box::leak(Box::new(BuildingFieldRequirementCatalog::default())),
        profiles: Box::leak(Box::new(
            crate::world::building::field_response::FieldResponseProfileCatalog::default(),
        )),
        fields: Box::leak(Box::new(TerrainFieldCatalog::default())),
        footprints: Box::leak(Box::new(FootprintCatalog::default())),
        requirement_revision: 0,
        profile_revision: 0,
    }
}

fn world_with_constant_field(field_id: &str, percent: f32) -> WorldData {
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

#[test]
fn metric_sampling_footprint_ignores_placement_uniform_scale() {
    let definition = stone_quarry_definition();
    let requirement = stone_quarry_requirement();
    let footprint_catalog = FootprintCatalog::default();
    let layout = WorldConfig::default().chunk_layout();
    let anchor = Vec2::new(64.0, 64.0);

    let unit_scale_cells = resolve_building_field_sample_cells(
        &definition,
        &requirement,
        &placement_at(anchor, 1.0),
        &footprint_catalog,
        layout,
    )
    .expect("sample cells");
    let scaled_cells = resolve_building_field_sample_cells(
        &definition,
        &requirement,
        &placement_at(anchor, STONE_MINE_VISUAL_BASELINE_SCALE),
        &footprint_catalog,
        layout,
    )
    .expect("sample cells");

    assert_eq!(unit_scale_cells, scaled_cells);
    let (width, depth) = cell_extent_meters(&unit_scale_cells);
    assert!(
        (width - QUARRY_EXCAVATION_WIDTH_M).abs() < 2.5,
        "expected ~{QUARRY_EXCAVATION_WIDTH_M} m width, got {width}"
    );
    assert!(
        (depth - QUARRY_EXCAVATION_DEPTH_M).abs() < 2.5,
        "expected ~{QUARRY_EXCAVATION_DEPTH_M} m depth, got {depth}"
    );
}

#[test]
fn quarry_on_constant_stone_field_reports_full_coverage_with_instance_scale() {
    let definition = stone_quarry_definition();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        BuildingCatalog::from_definitions(vec![definition.clone()], &categories).unwrap();
    let world = world_with_constant_field("stone", 50.0);
    let catalogs = catalogs(&building_catalog);
    let layout = WorldConfig::default().chunk_layout();
    let assessment = assess_building_terrain_at_placement(
        &world,
        &catalogs,
        &definition.id,
        placement_at(Vec2::new(64.0, 64.0), STONE_MINE_VISUAL_BASELINE_SCALE),
        layout,
    );
    let stone = assessment
        .per_requirement
        .iter()
        .find(|req| req.field_id.as_str() == "stone")
        .expect("stone requirement assessment");
    let average_percent = field_value_to_percent_display(stone.average_value.expect("average"));
    assert!(
        (average_percent - 50.0).abs() < 1.0,
        "expected ~50% average, got {average_percent}%"
    );
    assert!(
        (stone.usable_coverage_basis_points.as_percent_display() - 100.0).abs() < 1.0,
        "expected 100% coverage, got {}%",
        stone.usable_coverage_basis_points.as_percent_display()
    );
    assert!(stone.can_operate);
    assert!(assessment.can_operate);
}

#[test]
fn overlay_point_sampling_matches_assessment_cells() {
    let definition = stone_quarry_definition();
    let requirement = stone_quarry_requirement();
    let footprint_catalog = FootprintCatalog::default();
    let field_catalog = TerrainFieldCatalog::default();
    let layout = WorldConfig::default().chunk_layout();
    let world = world_with_constant_field("stone", 63.0);
    let placement = placement_at(Vec2::new(64.0, 64.0), STONE_MINE_VISUAL_BASELINE_SCALE);
    let cells = resolve_building_field_sample_cells(
        &definition,
        &requirement,
        &placement,
        &footprint_catalog,
        layout,
    )
    .expect("sample cells");

    for cell in cells {
        let center = cell.center_global();
        let position = WorldPosition::from_global(Vec3::new(center.x, 0.0, center.y), layout);
        let overlay_sample = sample_terrain_field_at(
            &world,
            &field_catalog,
            &TerrainFieldId::new("stone"),
            position,
        );
        assert!(overlay_sample.availability.is_available());
        assert_eq!(
            overlay_sample.value,
            field_value_from_percent(63.0),
            "overlay and assessment must read the same field authority at {:?}",
            center
        );
    }
}

#[test]
fn farm_sampling_footprint_also_ignores_instance_scale() {
    let definition = farm_definition();
    let requirement = farm_requirement();
    let footprint_catalog = FootprintCatalog::default();
    let layout = WorldConfig::default().chunk_layout();
    let anchor = Vec2::new(80.0, 80.0);

    let unit_scale_cells = resolve_building_field_sample_cells(
        &definition,
        &requirement,
        &placement_at(anchor, 1.0),
        &footprint_catalog,
        layout,
    )
    .expect("sample cells");
    let scaled_cells = resolve_building_field_sample_cells(
        &definition,
        &requirement,
        &placement_at(anchor, 2.5),
        &footprint_catalog,
        layout,
    )
    .expect("sample cells");

    assert_eq!(unit_scale_cells, scaled_cells);
    let (width, depth) = cell_extent_meters(&unit_scale_cells);
    assert!((width - 16.0).abs() < 2.5, "farm width got {width}");
    assert!((depth - 12.0).abs() < 2.5, "farm depth got {depth}");
}

#[test]
fn building_definition_footprint_fallback_still_scales_with_instance() {
    let definition = BuildingDefinition::new(
        BuildingDefinitionId::new("test_mine"),
        "Test Mine",
        BuildingCategoryId::new("production"),
        BuildingRenderKey::reserved("smelter"),
        BuildingRenderKey::reserved("smelter_collision"),
        400,
        90.0,
        FootprintSpec::Rectangle {
            width_meters: 8.0,
            depth_meters: 6.0,
        },
        30.0,
        true,
    );
    let requirement = BuildingFieldRequirementDefinition {
        building_definition_id: definition.id.clone(),
        terrain_field_id: TerrainFieldId::new("iron"),
        requirement_kind: BuildingFieldRequirementKind::RequiredEfficiency,
        response_profile_id: FieldResponseProfileId::new("iron_mine_monotonic"),
        minimum_average: field_value_from_percent(20.0),
        minimum_usable_coverage_basis_points: 4_000,
        usable_value_threshold: field_value_from_percent(15.0),
        sampling_footprint_id: None,
        primary_overlay: true,
        overlay_priority: 0,
        enabled: true,
    };
    let footprint_catalog = FootprintCatalog::default();
    let layout = WorldConfig::default().chunk_layout();
    let anchor = Vec2::new(48.0, 48.0);

    let unit_scale_cells = resolve_building_field_sample_cells(
        &definition,
        &requirement,
        &placement_at(anchor, 1.0),
        &footprint_catalog,
        layout,
    )
    .expect("sample cells");
    let scaled_cells = resolve_building_field_sample_cells(
        &definition,
        &requirement,
        &placement_at(anchor, 2.0),
        &footprint_catalog,
        layout,
    )
    .expect("sample cells");

    assert_ne!(unit_scale_cells, scaled_cells);
    let (unit_width, unit_depth) = cell_extent_meters(&unit_scale_cells);
    let (scaled_width, scaled_depth) = cell_extent_meters(&scaled_cells);
    assert!((scaled_width - unit_width * 2.0).abs() < 3.0);
    assert!((scaled_depth - unit_depth * 2.0).abs() < 3.0);
}
