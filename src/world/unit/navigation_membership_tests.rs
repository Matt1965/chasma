//! IN-11gG-M: explicit spawn/load navigation membership initialization.

use std::path::Path;

use bevy::prelude::*;

use super::navigation_membership::{
    infer_navigation_membership_at_position, initialize_unit_navigation_membership,
};
use crate::world::interaction::{
    InteractionOrderPlan, InteractionResolveContext, interaction_plan_to_unit_order,
    resolve_world_click_to_order,
};
use crate::world::{
    Affiliation, AttackTargetingPolicy, BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH,
    BuildingCatalog, BuildingCategoryCatalog, BuildingDefinition, BuildingDefinitionId, BuildingId,
    BuildingInteractionProfileCatalog, BuildingLifecycleState, BuildingNavigationBlueprint,
    BuildingNavigationBlueprintCatalog, BuildingOwnership, BuildingRenderKey, ChunkCoord,
    ChunkLayout, DoodadCatalog, FootprintCatalog, InteriorProfileCatalog, ItemPileSettings,
    NavigationConfig, NavigationEntranceDefinition, NavigationFloorDefinition, NavigationPolygon2d,
    NavigationRegionDefinition, OccupancyCatalogs, PassabilityCatalogs, SpaceId, UnitCatalog,
    UnitDefinitionId, UnitOwnership, UnitSource, UnitState, WeaponCatalog, WorldData,
    WorldPosition, create_unit, create_unit_with_ownership, issue_unit_order,
    place_player_building, resolve_navigation_start_space, resolve_pending_unit_orders,
    set_building_lifecycle_stage, step_unit_movement,
};

fn layout_world() -> WorldData {
    let layout = ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    };
    let mut world = WorldData::new(layout);
    let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
        crate::world::ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        crate::world::LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn imported_survival_hut_definition() -> BuildingDefinition {
    BuildingDefinition::new(
        BuildingDefinitionId::new("hut"),
        "Survival Hut",
        crate::world::BuildingCategoryId::new("residential"),
        crate::world::BuildingRenderKey::reserved("hut"),
        crate::world::BuildingRenderKey::reserved("hut_collision"),
        250,
        45.0,
        crate::world::FootprintSpec::Rectangle {
            width_meters: 4.0,
            depth_meters: 4.0,
        },
        35.0,
        true,
    )
}

fn imported_building_catalog() -> BuildingCatalog {
    BuildingCatalog::from_definitions(
        vec![imported_survival_hut_definition()],
        &BuildingCategoryCatalog::default(),
    )
    .expect("imported hut catalog")
}

fn persisted_hut_nav_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("hut_nav", "Survival Hut Navigation")
        .with_floors(vec![NavigationFloorDefinition {
            floor_id: 0,
            key: "floor_0".to_string(),
            display_label: "Floor 1.3m".to_string(),
            elevation_meters: 1.269_120_6,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![NavigationRegionDefinition {
                key: "region_3".to_string(),
                display_label: "Region 3".to_string(),
                room_tag: None,
                walkable_outline: NavigationPolygon2d {
                    vertices_xz: vec![
                        [3.979_980_5, -3.513_916],
                        [4.400_512_7, 4.112_793],
                        [-3.815_185_5, 4.112_793],
                        [-3.553_222_7, -3.517_578_1],
                    ],
                },
            }],
        }])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "entrance".to_string(),
            floor_key: "floor_0".to_string(),
            region_key: Some("region_3".to_string()),
            local_position_xz: [0.058_471_68, -2.773_437_5],
            radius_meters: 1.5,
            interior_spawn_local: [0.058_471_68, 1.269_120_6, -2.059_437_5],
            bidirectional: true,
            door_key: None,
        }])
}

fn persisted_nav_catalog() -> BuildingNavigationBlueprintCatalog {
    BuildingNavigationBlueprintCatalog::from_definitions(vec![persisted_hut_nav_blueprint()])
        .expect("persisted hut_nav catalog")
}

fn persisted_hut_nav_catalog_from_assets() -> BuildingNavigationBlueprintCatalog {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH);
    BuildingNavigationBlueprintCatalog::load_from_ron_path(&path).expect("nav catalog")
}

fn imported_hut_catalog_from_assets() -> BuildingCatalog {
    BuildingCatalog::from_definitions(
        vec![BuildingDefinition::new(
            BuildingDefinitionId::new("hut"),
            "Survival Hut",
            crate::world::BuildingCategoryId::new("residential"),
            BuildingRenderKey::reserved("hut"),
            BuildingRenderKey::reserved("hut_collision"),
            250,
            45.0,
            crate::world::FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        )],
        &BuildingCategoryCatalog::default(),
    )
    .expect("hut catalog")
}

fn activate_imported_hut_at(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    placement: WorldPosition,
) -> BuildingId {
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = OccupancyCatalogs {
        building: building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();

    let building_id = place_player_building(
        building_catalog,
        world,
        &BuildingDefinitionId::new("hut"),
        placement,
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .expect("place hut")
    .id;

    set_building_lifecycle_stage(
        world,
        building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        Some(nav_catalog),
        building_id,
        BuildingLifecycleState::Complete,
        1.0,
    )
    .expect("complete hut");
    building_id
}

fn activate_imported_hut(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
) -> BuildingId {
    activate_imported_hut_at(world, building_catalog, nav_catalog, pos(80.0, 80.0))
}

fn real_hut_region_space(world: &WorldData, building_id: BuildingId) -> SpaceId {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    runtime
        .regions
        .iter()
        .find(|region| region.region_key == "region_3")
        .expect("region_3")
        .space_id
}

#[test]
fn exterior_spawn_remains_surface() {
    let mut world = layout_world();
    let unit_catalog = UnitCatalog::default();
    let record = create_unit(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        pos(10.0, 10.0),
        UnitSource::Authored,
    )
    .expect("spawn");
    assert_eq!(record.current_space_id, SpaceId::SURFACE);
}

#[test]
fn projected_overlap_outside_floor_tolerance_stays_surface() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);
    let region_space = real_hut_region_space(&world, building_id);
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let floor_y = world
        .space_registry()
        .get_space(region_space)
        .expect("space")
        .floor_y_global;
    let interior_xz = runtime
        .model_transform
        .transform_point(Vec3::new(0.0, 0.0, 1.0));
    let far_y = floor_y + 3.0;
    let overlap_pos = WorldPosition::from_global(
        Vec3::new(interior_xz.x, far_y, interior_xz.z),
        world.layout(),
    );
    assert_eq!(
        infer_navigation_membership_at_position(&world, overlap_pos),
        SpaceId::SURFACE,
        "XZ overlap with Y outside floor tolerance must not infer interior membership"
    );
}

#[test]
fn real_hut_spawn_inside_region_assigns_interior_membership() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);
    let region_space = real_hut_region_space(&world, building_id);
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let interior_xz = runtime
        .model_transform
        .transform_point(Vec3::new(0.0, 0.0, 1.0));
    let spawn_pos =
        WorldPosition::from_global(Vec3::new(interior_xz.x, 0.0, interior_xz.z), world.layout());
    assert_eq!(
        infer_navigation_membership_at_position(&world, spawn_pos),
        region_space,
        "positional query must classify terrain-height spawn inside region_3"
    );

    let unit_catalog = UnitCatalog::default();
    let unit_id = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        spawn_pos,
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .expect("spawn")
    .id;
    assert_eq!(
        world.get_unit(unit_id).unwrap().current_space_id,
        region_space,
        "spawn initialization must assign interior membership without manual SpaceId"
    );
}

#[test]
fn real_hut_spawn_inside_moves_via_same_space_path() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);
    let region_space = real_hut_region_space(&world, building_id);
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime")
        .clone();
    let floor_y = world
        .space_registry()
        .get_space(region_space)
        .expect("space")
        .floor_y_global;
    let interior_xz = runtime
        .model_transform
        .transform_point(Vec3::new(0.0, 0.0, 1.0));
    let spawn_pos =
        WorldPosition::from_global(Vec3::new(interior_xz.x, 0.0, interior_xz.z), world.layout());
    let goal_global = runtime
        .model_transform
        .transform_point(Vec3::new(0.5, 0.0, 1.5));
    let goal = WorldPosition::from_global(
        Vec3::new(goal_global.x, floor_y, goal_global.z),
        world.layout(),
    );
    assert!(
        crate::world::interior_position_walkable(
            world.building_navigation_runtime(),
            world.space_registry(),
            world.layout(),
            goal,
            region_space,
        ),
        "goal must be walkable inside region_3"
    );

    let unit_catalog = UnitCatalog::default();
    let weapon_catalog = WeaponCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let nav_config = NavigationConfig::default();

    let unit_id = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        spawn_pos,
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .expect("spawn")
    .id;
    assert_eq!(
        world.get_unit(unit_id).unwrap().current_space_id,
        region_space
    );

    issue_unit_order(
        &mut world,
        &unit_catalog,
        &weapon_catalog,
        &doodad_catalog,
        &nav_config,
        unit_id,
        crate::world::UnitOrder::MoveTo { target: goal },
        AttackTargetingPolicy::default(),
    )
    .expect("issue");
    let resolve_report =
        resolve_pending_unit_orders(&mut world, &unit_catalog, catalogs, &nav_config);
    assert_eq!(
        resolve_report.resolved, 1,
        "path resolution failures: {:?}",
        resolve_report.failures
    );

    let command = world
        .movement_authority_trace()
        .latest_command_for_unit(unit_id)
        .expect("command trace");
    assert_eq!(command.start_space, region_space);
    assert_eq!(command.goal_space, region_space);
    assert_ne!(command.start_space, SpaceId::SURFACE);

    let layout = world.layout();
    let goal_xz = goal.to_global(layout).xz();
    let mut arrived = false;
    for _ in 0..400 {
        step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        assert_eq!(record.current_space_id, region_space);
        if record
            .placement
            .position
            .to_global(layout)
            .xz()
            .distance(goal_xz)
            < 1.5
            && matches!(record.state, UnitState::Idle)
        {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "interior same-space move must advance unit");
}

#[test]
fn real_hut_spawn_inside_moves_via_player_command_resolution_path() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);
    let region_space = real_hut_region_space(&world, building_id);
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime")
        .clone();
    let floor_y = world
        .space_registry()
        .get_space(region_space)
        .expect("space")
        .floor_y_global;
    let interior_xz = runtime
        .model_transform
        .transform_point(Vec3::new(0.0, 0.0, 1.0));
    let spawn_pos =
        WorldPosition::from_global(Vec3::new(interior_xz.x, 0.0, interior_xz.z), world.layout());
    let click_goal_global = runtime
        .model_transform
        .transform_point(Vec3::new(0.5, 0.0, 1.5));
    let click_goal = WorldPosition::from_global(
        Vec3::new(click_goal_global.x, floor_y, click_goal_global.z),
        world.layout(),
    );
    assert!(
        crate::world::interior_position_walkable(
            world.building_navigation_runtime(),
            world.space_registry(),
            world.layout(),
            click_goal,
            region_space,
        ),
        "click goal must be walkable inside region_3"
    );

    let unit_catalog = UnitCatalog::default();
    let weapon_catalog = WeaponCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let interaction_catalog = BuildingInteractionProfileCatalog::default();
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let nav_config = NavigationConfig::default();

    let unit_id = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        spawn_pos,
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .expect("spawn")
    .id;
    assert_eq!(
        world.get_unit(unit_id).unwrap().current_space_id,
        region_space
    );

    let selected = [unit_id];
    let pile_settings = ItemPileSettings::default();
    let ctx = InteractionResolveContext::new(
        &world,
        &doodad_catalog,
        &building_catalog,
        &footprint,
        &interaction_catalog,
        &unit_catalog,
        &weapon_catalog,
        &pile_settings,
        &selected,
    );
    let plan = resolve_world_click_to_order(&ctx, click_goal).expect("click resolves");
    assert!(matches!(plan, InteractionOrderPlan::MoveTo { .. }));
    let order = interaction_plan_to_unit_order(plan).expect("MoveTo order");
    issue_unit_order(
        &mut world,
        &unit_catalog,
        &weapon_catalog,
        &doodad_catalog,
        &nav_config,
        unit_id,
        order,
        AttackTargetingPolicy::default(),
    )
    .expect("issue");
    let resolve_report =
        resolve_pending_unit_orders(&mut world, &unit_catalog, catalogs, &nav_config);
    assert_eq!(
        resolve_report.resolved, 1,
        "path resolution failures: {:?}",
        resolve_report.failures
    );

    let command = world
        .movement_authority_trace()
        .latest_command_for_unit(unit_id)
        .expect("command trace");
    assert_eq!(command.start_space, region_space);
    assert_eq!(command.goal_space, region_space);
    assert_ne!(command.start_space, SpaceId::SURFACE);

    let layout = world.layout();
    let goal_xz = click_goal.to_global(layout).xz();
    let mut arrived = false;
    for _ in 0..400 {
        step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        assert_eq!(record.current_space_id, region_space);
        if record
            .placement
            .position
            .to_global(layout)
            .xz()
            .distance(goal_xz)
            < 1.5
            && matches!(record.state, UnitState::Idle)
        {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "real command path must complete interior move");
}

#[test]
fn surface_unit_overlapping_interior_outline_stays_surface_during_simulation() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);
    let region_space = real_hut_region_space(&world, building_id);
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let floor_y = world
        .space_registry()
        .get_space(region_space)
        .expect("space")
        .floor_y_global;
    let interior_xz = runtime
        .model_transform
        .transform_point(Vec3::new(0.0, 0.0, 1.0));
    let overlap_pos =
        WorldPosition::from_global(Vec3::new(interior_xz.x, 0.0, interior_xz.z), world.layout());
    let move_target = WorldPosition::from_global(
        Vec3::new(interior_xz.x + 0.5, 0.0, interior_xz.z + 0.5),
        world.layout(),
    );

    let unit_catalog = UnitCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let exterior_spawn =
        runtime
            .model_transform
            .transform_point(Vec3::new(0.058_471_68, 0.0, -12.0));
    let spawn_pos = WorldPosition::from_global(
        Vec3::new(exterior_spawn.x, 0.0, exterior_spawn.z),
        world.layout(),
    );
    let unit_id = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        spawn_pos,
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .expect("spawn")
    .id;
    world
        .set_unit_current_space(unit_id, SpaceId::SURFACE)
        .expect("force surface");
    world
        .update_unit_position(unit_id, overlap_pos)
        .expect("place inside projection");

    let resolved = resolve_navigation_start_space(
        world.building_navigation_runtime(),
        world.space_registry(),
        world.layout(),
        overlap_pos,
        SpaceId::SURFACE,
    );
    assert_eq!(resolved, SpaceId::SURFACE);
    assert_ne!(resolved, region_space);
    assert_eq!(
        world.get_unit(unit_id).unwrap().current_space_id,
        SpaceId::SURFACE
    );

    let path = crate::world::NavigationPath::new(vec![
        crate::world::NavigationWaypoint::in_space(overlap_pos, SpaceId::SURFACE),
        crate::world::NavigationWaypoint::in_space(move_target, SpaceId::SURFACE),
    ]);
    world
        .set_unit_state(
            unit_id,
            UnitState::Moving {
                target: move_target,
                path,
                waypoint_index: 0,
            },
        )
        .expect("start surface-only path through overlap projection");

    let layout = world.layout();
    for _ in 0..200 {
        step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        assert_eq!(
            record.current_space_id,
            SpaceId::SURFACE,
            "overlap during ordinary movement must not infer interior membership"
        );
        assert!(
            record.placement.position.to_global(layout).y.abs() < 0.25,
            "surface-tracked overlap move must not snap Y to interior floor"
        );
        if matches!(record.state, UnitState::Idle) {
            break;
        }
    }
    assert_eq!(
        world.get_unit(unit_id).unwrap().current_space_id,
        SpaceId::SURFACE
    );
}

/// IN-11gI-B: Surface-only movement through the entrance projection must not infer membership.
#[test]
fn surface_only_movement_through_entrance_projection_stays_surface() {
    let mut world = layout_world();
    let building_catalog = imported_hut_catalog_from_assets();
    let nav_catalog = persisted_hut_nav_catalog_from_assets();
    let building_id =
        activate_imported_hut_at(&mut world, &building_catalog, &nav_catalog, pos(80.0, 80.0));
    let region_space = real_hut_region_space(&world, building_id);
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let floor_y = world
        .space_registry()
        .get_space(region_space)
        .expect("space")
        .floor_y_global;
    let layout = world.layout();
    let interior_xz = runtime
        .model_transform
        .transform_point(Vec3::new(0.0, 0.0, 1.0));
    let start =
        WorldPosition::from_global(Vec3::new(interior_xz.x, 0.0, interior_xz.z), world.layout());
    let goal_global = runtime
        .model_transform
        .transform_point(Vec3::new(0.058_471_68, 0.0, -8.0));
    let goal =
        WorldPosition::from_global(Vec3::new(goal_global.x, 0.0, goal_global.z), world.layout());
    let exterior_spawn =
        runtime
            .model_transform
            .transform_point(Vec3::new(0.058_471_68, 0.0, -12.0));
    let spawn_pos = WorldPosition::from_global(
        Vec3::new(exterior_spawn.x, 0.0, exterior_spawn.z),
        world.layout(),
    );

    let unit_catalog = UnitCatalog::default();
    let weapon_catalog = WeaponCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let nav_config = NavigationConfig::default();
    let unit_id = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        spawn_pos,
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .expect("spawn")
    .id;
    world
        .set_unit_current_space(unit_id, SpaceId::SURFACE)
        .expect("force surface");
    world
        .update_unit_position(unit_id, start)
        .expect("place inside projection");

    issue_unit_order(
        &mut world,
        &unit_catalog,
        &weapon_catalog,
        &doodad_catalog,
        &nav_config,
        unit_id,
        crate::world::UnitOrder::MoveTo { target: goal },
        AttackTargetingPolicy::default(),
    )
    .expect("issue");
    let resolve_report =
        resolve_pending_unit_orders(&mut world, &unit_catalog, catalogs, &nav_config);
    assert_eq!(
        resolve_report.resolved, 1,
        "surface path failures: {:?}",
        resolve_report.failures
    );
    let command = world
        .movement_authority_trace()
        .latest_command_for_unit(unit_id)
        .expect("command trace");
    assert_eq!(command.start_space, SpaceId::SURFACE);
    assert_eq!(command.goal_space, SpaceId::SURFACE);
    let UnitState::Moving { path, .. } = &world.get_unit(unit_id).unwrap().state else {
        panic!("unit must be moving after path resolution");
    };
    assert!(
        !path
            .waypoints
            .iter()
            .any(|waypoint| waypoint.portal_id.is_some()),
        "surface-only route must not include portal transition waypoints"
    );

    for _ in 0..800 {
        step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        assert_eq!(
            record.current_space_id,
            SpaceId::SURFACE,
            "surface movement must not infer interior membership"
        );
        assert!(
            (record.placement.position.to_global(layout).y - floor_y).abs() > 0.25,
            "surface movement must not snap Y to interior floor"
        );
        if matches!(record.state, UnitState::Idle) {
            break;
        }
    }
    assert_eq!(
        world.get_unit(unit_id).unwrap().current_space_id,
        SpaceId::SURFACE
    );
}

#[test]
fn portal_transition_mechanism_independent_of_position_update_membership() {
    use crate::world::{
        PortalId, PortalType, SpaceRecord, UnitPortalTransitionState, try_portal_transition,
    };

    let mut world = layout_world();
    let interior = world.space_registry_mut().allocate_space_id();
    world.space_registry_mut().insert_space(SpaceRecord {
        id: interior,
        owning_building_id: None,
        display_floor_label: "Ground".into(),
        visibility_group_id: 1,
        reference_elevation: 0.0,
        floor_y_global: 1.1,
        room_tag: None,
        enabled: true,
        walkable: true,
    });
    let portal = crate::world::PortalRecord {
        id: PortalId::new(1),
        portal_type: PortalType::ExteriorEntrance,
        from_space: SpaceId::SURFACE,
        to_space: interior,
        from_center_global_xz: Vec2::new(10.0, 10.0),
        from_radius_meters: 2.0,
        to_position: pos(10.0, 10.0),
        traversal_cost: 1.0,
        bidirectional: true,
        enabled: true,
        owning_building_id: None,
        entrance_threshold_global_xz: None,
        entrance_owning_edge_index: None,
    };
    world.space_registry_mut().insert_portal(portal.clone());

    let layout = world.layout();
    let agent = pos(10.0, 10.0);
    let mut state = UnitPortalTransitionState::default();
    let transition = try_portal_transition(
        &world,
        world.space_registry(),
        layout,
        SpaceId::SURFACE,
        agent,
        &mut state,
        Some(portal.id),
    )
    .expect("portal trigger must still transition when agent is inside disc");
    assert_eq!(transition.0, interior);
    assert_eq!(transition.2, portal.id);
}

#[test]
fn post_hydration_initializes_surface_units_inside_region() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);
    let region_space = real_hut_region_space(&world, building_id);
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let interior_xz = runtime
        .model_transform
        .transform_point(Vec3::new(0.0, 0.0, 1.0));
    let spawn_pos =
        WorldPosition::from_global(Vec3::new(interior_xz.x, 0.0, interior_xz.z), world.layout());

    let unit_catalog = UnitCatalog::default();
    let mut record = crate::world::UnitRecord::new(
        crate::world::UnitId::new(99),
        UnitDefinitionId::new("wolf"),
        crate::world::UnitPlacement::new(spawn_pos, Quat::IDENTITY),
        UnitSource::Authored,
        UnitOwnership::player_default(),
        5,
    );
    world
        .insert_unit(crate::world::ChunkId::new(ChunkCoord::new(0, 0)), record)
        .expect("insert without membership init");
    let unit_id = crate::world::UnitId::new(99);
    assert_eq!(
        world.get_unit(unit_id).unwrap().current_space_id,
        SpaceId::SURFACE
    );

    assert!(initialize_unit_navigation_membership(&mut world, unit_id));
    assert_eq!(
        world.get_unit(unit_id).unwrap().current_space_id,
        region_space
    );
}
