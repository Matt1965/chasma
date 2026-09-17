use bevy::prelude::*;

use super::{
    TerrainPlacementMode, resolve_building_placement, rotation_from_yaw_and_normal,
    sample_terrain_under_footprint, terrain_clearance_sim,
};
use crate::world::building::catalog::{BuildingCatalog, BuildingDefinitionId};
use crate::world::building::placement_validation::{
    BuildingPlacementContext, BuildingPlacementRejectReason, validate_building_placement,
};
use super::plane_normal_from_coefficients;
use crate::world::{
    BuildingOwnership, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog,
    FootprintCatalog, Heightfield, LocalPosition, UnitCatalog, WorldData, WorldPosition,
    build_building_placement_plan, effective_building_footprint_for_placement,
    presentation_foundation_depth_meters, resolve_authoritative_building_placement,
};

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 128.0,
        units_per_meter: 1.0,
    }
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn world_with_heights(samples: Vec<f32>, samples_per_edge: u32, spacing: f32) -> WorldData {
    let layout = layout();
    let mut world = WorldData::new(layout);
    let heightfield =
        Heightfield::from_samples(samples_per_edge, spacing, samples).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn flat_world() -> WorldData {
    world_with_heights(vec![0.0; 9], 3, 64.0)
}

struct TestCatalogs {
    footprint: FootprintCatalog,
    doodad: DoodadCatalog,
    unit: UnitCatalog,
}

impl Default for TestCatalogs {
    fn default() -> Self {
        Self {
            footprint: FootprintCatalog::default(),
            doodad: DoodadCatalog::default(),
            unit: UnitCatalog::default(),
        }
    }
}

fn ctx<'a>(
    world: &'a WorldData,
    building: &'a BuildingCatalog,
    catalogs: &'a TestCatalogs,
) -> BuildingPlacementContext<'a> {
    BuildingPlacementContext {
        world,
        building_catalog: building,
        footprint_catalog: &catalogs.footprint,
        doodad_catalog: &catalogs.doodad,
        unit_catalog: &catalogs.unit,
        config: Default::default(),
        player_authorized: true,
        terrain_vertical_scale: 1.0,
    }
}

#[test]
fn level_foundation_flat_ground_unchanged() {
    let world = flat_world();
    let building = BuildingCatalog::default();
    let hut = building.get(&BuildingDefinitionId::new("hut")).unwrap();
    let resolved = resolve_building_placement(
        &world,
        layout(),
        hut,
        &FootprintCatalog::default(),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        1.0,
        1.0,
    )
    .unwrap();
    let expected = terrain_clearance_sim(1.0);
    assert!((resolved.anchor.to_global(layout()).y - expected).abs() < 0.01);
    assert_eq!(resolved.rotation, Quat::IDENTITY);
    assert!(resolved.foundation.is_none());
}

#[test]
fn level_foundation_stays_level_on_rotated_slope() {
    let mut samples = vec![0.0f32; 9];
    for row in 0..3u32 {
        for col in 0..3u32 {
            samples[(row * 3 + col) as usize] = col as f32 * 0.4;
        }
    }
    let world = world_with_heights(samples, 3, 64.0);
    let building = BuildingCatalog::default();
    let hut = building.get(&BuildingDefinitionId::new("hut")).unwrap();
    let yaw = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let resolved = resolve_building_placement(
        &world,
        layout(),
        hut,
        &FootprintCatalog::default(),
        pos(64.0, 64.0),
        yaw,
        1.0,
        1.0,
    )
    .unwrap();
    let euler = resolved.rotation.to_euler(EulerRot::YXZ);
    assert!(euler.1.abs() < 0.01);
    assert!(euler.2.abs() < 0.01);
}

#[test]
fn level_foundation_uses_max_height_across_footprint() {
    let world = world_with_heights(
        vec![
            0.0, 0.0, 0.0,
            0.0, 2.5, 0.0,
            0.0, 0.0, 0.0,
        ],
        3,
        64.0,
    );
    let building = BuildingCatalog::default();
    let footprint = FootprintCatalog::default();
    let hut = building.get(&BuildingDefinitionId::new("hut")).unwrap();
    let shape = effective_building_footprint_for_placement(hut, &footprint, 1.0).unwrap();
    let report = sample_terrain_under_footprint(
        &world,
        layout(),
        shape.as_ref(),
        Vec2::new(64.0, 64.0),
        0.0,
    )
    .unwrap();
    assert!((report.max_height - 2.5).abs() < 0.1);

    let resolved = resolve_building_placement(
        &world,
        layout(),
        hut,
        &FootprintCatalog::default(),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        1.0,
        1.0,
    )
    .unwrap();
    assert!((resolved.anchor.to_global(layout()).y - 2.53).abs() < 0.1);
    assert_eq!(resolved.rotation, Quat::IDENTITY);
}

fn stepped_terrain_world() -> WorldData {
    let n = 17u32;
    let mut samples = Vec::new();
    for _row in 0..n {
        for col in 0..n {
            samples.push(if col < 8 { 0.0 } else { 5.0 });
        }
    }
    world_with_heights(samples, n, 8.0)
}

#[test]
fn level_foundation_rejects_deep_foundation() {
    let world = stepped_terrain_world();
    let building = BuildingCatalog::default();
    let hut = building.get(&BuildingDefinitionId::new("hut")).unwrap();
    let err = resolve_building_placement(
        &world,
        layout(),
        hut,
        &FootprintCatalog::default(),
        pos(60.0, 64.0),
        Quat::IDENTITY,
        1.0,
        1.0,
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            BuildingPlacementRejectReason::FoundationTooDeep
                | BuildingPlacementRejectReason::HeightVariationTooLarge
        ),
        "unexpected reject reason: {err:?}"
    );
}

fn mild_x_slope_world() -> WorldData {
    let mut samples = Vec::with_capacity(25);
    for _row in 0..5u32 {
        for col in 0..5u32 {
            samples.push(col as f32 * 0.35);
        }
    }
    world_with_heights(samples, 5, 32.0)
}

#[test]
fn conform_preserves_yaw_and_clears_terrain() {
    let world = mild_x_slope_world();
    let building = BuildingCatalog::default();
    let farm = building
        .get(&BuildingDefinitionId::new("prispod_farm"))
        .unwrap();
    let yaw = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let resolved = resolve_building_placement(
        &world,
        layout(),
        farm,
        &FootprintCatalog::default(),
        pos(64.0, 64.0),
        yaw,
        1.0,
        1.0,
    )
    .unwrap();
    let up = resolved.rotation * Vec3::Y;
    assert!(up.y > 0.5);
    let yaw_radians = std::f32::consts::FRAC_PI_2;
    let yaw_forward = Vec3::new(yaw_radians.sin(), 0.0, -yaw_radians.cos());
    let expected_forward = (yaw_forward - up * yaw_forward.dot(up)).normalize();
    let actual_forward = (resolved.rotation * Vec3::NEG_Z).normalize();
    assert!(actual_forward.dot(expected_forward) > 0.98);

    let footprint = FootprintCatalog::default();
    let shape = effective_building_footprint_for_placement(farm, &footprint, 1.0).unwrap();
    let report = sample_terrain_under_footprint(
        &world,
        layout(),
        shape.as_ref(),
        Vec2::new(64.0, 64.0),
        std::f32::consts::FRAC_PI_2,
    )
    .unwrap();
    let anchor_xz = Vec2::new(64.0, 64.0);
    let anchor_y = resolved.anchor.to_global(layout()).y;
    for sample in &report.samples {
        let base = super::base_height_on_plane_at(
            anchor_xz,
            anchor_y,
            up,
            sample.global_xz,
        );
        assert!(base >= sample.height - 0.01);
    }
}

#[test]
fn conform_rejects_irregular_terrain() {
    let world = stepped_terrain_world();
    let building = BuildingCatalog::default();
    let mine = building
        .get(&BuildingDefinitionId::new("iron_mine"))
        .unwrap();
    let err = resolve_building_placement(
        &world,
        layout(),
        mine,
        &FootprintCatalog::default(),
        pos(60.0, 64.0),
        Quat::IDENTITY,
        1.0,
        1.0,
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            BuildingPlacementRejectReason::TerrainTooRough
                | BuildingPlacementRejectReason::HeightVariationTooLarge
                | BuildingPlacementRejectReason::SlopeTooSteep
        ),
        "unexpected reject reason: {err:?}"
    );
}

#[test]
fn preview_and_commit_share_resolver() {
    let world = flat_world();
    let building = BuildingCatalog::default();
    let catalogs = TestCatalogs::default();
    let c = ctx(&world, &building, &catalogs);
    let anchor = pos(64.0, 64.0);
    let rotation = Quat::IDENTITY;
    let plan = build_building_placement_plan(
        &c,
        &BuildingDefinitionId::new("hut"),
        anchor,
        rotation,
        BuildingOwnership::neutral(),
    );
    let validation = validate_building_placement(
        &c,
        &BuildingDefinitionId::new("hut"),
        anchor,
        rotation,
        BuildingOwnership::neutral(),
    );
    assert_eq!(plan.grounded_anchor, validation.grounded_anchor.unwrap());
    assert_eq!(plan.rotation, validation.resolved_rotation.unwrap());
}

fn steep_x_slope_world() -> WorldData {
    let mut samples = Vec::with_capacity(25);
    for _row in 0..5u32 {
        for col in 0..5u32 {
            samples.push(col as f32 * 2.0);
        }
    }
    world_with_heights(samples, 5, 32.0)
}

#[test]
fn presentation_clearance_stays_small_on_exaggerated_vertical_scale() {
    use super::terrain_clearance_sim;
    use crate::terrain::render_height;

    let vertical_scale = 20_000.0;
    let max_height = 0.00001;
    let legacy_floor = max_height + 0.03;
    let fixed_floor = max_height + terrain_clearance_sim(vertical_scale);
    let legacy_gap = render_height(legacy_floor, vertical_scale) - render_height(max_height, vertical_scale);
    let fixed_gap =
        render_height(fixed_floor, vertical_scale) - render_height(max_height, vertical_scale);
    assert!(legacy_gap > 100.0, "legacy 0.03m sim clearance balloons in render space");
    assert!(
        (fixed_gap - super::PRESENTATION_TERRAIN_CLEARANCE_METERS).abs() < 0.01,
        "presentation clearance should stay ~5cm visible"
    );
}

#[test]
fn presentation_depth_matches_visible_delta_when_sim_range_is_tiny() {
    use super::{
        FoundationSkirtSpec, TerrainFootprintReport, TerrainFootprintSample,
        presentation_foundation_depth_meters,
    };

    let terrain = TerrainFootprintReport {
        samples: Vec::new(),
        perimeter: Vec::new(),
        min_height: 0.0,
        max_height: 0.08,
        height_range: 0.08,
        plane_a: 0.012,
        plane_b: 0.0,
        plane_c: 0.0,
        plane_normal: Vec3::new(-0.012, 1.0, 0.0).normalize(),
        plane_slope_degrees: 0.69,
        plane_rms_residual: 0.0,
        plane_peak_residual: 0.0,
    };
    let vertical_scale = 9.25;
    let span = 5.0;
    let presentation_depth =
        presentation_foundation_depth_meters(&terrain, span, vertical_scale);
    assert!(
        presentation_depth >= 0.7,
        "expected ~0.74m visible variation, got {}",
        presentation_depth
    );
    let perimeter = vec![
        TerrainFootprintSample {
            global_xz: Vec2::new(-2.0, -2.0),
            height: 0.0,
        },
        TerrainFootprintSample {
            global_xz: Vec2::new(2.0, -2.0),
            height: 0.0,
        },
        TerrainFootprintSample {
            global_xz: Vec2::new(2.0, 2.0),
            height: -0.08,
        },
    ];
    let spec = FoundationSkirtSpec::from_level_placement(
        Vec2::ZERO,
        0.08,
        &perimeter,
        0.0,
        0.0,
        &terrain,
        span,
        vertical_scale,
    )
    .expect("presentation depth should produce a skirt spec");
    assert!(spec.presentation_depth_meters >= 0.7);
    assert!(spec.outward_expansion_meters >= 0.7);
    assert!(terrain.height_range < 0.1);
}

#[test]
fn foundation_skirt_rejects_excessive_presentation_depth() {
    use super::{
        FoundationSkirtSpec, MAX_FOUNDATION_VISIBLE_DEPTH_METERS, TerrainFootprintReport,
        TerrainFootprintSample, presentation_foundation_depth_meters,
    };

    let terrain = TerrainFootprintReport {
        samples: Vec::new(),
        perimeter: Vec::new(),
        min_height: 0.0,
        max_height: 5.0,
        height_range: 5.0,
        plane_a: 1.0,
        plane_b: 0.0,
        plane_c: 0.0,
        plane_normal: Vec3::new(-0.2, 1.0, 0.0).normalize(),
        plane_slope_degrees: 11.0,
        plane_rms_residual: 0.0,
        plane_peak_residual: 0.0,
    };
    let span = 6.0;
    let depth = presentation_foundation_depth_meters(&terrain, span, 1.0);
    assert!(depth > MAX_FOUNDATION_VISIBLE_DEPTH_METERS);
    let perimeter = vec![TerrainFootprintSample {
        global_xz: Vec2::new(1.0, 0.0),
        height: 0.0,
    }];
    assert!(
        FoundationSkirtSpec::from_level_placement(
            Vec2::ZERO,
            5.0,
            &perimeter,
            0.0,
            0.0,
            &terrain,
            span,
            1.0,
        )
        .is_none()
    );
}

#[test]
fn authoritative_resolver_matches_build_mode_validation() {
    let world = mild_x_slope_world();
    let building = BuildingCatalog::default();
    let catalogs = TestCatalogs::default();
    let ctx = ctx(&world, &building, &catalogs);
    let anchor = pos(64.0, 64.0);
    let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
    let authoritative = resolve_authoritative_building_placement(
        &ctx,
        &BuildingDefinitionId::new("hut"),
        anchor,
        rotation,
        BuildingOwnership::neutral(),
    );
    let direct = validate_building_placement(
        &ctx,
        &BuildingDefinitionId::new("hut"),
        anchor,
        rotation,
        BuildingOwnership::neutral(),
    );
    assert_eq!(authoritative.valid, direct.valid);
    assert_eq!(authoritative.grounded_anchor, direct.grounded_anchor);
    assert_eq!(authoritative.resolved_rotation, direct.resolved_rotation);
    assert_eq!(
        authoritative.foundation.is_some(),
        direct.foundation.is_some()
    );
}

#[test]
fn level_foundation_produces_skirt_spec_on_slope() {
    let world = steep_x_slope_world();
    let building = BuildingCatalog::default();
    let hut = building.get(&BuildingDefinitionId::new("hut")).unwrap();
    let resolved = resolve_building_placement(
        &world,
        layout(),
        hut,
        &FootprintCatalog::default(),
        pos(64.0, 64.0),
        Quat::IDENTITY,
        1.0,
        1.0,
    )
    .unwrap();
    let spec = resolved.foundation.expect("foundation skirt spec");
    assert!(spec.max_depth_meters >= super::mode::FOUNDATION_SKIRT_MIN_DEPTH);
    assert!(spec.perimeter.len() >= 3);
}

#[test]
fn placed_hut_preserves_resolved_anchor_and_yaw() {
    use crate::world::{BuildingOwnership, OccupancyCatalogs, place_player_building};

    let mut world = mild_x_slope_world();
    let building = BuildingCatalog::default();
    let footprint = FootprintCatalog::default();
    let doodad = DoodadCatalog::default();
    let (grounded, rotation) = {
        let catalogs = TestCatalogs::default();
        let ctx = ctx(&world, &building, &catalogs);
        let validation = validate_building_placement(
            &ctx,
            &BuildingDefinitionId::new("hut"),
            pos(64.0, 64.0),
            Quat::IDENTITY,
            BuildingOwnership::neutral(),
        );
        assert!(validation.valid);
        (
            validation.grounded_anchor.unwrap(),
            validation.resolved_rotation.unwrap(),
        )
    };
    let occ = OccupancyCatalogs {
        doodad: &doodad,
        building: &building,
        footprint: &footprint,
    };
    let record = place_player_building(
        &building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        grounded,
        rotation,
        BuildingOwnership::neutral(),
        occ,
    )
    .unwrap();
    assert_eq!(record.placement.position, grounded);
    assert_eq!(record.placement.rotation, rotation);
    let euler = rotation.to_euler(EulerRot::YXZ);
    assert!(euler.1.abs() < 0.01);
    assert!(euler.2.abs() < 0.01);
    assert!(
        grounded.to_global(layout()).y > 0.03,
        "level foundation should lift floor above downhill terrain"
    );
}

#[test]
fn validation_resolves_conform_tilt_for_prispod_farm() {
    use crate::world::BuildingOwnership;

    let world = mild_x_slope_world();
    let building = BuildingCatalog::default();
    let yaw = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let catalogs = TestCatalogs::default();
    let ctx = ctx(&world, &building, &catalogs);
    let validation = validate_building_placement(
        &ctx,
        &BuildingDefinitionId::new("prispod_farm"),
        pos(64.0, 64.0),
        yaw,
        BuildingOwnership::neutral(),
    );
    assert!(validation.valid);
    let rotation = validation.resolved_rotation.unwrap();
    let up = rotation * Vec3::Y;
    assert!(up.y > 0.5, "conform validation should resolve tilted up vector");
}

#[cfg(feature = "data-import")]
#[test]
fn dev_catalog_exposes_terrain_placement_modes() {
    use crate::data_import::resolve_dev_building_catalog;
    use crate::world::InventoryProfileCatalog;

    let profiles = InventoryProfileCatalog::default();
    let (_, catalog) = resolve_dev_building_catalog(&profiles, None);
    let farm = catalog
        .get(&BuildingDefinitionId::new("prispod_farm"))
        .expect("prispod_farm in dev catalog");
    let hut = catalog.get(&BuildingDefinitionId::new("hut")).expect("hut in dev catalog");
    assert_eq!(farm.terrain_placement_mode, TerrainPlacementMode::ConformToTerrain);
    assert_eq!(hut.terrain_placement_mode, TerrainPlacementMode::LevelFoundation);
}

#[test]
fn plane_normal_from_x_slope() {
    let normal = plane_normal_from_coefficients(1.0, 0.0);
    assert!(normal.x < 0.0);
    assert!(normal.y > 0.0);
    assert!(normal.z.abs() < 0.01);
}

#[test]
fn rotation_from_yaw_and_normal_is_stable() {
    let normal = Vec3::new(-0.2, 1.0, 0.0).normalize();
    let rot = rotation_from_yaw_and_normal(0.0, normal);
    let up = rot * Vec3::Y;
    assert!(up.dot(normal) > 0.99);
}

#[test]
fn presentation_vertical_scale_steepens_conform_tilt() {
    let world = steep_x_slope_world();
    let building = BuildingCatalog::default();
    let farm = building
        .get(&BuildingDefinitionId::new("prispod_farm"))
        .unwrap();
    let anchor = pos(64.0, 64.0);
    let yaw = Quat::IDENTITY;
    let sim = resolve_building_placement(
        &world,
        layout(),
        farm,
        &FootprintCatalog::default(),
        anchor,
        yaw,
        1.0,
        1.0,
    )
    .unwrap();
    let pres = resolve_building_placement(
        &world,
        layout(),
        farm,
        &FootprintCatalog::default(),
        anchor,
        yaw,
        1.0,
        3.0,
    )
    .unwrap();
    let sim_tilt = (sim.rotation * Vec3::Y).angle_between(Vec3::Y);
    let pres_tilt = (pres.rotation * Vec3::Y).angle_between(Vec3::Y);
    assert!(
        pres_tilt > sim_tilt + 0.01,
        "presentation vertical scale should steepen conform tilt (sim={sim_tilt}, pres={pres_tilt})"
    );
}

#[test]
fn gameplay_chain_hut_yaw_steps_remain_valid_on_same_anchor() {
    use crate::world::rotation_from_quadrants;

    let world = steep_x_slope_world();
    let building = BuildingCatalog::default();
    let catalogs = TestCatalogs::default();
    let c = ctx(&world, &building, &catalogs);
    let anchor = pos(64.0, 64.0);
    for quadrants in [0u8, 1, 3] {
        let rotation = rotation_from_quadrants(quadrants);
        let validation = validate_building_placement(
            &c,
            &BuildingDefinitionId::new("hut"),
            anchor,
            rotation,
            BuildingOwnership::neutral(),
        );
        assert!(
            validation.valid,
            "hut yaw quadrant {} should stay valid: {:?}",
            quadrants,
            validation.primary_reason
        );
        let plan = build_building_placement_plan(
            &c,
            &BuildingDefinitionId::new("hut"),
            anchor,
            rotation,
            BuildingOwnership::neutral(),
        );
        assert!(plan.is_valid());
        assert!(validation.foundation.is_some());
    }
}

#[test]
fn gameplay_chain_farm_yaw_steps_remain_valid_on_same_anchor() {
    use crate::world::rotation_from_quadrants;

    let world = steep_x_slope_world();
    let building = BuildingCatalog::default();
    let catalogs = TestCatalogs::default();
    let c = ctx(&world, &building, &catalogs);
    let anchor = pos(64.0, 64.0);
    for quadrants in [0u8, 1, 3] {
        let rotation = rotation_from_quadrants(quadrants);
        let validation = validate_building_placement(
            &c,
            &BuildingDefinitionId::new("prispod_farm"),
            anchor,
            rotation,
            BuildingOwnership::neutral(),
        );
        assert!(
            validation.valid,
            "farm yaw quadrant {} should stay valid: {:?}",
            quadrants,
            validation.primary_reason
        );
        let plan = build_building_placement_plan(
            &c,
            &BuildingDefinitionId::new("prispod_farm"),
            anchor,
            rotation,
            BuildingOwnership::neutral(),
        );
        assert!(plan.is_valid(), "farm plan invalid at quadrant {}", quadrants);
        let resolved = plan.rotation;
        let up = resolved * Vec3::Y;
        assert!(up.y > 0.5, "farm should resolve conform up vector at quadrant {}", quadrants);
    }
}

#[test]
fn conform_tilted_rotation_registers_occupancy() {
    use crate::world::{
        BuildingId, BuildingLifecycleState, BuildingPlacement, BuildingRecord, BuildingSource,
        OccupancyCatalogs, QuantizedRotation, register_building_occupancy,
    };

    let mut world = flat_world();
    let building = BuildingCatalog::default();
    let catalogs = TestCatalogs::default();
    let farm = building
        .get(&BuildingDefinitionId::new("prispod_farm"))
        .unwrap();
    let grounded = pos(64.0, 64.0);
    let tilted = rotation_from_yaw_and_normal(
        0.0,
        plane_normal_from_coefficients(0.4, 0.0),
    );
    assert!(QuantizedRotation::from_quat(tilted).is_err());
    assert_eq!(
        QuantizedRotation::yaw_for_occupancy(tilted).unwrap(),
        QuantizedRotation::Deg0
    );

    let mut record = BuildingRecord::new(
        BuildingId::new(1),
        farm.id.clone(),
        BuildingPlacement::new(grounded, tilted),
        BuildingOwnership::neutral(),
        farm.max_hp,
        BuildingSource::Authored,
    );
    record.lifecycle_state = BuildingLifecycleState::Planned;
    let occ = OccupancyCatalogs {
        doodad: &catalogs.doodad,
        building: &building,
        footprint: &catalogs.footprint,
    };
    register_building_occupancy(&mut world, occ, &record).expect("tilted conform rotation");
}

#[test]
fn foundation_skirt_mesh_scales_terrain_delta_with_vertical_scale() {
    use super::FoundationPerimeterVertex;
    use crate::buildings::foundation::{build_foundation_skirt_mesh, FoundationUvMode};
    use crate::world::building::terrain_placement::FoundationSkirtSpec;

    let spec = FoundationSkirtSpec {
        perimeter: vec![
            FoundationPerimeterVertex {
                local_xz: Vec2::new(-2.0, -2.0),
                terrain_world_y: 0.0,
            },
            FoundationPerimeterVertex {
                local_xz: Vec2::new(2.0, -2.0),
                terrain_world_y: 0.0,
            },
            FoundationPerimeterVertex {
                local_xz: Vec2::new(2.0, 2.0),
                terrain_world_y: 0.0,
            },
            FoundationPerimeterVertex {
                local_xz: Vec2::new(-2.0, 2.0),
                terrain_world_y: -1.0,
            },
        ],
        floor_world_y: 2.0,
        max_depth_meters: 2.0,
        presentation_depth_meters: 2.0,
        outward_expansion_meters: 2.0,
    };
    let mesh_vs1 = build_foundation_skirt_mesh(&spec, 2.0, 1.0, FoundationUvMode::Tiled);
    let mesh_vs3 = build_foundation_skirt_mesh(&spec, 2.0, 3.0, FoundationUvMode::Tiled);
    let positions_vs1 = mesh_vs1
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap()
        .as_float3()
        .unwrap();
    let positions_vs3 = mesh_vs3
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap()
        .as_float3()
        .unwrap();
    let min_y_vs1 = positions_vs1.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let min_y_vs3 = positions_vs3.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    assert!(
        min_y_vs3.abs() > min_y_vs1.abs() + 0.5,
        "skirt bottom should reach further in render space when vertical_scale > 1"
    );
}
