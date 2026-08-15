//! IN-11b: the real Survival Hut must activate runtime navigation with no InteriorProfile.
//!
//! The IN-11 ladder passed while the game failed because its activation helper used the
//! starter `hut` definition, which declares `interior_profile_id`. These tests use the
//! definition shape actually produced by the Excel import (no interior profile, no
//! explicit blueprint id) and the real persisted `hut_nav` catalog entry, so the
//! zero-runtime-spaces defect cannot recur unnoticed.

use std::path::Path;

use bevy::prelude::*;

use super::catalog::BuildingNavigationBlueprintCatalog;
use super::definition::{
    BuildingNavigationBlueprint, NavigationEntranceDefinition, NavigationFloorDefinition,
    NavigationPolygon2d, NavigationRegionDefinition,
};
use super::id::blueprint_id_for_building;
use super::resolve::resolve_building_navigation_blueprint;
use super::source::{BlueprintAuthoritySource, classify_blueprint_authority};
use crate::world::interaction::{
    InteractionQueryContext, InteractionType, query_world_interaction,
};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingCategoryCatalog, BuildingDefinition,
    BuildingDefinitionId, BuildingId, BuildingInteractionProfileCatalog, BuildingLifecycleState,
    BuildingNavigationBlueprintInstanceOverride, BuildingOwnership, ChunkCoord, ChunkLayout,
    DoodadCatalog, FootprintCatalog, InteriorActivationStatus, InteriorProfileCatalog,
    OccupancyCatalogs, PortalType, SpaceId, WeaponCatalog, WorldData, WorldPosition,
    place_player_building, set_building_lifecycle_stage,
};

const REAL_BUILDING_CATALOG_RON: &str = "assets/buildings/catalog.ron";

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

/// Survival Hut definition matching the real exported catalog row.
///
/// `interior_profile_id` and `navigation_blueprint_id` are both absent, exactly as the
/// Excel importer produces. `real_exported_hut_has_no_interior_profile` guards the
/// premise against asset drift.
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

/// Faithful copy of the persisted `hut_nav` entry: one floor, one four-vertex region,
/// one doorless entrance, floor elevation ~1.269 m.
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

/// Catalog holding only the persisted hut blueprint, keyed by the generated id the
/// runtime resolver actually looks up.
fn persisted_nav_catalog() -> BuildingNavigationBlueprintCatalog {
    BuildingNavigationBlueprintCatalog::from_definitions(vec![persisted_hut_nav_blueprint()])
        .expect("persisted hut_nav catalog")
}

fn activate_imported_hut(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
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
        pos(80.0, 80.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .expect("place hut")
    .id;

    // Real activation entry, not a test-only shortcut.
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

/// Guard the premise: the real exported catalog row really has no interior profile.
#[test]
fn real_exported_hut_has_no_interior_profile() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(REAL_BUILDING_CATALOG_RON);
    let Ok(text) = std::fs::read_to_string(&path) else {
        // The exported catalog is a dev artifact; skip rather than fail on a clean tree.
        return;
    };
    let catalog: crate::data_import::BuildingCatalogRon =
        ron::from_str(&text).expect("real building catalog RON parses");
    let hut = catalog
        .definitions
        .iter()
        .find(|definition| definition.id == "hut")
        .expect("real catalog contains hut");

    assert_eq!(hut.display_name, "Survival Hut");
    assert!(
        hut.interior_profile_id.is_none(),
        "premise of IN-11b: the imported hut has no interior profile"
    );
    assert!(
        hut.navigation_blueprint_id.is_none(),
        "premise of IN-11b: the imported hut names no blueprint, so resolution is by generated id"
    );
}

#[test]
fn imported_hut_resolves_persisted_blueprint_by_generated_id() {
    let definition = imported_survival_hut_definition();
    assert_eq!(blueprint_id_for_building(&definition).as_str(), "hut_nav");

    let nav_catalog = persisted_nav_catalog();
    let resolved = resolve_building_navigation_blueprint(&definition, &nav_catalog, None)
        .expect("resolution must not error")
        .expect("persisted hut_nav must resolve without an interior profile");
    assert_eq!(resolved.blueprint().id.as_str(), "hut_nav");
    assert_eq!(resolved.blueprint().floors.len(), 1);
    assert_eq!(resolved.blueprint().floors[0].regions.len(), 1);
    assert_eq!(resolved.blueprint().entrances.len(), 1);
    assert_eq!(
        classify_blueprint_authority(&definition, &nav_catalog, None),
        BlueprintAuthoritySource::Generated
    );
}

#[test]
fn real_hut_activation_creates_runtime_space_and_enabled_entrance_portal() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);

    let record = world.get_building(building_id).expect("building");
    assert!(
        record.interior.activated,
        "activation must not return early for a profile-less building"
    );

    let outcome = world
        .interior_activation_outcomes()
        .get(building_id)
        .expect("activation outcome must be recorded");
    assert_eq!(
        outcome.status,
        InteriorActivationStatus::NavigationWithoutProfile,
        "expected navigation active without profile, got {}",
        outcome.summary()
    );
    assert_eq!(outcome.runtime_floor_count, 1, "{}", outcome.summary());
    assert_eq!(outcome.runtime_region_count, 1, "{}", outcome.summary());
    assert_eq!(outcome.runtime_portal_count, 1, "{}", outcome.summary());

    // Runtime store is the source the navigation overlay reads.
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime navigation must exist");
    assert_eq!(runtime.blueprint_id.as_str(), "hut_nav");
    assert_eq!(runtime.floors.len(), 1);
    assert_eq!(runtime.regions.len(), 1);
    assert_eq!(runtime.regions[0].region_key, "region_3");
    assert_eq!(runtime.regions[0].world_outline_xz.len(), 4);

    let portal_id = *runtime
        .portal_keys
        .get("entrance")
        .expect("entrance portal must be registered");
    let portal = world
        .space_registry()
        .get_portal(portal_id)
        .expect("portal record");
    assert_eq!(portal.portal_type, PortalType::ExteriorEntrance);
    assert_eq!(portal.from_space, SpaceId::SURFACE);
    assert_eq!(portal.to_space, runtime.regions[0].space_id);
    assert!(
        portal.enabled,
        "a doorless entrance must stay enabled when no profile door exists"
    );
    assert!(
        world.door_store().building_door_ids(building_id).is_empty(),
        "no profile means no doors"
    );
}

#[test]
fn real_hut_interior_click_resolves_to_runtime_region() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);

    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let region_space = runtime.regions[0].space_id;
    let floor_y = world
        .space_registry()
        .get_space(region_space)
        .expect("space")
        .floor_y_global;
    // Interior point well inside the authored region polygon.
    let interior_global = runtime
        .model_transform
        .transform_point(Vec3::new(0.0, 0.0, 1.0));
    let interior_click = WorldPosition::from_global(
        Vec3::new(interior_global.x, floor_y, interior_global.z),
        world.layout(),
    );

    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let interaction_catalog = BuildingInteractionProfileCatalog::default();
    let unit_catalog = crate::world::UnitCatalog::default();
    let weapon_catalog = WeaponCatalog::default();
    let ctx = InteractionQueryContext::new(
        &world,
        &doodad_catalog,
        &building_catalog,
        &footprint,
        &interaction_catalog,
        &unit_catalog,
        &weapon_catalog,
    );
    let interaction = query_world_interaction(&ctx, interior_click).expect("interaction");
    assert_eq!(
        interaction.interaction_type,
        InteractionType::MoveTarget,
        "interior click must be a move target once runtime navigation exists"
    );
    assert!(interaction.valid);
}

/// The real authored geometry routes surface to interior over flat ground.
///
/// On level terrain the authored entrance is reachable and the route traverses the entrance
/// portal, so any remaining in-game approach failure is terrain- or occupancy-specific
/// (IN-11c) rather than a missing runtime space or portal. The `Err` arm proves that
/// distinction instead of accepting silent failure.
#[test]
fn real_hut_cross_space_route_traverses_entrance_portal() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);

    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime")
        .clone();
    let goal_space = runtime.regions[0].space_id;
    let floor_y = world
        .space_registry()
        .get_space(goal_space)
        .expect("space")
        .floor_y_global;
    let interior_global = runtime
        .model_transform
        .transform_point(Vec3::new(0.0, 0.0, 1.0));
    let goal = WorldPosition::from_global(
        Vec3::new(interior_global.x, floor_y, interior_global.z),
        world.layout(),
    );
    // Approach from outside the footprint on the entrance side.
    let entrance_global =
        runtime
            .model_transform
            .transform_point(Vec3::new(0.058_471_68, 0.0, -6.0));
    let start = WorldPosition::from_global(
        Vec3::new(entrance_global.x, 0.0, entrance_global.z),
        world.layout(),
    );

    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = crate::world::PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let path = crate::world::find_path_with_spaces(
        &world,
        catalogs,
        &crate::world::NavigationConfig::default(),
        0.68,
        45.0,
        start,
        goal,
        SpaceId::SURFACE,
        goal_space,
        Some(crate::world::UnitOwnership::player_default()),
    )
    .expect("cross-space route must succeed after clearance-safe landing resolution");
    assert!(
        path.waypoints
            .iter()
            .any(|waypoint| waypoint.portal_id.is_some()),
        "a surface-to-interior route must traverse the entrance portal"
    );
    assert!(
        path.waypoints
            .iter()
            .any(|wp| wp.portal_interior_destination.is_some()),
        "portal waypoint must include clearance-safe interior destination"
    );
    assert_eq!(
        path.waypoints.last().expect("waypoints").space_id,
        goal_space,
        "route must end in the authored interior region"
    );
}

/// Robot as the Excel importer produces it: 0.6 m collision radius, 9 m/s, `robot` mesh.
fn imported_robot_catalog() -> crate::world::UnitCatalog {
    crate::world::UnitCatalog::from_definitions(vec![crate::world::UnitDefinition::new(
        crate::world::UnitDefinitionId::new("robot"),
        "Robot",
        "Player",
        1,
        100,
        100,
        5,
        5,
        5,
        5,
        5,
        5,
        10.0,
        "Common",
        9.0,
        0.6,
        45.0,
        crate::world::WeaponDefinitionId::new("weapon_fists"),
        true,
        crate::world::UnitRenderKey::reserved("robot"),
    )])
    .expect("robot catalog")
}

/// IN-11c: entering the real hut must leave the robot present, correctly placed, and moving.
///
/// The disappearance was a presentation mapping fault, so this asserts both the
/// authoritative landing and the render translation the unit sync system would apply
/// under the live dev vertical exaggeration.
#[test]
fn real_hut_entry_keeps_the_robot_present_on_the_interior_floor() {
    // Live dev preview exaggeration: 3 render units spread over the authored height span
    // of chunk (0,0). Any interior offset multiplied by this throws the unit off-world.
    const DEV_VERTICAL_SCALE: f32 = 18_336.0;

    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let unit_catalog = imported_robot_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);

    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime")
        .clone();
    let goal_space = runtime.regions[0].space_id;
    let floor_y = world
        .space_registry()
        .get_space(goal_space)
        .expect("space")
        .floor_y_global;
    let goal = WorldPosition::from_global(
        {
            let interior = runtime
                .model_transform
                .transform_point(Vec3::new(0.0, 0.0, 1.0));
            Vec3::new(interior.x, floor_y, interior.z)
        },
        world.layout(),
    );
    // Start immediately outside the real doorway to isolate post-transition behavior from
    // the known exterior approach defects (IN-11d).
    let start = WorldPosition::from_global(
        {
            let outside =
                runtime
                    .model_transform
                    .transform_point(Vec3::new(0.058_471_68, 0.0, -6.0));
            Vec3::new(outside.x, 0.0, outside.z)
        },
        world.layout(),
    );

    let unit_id = crate::world::create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &crate::world::UnitDefinitionId::new("robot"),
        start,
        crate::world::UnitSource::Authored,
        crate::world::UnitOwnership::player_default(),
    )
    .expect("spawn robot")
    .id;

    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = crate::world::PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let nav_config = crate::world::NavigationConfig::default();

    world
        .command_buffer_mut()
        .enqueue(unit_id, crate::world::UnitOrder::MoveTo { target: goal });
    assert_eq!(
        crate::world::resolve_pending_unit_orders(&mut world, &unit_catalog, catalogs, &nav_config)
            .resolved,
        1,
        "a surface-to-interior order must resolve for the real hut"
    );

    let layout = world.layout();
    let mut arrived = false;
    for _ in 0..400 {
        let _ =
            crate::world::step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).expect("robot must survive entry");
        if record.current_space_id == goal_space
            && matches!(record.state, crate::world::UnitState::Idle)
        {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "robot must reach the commanded interior target");

    let record = world.get_unit(unit_id).expect("robot record survives");
    assert_eq!(record.id, unit_id, "entry must not respawn the unit");
    assert_eq!(record.current_space_id, goal_space);
    let global = record.placement.position.to_global(layout);
    assert!(global.is_finite(), "landing position must be finite");
    assert!(
        (global.y - floor_y).abs() < 0.01,
        "robot must stand on the interior floor plane, got y={} floor={floor_y}",
        global.y
    );
    assert!(
        global.xz().distance(goal.to_global(layout).xz()) < 1.0,
        "robot must continue past the portal to the commanded target"
    );

    // Exactly one traversal: no oscillation back through the entrance.
    let trace = world.portal_transition_trace();
    assert_eq!(
        trace.count_for_unit(unit_id),
        1,
        "entry must record exactly one space transition"
    );
    let event = trace.latest_for_unit(unit_id).expect("traced transition");
    assert_eq!(event.from_space, SpaceId::SURFACE);
    assert_eq!(event.to_space, goal_space);
    assert_eq!(event.destination_floor_y, Some(floor_y));

    // Presentation: the render translation must sit on the visible floor, which is the
    // building's render anchor plus the authored metric floor offset — not the offset
    // multiplied by the terrain exaggeration.
    let anchor_y = world
        .get_building(building_id)
        .expect("building")
        .placement
        .position
        .to_global(layout)
        .y;
    let render = crate::units::unit_render_translation(&world, record, layout, DEV_VERTICAL_SCALE);
    let expected_floor_render_y =
        crate::terrain::render_height(anchor_y, DEV_VERTICAL_SCALE) + (floor_y - anchor_y);
    assert!(
        (render.y - expected_floor_render_y).abs() < 0.01,
        "robot must render on the visible interior floor, got {} expected {}",
        render.y,
        expected_floor_render_y
    );
    assert!(
        (render.y - crate::terrain::render_height(global.y, DEV_VERTICAL_SCALE)).abs() > 1000.0,
        "guard: the old terrain-exaggerated mapping would place the robot far off-world"
    );

    // A second interior command must still be accepted and completed.
    let second_goal = WorldPosition::from_global(
        {
            let interior = runtime
                .model_transform
                .transform_point(Vec3::new(2.0, 0.0, 2.0));
            Vec3::new(interior.x, floor_y, interior.z)
        },
        world.layout(),
    );
    world.command_buffer_mut().enqueue(
        unit_id,
        crate::world::UnitOrder::MoveTo {
            target: second_goal,
        },
    );
    assert_eq!(
        crate::world::resolve_pending_unit_orders(&mut world, &unit_catalog, catalogs, &nav_config)
            .resolved,
        1,
        "interior-to-interior orders must resolve after entry"
    );
    for _ in 0..200 {
        let _ =
            crate::world::step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        if matches!(
            world.get_unit(unit_id).expect("robot").state,
            crate::world::UnitState::Idle
        ) {
            break;
        }
    }
    let record = world.get_unit(unit_id).expect("robot");
    assert_eq!(
        record.current_space_id, goal_space,
        "interior movement must not reclassify the robot as surface"
    );
    assert!(
        (record.placement.position.to_global(layout).y - floor_y).abs() < 0.01,
        "interior movement must not re-ground the robot to terrain"
    );

    // Exit through the same entrance and stay on the surface.
    world
        .command_buffer_mut()
        .enqueue(unit_id, crate::world::UnitOrder::MoveTo { target: start });
    let _ =
        crate::world::resolve_pending_unit_orders(&mut world, &unit_catalog, catalogs, &nav_config);
    let mut exited = false;
    for _ in 0..400 {
        let _ =
            crate::world::step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        if world
            .get_unit(unit_id)
            .expect("robot must survive exit")
            .current_space_id
            .is_surface()
        {
            exited = true;
            break;
        }
    }
    assert!(exited, "robot must be able to leave through the entrance");
    let record = world.get_unit(unit_id).expect("robot");
    let exit_render =
        crate::units::unit_render_translation(&world, record, layout, DEV_VERTICAL_SCALE);
    assert!(
        (exit_render.y
            - crate::terrain::render_height(
                record.placement.position.to_global(layout).y,
                DEV_VERTICAL_SCALE
            ))
        .abs()
            < 1e-3,
        "surface units must keep the plain terrain-exaggerated mapping"
    );
}

/// IN-11gI-E: bidirectional Entrance traversal through real movement, not manual SpaceIds.
#[test]
fn real_hut_bidirectional_entrance_traversal_via_movement() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let unit_catalog = imported_robot_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime")
        .clone();
    let interior_space = runtime.regions[0].space_id;
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .expect("space")
        .floor_y_global;
    let interior_goal = WorldPosition::from_global(
        {
            let p = runtime
                .model_transform
                .transform_point(Vec3::new(0.0, 0.0, 1.0));
            Vec3::new(p.x, floor_y, p.z)
        },
        world.layout(),
    );
    let surface_start = WorldPosition::from_global(
        {
            let p = runtime
                .model_transform
                .transform_point(Vec3::new(0.058_471_68, 0.0, -6.0));
            Vec3::new(p.x, 0.0, p.z)
        },
        world.layout(),
    );

    let unit_id = crate::world::create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &crate::world::UnitDefinitionId::new("robot"),
        surface_start,
        crate::world::UnitSource::Authored,
        crate::world::UnitOwnership::player_default(),
    )
    .expect("spawn robot")
    .id;

    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = crate::world::PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let nav_config = crate::world::NavigationConfig::default();
    let layout = world.layout();

    world.command_buffer_mut().enqueue(
        unit_id,
        crate::world::UnitOrder::MoveTo {
            target: interior_goal,
        },
    );
    assert_eq!(
        crate::world::resolve_pending_unit_orders(&mut world, &unit_catalog, catalogs, &nav_config)
            .resolved,
        1
    );
    for _ in 0..400 {
        let _ =
            crate::world::step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        if world.get_unit(unit_id).expect("robot").current_space_id == interior_space {
            break;
        }
    }
    assert_eq!(
        world.get_unit(unit_id).expect("robot").current_space_id,
        interior_space
    );
    // One movement step after entry must not bounce back to Surface.
    let _ = crate::world::step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
    assert_eq!(
        world.get_unit(unit_id).expect("robot").current_space_id,
        interior_space,
        "no immediate portal bounce after entry"
    );

    world.command_buffer_mut().enqueue(
        unit_id,
        crate::world::UnitOrder::MoveTo {
            target: surface_start,
        },
    );
    let _ =
        crate::world::resolve_pending_unit_orders(&mut world, &unit_catalog, catalogs, &nav_config);
    let mut exited = false;
    for _ in 0..400 {
        let _ =
            crate::world::step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        if world
            .get_unit(unit_id)
            .expect("robot")
            .current_space_id
            .is_surface()
        {
            exited = true;
            break;
        }
    }
    assert!(exited, "Interior → Surface exit must succeed");
    let record = world.get_unit(unit_id).expect("robot");
    assert!(record.current_space_id.is_surface());
    assert!(
        (record.placement.position.to_global(layout).y - 0.0).abs() < 0.05,
        "exit must ground to Surface terrain Y"
    );

    world.command_buffer_mut().enqueue(
        unit_id,
        crate::world::UnitOrder::MoveTo {
            target: interior_goal,
        },
    );
    let _ =
        crate::world::resolve_pending_unit_orders(&mut world, &unit_catalog, catalogs, &nav_config);
    for _ in 0..400 {
        let _ =
            crate::world::step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        if world.get_unit(unit_id).expect("robot").current_space_id == interior_space {
            break;
        }
    }
    assert_eq!(
        world.get_unit(unit_id).expect("robot").current_space_id,
        interior_space,
        "same Entrance must allow re-entry"
    );
    assert_eq!(
        world.portal_transition_trace().count_for_unit(unit_id),
        3,
        "entry, exit, and re-entry portal transitions expected"
    );
}

/// IN-11gI-E: Surface ↔ Interior ↔ Surface through the same Entrance without permanent lockout.
#[test]
fn real_hut_same_entrance_repeated_surface_interior_cycles() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let unit_catalog = imported_robot_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime")
        .clone();
    let interior_space = runtime.regions[0].space_id;
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .expect("space")
        .floor_y_global;
    let interior_goal = WorldPosition::from_global(
        {
            let p = runtime
                .model_transform
                .transform_point(Vec3::new(0.0, 0.0, 1.0));
            Vec3::new(p.x, floor_y, p.z)
        },
        world.layout(),
    );
    let surface_start = WorldPosition::from_global(
        {
            let p = runtime
                .model_transform
                .transform_point(Vec3::new(0.058_471_68, 0.0, -6.0));
            Vec3::new(p.x, 0.0, p.z)
        },
        world.layout(),
    );

    let unit_id = crate::world::create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &crate::world::UnitDefinitionId::new("robot"),
        surface_start,
        crate::world::UnitSource::Authored,
        crate::world::UnitOwnership::player_default(),
    )
    .expect("spawn robot")
    .id;

    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = crate::world::PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let nav_config = crate::world::NavigationConfig::default();

    let targets = [interior_goal, surface_start, interior_goal, surface_start];
    let expected_spaces = [
        interior_space,
        SpaceId::SURFACE,
        interior_space,
        SpaceId::SURFACE,
    ];

    for (target, expected_space) in targets.iter().zip(expected_spaces.iter()) {
        world
            .command_buffer_mut()
            .enqueue(unit_id, crate::world::UnitOrder::MoveTo { target: *target });
        let _ = crate::world::resolve_pending_unit_orders(
            &mut world,
            &unit_catalog,
            catalogs,
            &nav_config,
        );
        for _ in 0..400 {
            let _ = crate::world::step_unit_movement(
                &mut world,
                &unit_catalog,
                catalogs,
                unit_id,
                0.25,
            );
            if world.get_unit(unit_id).expect("robot").current_space_id == *expected_space {
                break;
            }
        }
        assert_eq!(
            world.get_unit(unit_id).expect("robot").current_space_id,
            *expected_space
        );
    }
    assert_eq!(
        world.portal_transition_trace().count_for_unit(unit_id),
        4,
        "four portal transitions across two full exit/entry cycles"
    );
}

/// Instance override on a profile-less definition must resolve and activate too.
#[test]
fn instance_override_activates_without_profile() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog =
        BuildingNavigationBlueprintCatalog::from_definitions(Vec::new()).expect("empty catalog");
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = OccupancyCatalogs {
        building: &building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();

    let building_id = place_player_building(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(80.0, 80.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .expect("place hut")
    .id;
    world
        .mutate_building(building_id, |record| {
            record.interior.navigation_blueprint_override = Some(
                BuildingNavigationBlueprintInstanceOverride::inline(persisted_hut_nav_blueprint()),
            );
        })
        .expect("building");

    set_building_lifecycle_stage(
        &mut world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        Some(&nav_catalog),
        building_id,
        BuildingLifecycleState::Complete,
        1.0,
    )
    .expect("complete hut");

    let outcome = world
        .interior_activation_outcomes()
        .get(building_id)
        .expect("outcome");
    assert_eq!(
        outcome.status,
        InteriorActivationStatus::NavigationWithoutProfile
    );
    assert_eq!(
        outcome.blueprint_authority,
        BlueprintAuthoritySource::InstanceOverride
    );
    assert_eq!(outcome.runtime_region_count, 1);
}

/// IN-11f: dev complete spawn without activation must rehydrate from the catalog alone.
#[test]
fn cold_load_reconcile_activates_persisted_hut_without_editor() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = OccupancyCatalogs {
        building: &building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();

    let building_id = place_player_building(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(80.0, 80.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .expect("place hut")
    .id;
    world
        .mutate_building(building_id, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        })
        .expect("building");

    assert!(
        world
            .building_navigation_runtime()
            .get(building_id)
            .is_none(),
        "precondition: no runtime before reconcile"
    );

    crate::world::reconcile_building_navigation_runtime(
        &mut world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        &nav_catalog,
        building_id,
        false,
    )
    .expect("reconcile");

    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime after cold reconcile");
    assert_eq!(runtime.regions.len(), 1);
    assert_eq!(runtime.portal_keys.len(), 1);
    assert!(world.get_building(building_id).unwrap().interior.activated);
}

/// IN-11f: worker construction completion must pass the navigation catalog through.
#[test]
fn construction_labor_completion_activates_with_nav_catalog() {
    use crate::world::{
        add_building_construction_progress, blueprint_topology_fingerprint,
        runtime_topology_fingerprint,
    };

    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = OccupancyCatalogs {
        building: &building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();

    let building_id = place_player_building(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(80.0, 80.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .expect("place hut")
    .id;

    add_building_construction_progress(
        &mut world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        Some(&nav_catalog),
        building_id,
        1.0,
    )
    .expect("complete via labor");

    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime after labor completion");
    let blueprint = nav_catalog.get(&runtime.blueprint_id).expect("blueprint");
    assert_eq!(
        runtime_topology_fingerprint(runtime),
        blueprint_topology_fingerprint(blueprint)
    );
    assert_eq!(runtime.portal_keys.len(), 1);
}

/// IN-11f: unchanged Save/Apply must preserve runtime topology fingerprint.
#[test]
fn noop_save_apply_preserves_runtime_topology() {
    use super::persistence::{InteriorActivationCatalogs, apply_blueprint_to_asset};
    use crate::world::{
        BuildingNavigationBlueprintCatalogRevision, blueprint_topology_fingerprint,
        runtime_topology_fingerprint,
    };

    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let mut nav_catalog = persisted_nav_catalog();
    let mut nav_revision = BuildingNavigationBlueprintCatalogRevision(0);
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = OccupancyCatalogs {
        building: &building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);

    let before = runtime_topology_fingerprint(
        world
            .building_navigation_runtime()
            .get(building_id)
            .expect("runtime"),
    );
    let blueprint_id = before.blueprint_id.clone();
    let blueprint = nav_catalog
        .get(&blueprint_id)
        .expect("catalog blueprint")
        .clone();

    let expected = blueprint_topology_fingerprint(&blueprint);

    apply_blueprint_to_asset(
        &mut world,
        &building_catalog,
        InteriorActivationCatalogs {
            interior: &interior,
            doodad: &doodad_catalog,
            footprint: &footprint,
        },
        &mut nav_catalog,
        &mut nav_revision,
        &BuildingDefinitionId::new("hut"),
        blueprint,
    )
    .expect("noop apply");

    let after = runtime_topology_fingerprint(
        world
            .building_navigation_runtime()
            .get(building_id)
            .expect("runtime after apply"),
    );
    assert_eq!(before, after);
    assert_eq!(after, expected);
}

/// IN-11gE: persisted activated flag must not suppress hydration when runtime is empty.
#[test]
fn activated_flag_with_empty_runtime_rehydrates_on_cold_load() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = OccupancyCatalogs {
        building: &building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();

    let building_id = place_player_building(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(80.0, 80.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .expect("place hut")
    .id;
    world
        .mutate_building(building_id, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
            record.interior.activated = true;
        })
        .expect("building");

    assert!(
        world
            .building_navigation_runtime()
            .get(building_id)
            .is_none(),
        "precondition: stale activated flag without runtime store entry"
    );
    assert!(
        world
            .space_registry()
            .building_space_ids(building_id)
            .is_empty(),
        "precondition: no hydrated spaces in registry"
    );

    let outcome = crate::world::reconcile_building_navigation_runtime(
        &mut world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        &nav_catalog,
        building_id,
        false,
    )
    .expect("reconcile");
    assert_ne!(
        outcome,
        crate::world::NavigationReconcileOutcome::NotNeeded,
        "empty runtime must not early-return as NotNeeded"
    );
    assert!(
        world
            .building_navigation_runtime()
            .get(building_id)
            .is_some(),
        "cold-load reconcile must hydrate runtime store"
    );
    assert!(
        !world
            .space_registry()
            .building_space_ids(building_id)
            .is_empty(),
        "cold-load reconcile must register spaces"
    );
}

/// IN-11gE: cold-load reconciliation and no-op Save/Apply must produce equivalent topology.
#[test]
fn cold_load_matches_noop_save_apply_topology() {
    use super::persistence::{InteriorActivationCatalogs, apply_blueprint_to_asset};
    use crate::world::{
        BuildingNavigationBlueprintCatalogRevision, capture_building_navigation_topology_snapshot,
    };

    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = OccupancyCatalogs {
        building: &building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();

    let mut world_cold = layout_world();
    let building_id_cold = place_player_building(
        &building_catalog,
        &mut world_cold,
        &BuildingDefinitionId::new("hut"),
        pos(80.0, 80.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .expect("place hut")
    .id;
    world_cold
        .mutate_building(building_id_cold, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
            record.interior.activated = true;
        })
        .expect("building");
    crate::world::reconcile_building_navigation_runtime(
        &mut world_cold,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        &nav_catalog,
        building_id_cold,
        false,
    )
    .expect("cold reconcile");
    let cold_snapshot =
        capture_building_navigation_topology_snapshot(&world_cold, building_id_cold)
            .expect("cold topology");

    let mut world_apply = layout_world();
    let building_id_apply = place_player_building(
        &building_catalog,
        &mut world_apply,
        &BuildingDefinitionId::new("hut"),
        pos(80.0, 80.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .expect("place hut")
    .id;
    world_apply
        .mutate_building(building_id_apply, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
            record.interior.activated = true;
        })
        .expect("building");

    let blueprint = nav_catalog
        .get(&cold_snapshot.fingerprint.blueprint_id)
        .expect("blueprint")
        .clone();
    let mut nav_revision = BuildingNavigationBlueprintCatalogRevision(0);
    apply_blueprint_to_asset(
        &mut world_apply,
        &building_catalog,
        InteriorActivationCatalogs {
            interior: &interior,
            doodad: &doodad_catalog,
            footprint: &footprint,
        },
        &mut nav_catalog.clone(),
        &mut nav_revision,
        &BuildingDefinitionId::new("hut"),
        blueprint,
    )
    .expect("noop apply");
    let apply_snapshot =
        capture_building_navigation_topology_snapshot(&world_apply, building_id_apply)
            .expect("apply topology");

    assert_eq!(cold_snapshot, apply_snapshot);
}

/// IN-11gE: repeated reconcile with unchanged blueprint must not duplicate topology.
#[test]
fn reconcile_is_idempotent_when_topology_hydrated() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = OccupancyCatalogs {
        building: &building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);

    let before = crate::world::capture_building_navigation_topology_snapshot(&world, building_id)
        .expect("topology before");
    let space_count_before = world.space_registry().building_space_ids(building_id).len();
    let portal_count_before = world
        .space_registry()
        .portals()
        .filter(|(_, portal)| portal.owning_building_id == Some(building_id))
        .count();

    let outcome = crate::world::reconcile_building_navigation_runtime(
        &mut world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        &nav_catalog,
        building_id,
        false,
    )
    .expect("reconcile");
    assert_eq!(outcome, crate::world::NavigationReconcileOutcome::NotNeeded);

    let after = crate::world::capture_building_navigation_topology_snapshot(&world, building_id)
        .expect("topology after");
    assert_eq!(before, after);
    assert_eq!(
        world.space_registry().building_space_ids(building_id).len(),
        space_count_before
    );
    assert_eq!(
        world
            .space_registry()
            .portals()
            .filter(|(_, portal)| portal.owning_building_id == Some(building_id))
            .count(),
        portal_count_before
    );
}

/// IN-11gE: when authoritative blueprint no longer resolves, derived runtime is cleared (ghost).
#[test]
fn missing_resolved_blueprint_clears_stale_runtime() {
    let mut world = layout_world();
    let building_catalog = imported_building_catalog();
    let nav_catalog = persisted_nav_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);
    assert!(
        world
            .building_navigation_runtime()
            .get(building_id)
            .is_some(),
        "precondition"
    );

    let empty_nav = BuildingNavigationBlueprintCatalog::from_definitions(Vec::new())
        .expect("empty nav catalog");
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = OccupancyCatalogs {
        building: &building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();

    crate::world::reconcile_building_navigation_runtime(
        &mut world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        &empty_nav,
        building_id,
        false,
    )
    .expect("reconcile without blueprint");

    assert!(
        world
            .building_navigation_runtime()
            .get(building_id)
            .is_none(),
        "stale runtime must be removed when blueprint does not resolve"
    );
    assert!(
        world
            .space_registry()
            .building_space_ids(building_id)
            .is_empty(),
        "stale spaces must be removed"
    );
}
