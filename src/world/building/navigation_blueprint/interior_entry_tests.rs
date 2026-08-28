//! IN-11 player-commanded interior entry end-to-end tests.

use bevy::prelude::*;

use super::adapt::region_space_key;
use super::fixtures::one_region_doorless_navigation_blueprint;
use super::runtime::{
    interior_navigation_move_target_at_position, resolve_navigation_space_at_position,
    resolve_navigation_start_space,
};
use crate::units::input::{SelectedUnits, issue_move_orders_to_selection};
use crate::world::interaction::{
    InteractionOrderPlan, InteractionQueryContext, InteractionResolveContext, InteractionType,
    query_world_interaction, resolve_interaction_to_order, resolve_world_click_to_order,
};
use crate::world::unit::{
    UnitOrder, UnitSource, UnitState, create_unit_with_ownership, step_unit_movement,
};
use crate::world::{
    Affiliation, AttackTargetingPolicy, BuildingCatalog, BuildingDefinitionId,
    BuildingInteractionProfileCatalog, BuildingLifecycleState, BuildingNavigationBlueprint,
    BuildingNavigationBlueprintCatalog, BuildingNavigationBlueprintInstanceOverride,
    BuildingOwnership, ChunkCoord, ChunkLayout, DoodadCatalog, FootprintCatalog,
    InteriorActivationStatus, ItemPileSettings, NavigationConfig, OccupancyCatalogs,
    PassabilityCatalogs, PortalType, SpaceId, UnitDefinitionId, WeaponCatalog, WorldData,
    WorldPosition, find_path_with_spaces, place_player_building, resolve_pending_unit_orders,
    set_building_lifecycle_stage, validate_blueprint_for_inspection,
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

fn occ<'a>(
    building: &'a BuildingCatalog,
    doodad: &'a DoodadCatalog,
    footprint: &'a FootprintCatalog,
) -> OccupancyCatalogs<'a> {
    OccupancyCatalogs {
        building,
        doodad,
        footprint,
    }
}

/// Hut-shaped definition with no interior profile and no explicit blueprint id.
///
/// This is the shape of the real imported Survival Hut. The original IN-11 ladder
/// used the starter `hut`, which declares `interior_profile_id`, so it activated
/// through a gate the real building could never pass (IN-11b).
pub(super) fn no_profile_building_catalog() -> BuildingCatalog {
    let categories = crate::world::BuildingCategoryCatalog::default();
    let definitions = crate::world::starter_building_definitions()
        .into_iter()
        .map(|mut definition| {
            if definition.id == BuildingDefinitionId::new("hut") {
                definition.interior_profile_id = None;
                definition.navigation_blueprint_id = None;
            }
            definition
        })
        .collect();
    BuildingCatalog::from_definitions(definitions, &categories).expect("no-profile catalog")
}

fn place_complete_hut(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    placement: WorldPosition,
    instance_override: Option<BuildingNavigationBlueprint>,
) -> crate::world::BuildingId {
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = occ(building_catalog, &doodad_catalog, &footprint);
    let interior = crate::world::InteriorProfileCatalog::default();

    let id = place_player_building(
        building_catalog,
        world,
        &BuildingDefinitionId::new("hut"),
        placement,
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .unwrap()
    .id;

    if let Some(blueprint) = instance_override {
        world
            .mutate_building(id, |record| {
                record.interior.navigation_blueprint_override = Some(
                    BuildingNavigationBlueprintInstanceOverride::inline(blueprint),
                );
            })
            .expect("building");
    }

    set_building_lifecycle_stage(
        world,
        building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        Some(nav_catalog),
        id,
        BuildingLifecycleState::Complete,
        1.0,
    )
    .unwrap();
    id
}

/// Activate via instance override on a definition with no interior profile.
fn activate_fixture(
    world: &mut WorldData,
    blueprint: BuildingNavigationBlueprint,
    placement: WorldPosition,
) -> crate::world::BuildingId {
    let building_catalog = no_profile_building_catalog();
    let nav_catalog = BuildingNavigationBlueprintCatalog::default();
    place_complete_hut(
        world,
        &building_catalog,
        &nav_catalog,
        placement,
        Some(blueprint),
    )
}

/// Activate via the asset catalog under the generated id, with no instance override.
///
/// This is the real Survival Hut authority path: the editor's "Apply to Asset"
/// writes `hut_nav` to the catalog and the definition names no blueprint.
fn activate_from_asset_catalog(
    world: &mut WorldData,
    mut blueprint: BuildingNavigationBlueprint,
    placement: WorldPosition,
) -> crate::world::BuildingId {
    let building_catalog = no_profile_building_catalog();
    let definition = building_catalog
        .get(&BuildingDefinitionId::new("hut"))
        .expect("hut definition");
    blueprint.id = crate::world::blueprint_id_for_building(definition);
    let mut nav_catalog = BuildingNavigationBlueprintCatalog::default();
    nav_catalog.upsert(blueprint).expect("upsert");
    place_complete_hut(world, &building_catalog, &nav_catalog, placement, None)
}

fn local_xz_to_world(
    world: &WorldData,
    building_id: crate::world::BuildingId,
    local_xz: Vec2,
    floor_y: f32,
) -> WorldPosition {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let layout = world.layout();
    let global = runtime
        .model_transform
        .transform_point(Vec3::new(local_xz.x, floor_y, local_xz.y));
    WorldPosition::from_global(global, layout)
}

fn region_space(
    world: &WorldData,
    building_id: crate::world::BuildingId,
    floor_key: &str,
    region_key: &str,
) -> SpaceId {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let key = region_space_key(floor_key, region_key);
    *runtime.space_keys.get(&key).unwrap_or_else(|| {
        panic!("missing space key `{key}`");
    })
}

fn exterior_approach(world: &WorldData, building_id: crate::world::BuildingId) -> WorldPosition {
    let layout = world.layout();
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let portal_id = *runtime
        .portal_keys
        .get("exterior_entrance")
        .expect("entrance portal");
    let portal = world.space_registry().get_portal(portal_id).unwrap();
    let approach = portal.from_center_global_xz + Vec2::new(3.0, 0.0);
    WorldPosition::from_global(Vec3::new(approach.x, 0.0, approach.y), layout)
}

/// IN-11b: blueprint present, profile absent — navigation must activate anyway.
#[test]
fn blueprint_without_profile_activates_navigation_with_enabled_doorless_entrance() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );

    let outcome = world
        .interior_activation_outcomes()
        .get(building_id)
        .expect("activation outcome recorded");
    assert_eq!(
        outcome.status,
        InteriorActivationStatus::NavigationWithoutProfile,
        "no profile must not prevent navigation activation"
    );
    assert_eq!(outcome.runtime_floor_count, 1);
    assert_eq!(outcome.runtime_region_count, 1);
    assert_eq!(outcome.runtime_portal_count, 1);
    assert!(outcome.profile_id.is_none());

    assert!(
        world.door_store().building_door_ids(building_id).is_empty(),
        "a doorless blueprint must not synthesize doors"
    );
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let portal_id = runtime.portal_keys["exterior_entrance"];
    assert!(
        world
            .space_registry()
            .get_portal(portal_id)
            .unwrap()
            .enabled
    );
}

/// IN-11b: the real hut authority path — asset catalog blueprint, no override, no profile.
#[test]
fn asset_catalog_blueprint_activates_without_profile_or_instance_override() {
    let mut world = layout_world();
    let building_id = activate_from_asset_catalog(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );

    let record = world.get_building(building_id).expect("building");
    assert!(
        record.interior.navigation_blueprint_override.is_none(),
        "must exercise the asset-catalog path, not an instance override"
    );
    assert!(record.interior.activated);

    let outcome = world
        .interior_activation_outcomes()
        .get(building_id)
        .expect("activation outcome");
    assert_eq!(
        outcome.status,
        InteriorActivationStatus::NavigationWithoutProfile
    );
    assert_eq!(
        outcome.blueprint_id.as_ref().map(|id| id.as_str()),
        Some("hut_nav")
    );
    assert_eq!(outcome.runtime_region_count, 1);
    assert_eq!(outcome.runtime_portal_count, 1);
}

/// IN-11b: blueprint plus profile — both activate, and the authored door binds.
#[test]
fn blueprint_and_profile_both_activate_with_door_controlled_entrance() {
    let mut world = layout_world();
    let building_catalog = BuildingCatalog::default();
    let nav_catalog = BuildingNavigationBlueprintCatalog::default();
    let building_id = place_complete_hut(
        &mut world,
        &building_catalog,
        &nav_catalog,
        pos(80.0, 80.0),
        None,
    );

    let outcome = world
        .interior_activation_outcomes()
        .get(building_id)
        .expect("activation outcome");
    assert_eq!(
        outcome.status,
        InteriorActivationStatus::NavigationAndProfile
    );
    assert!(outcome.runtime_region_count >= 1);
    assert!(
        !world.door_store().building_door_ids(building_id).is_empty(),
        "profile doors must still be created"
    );

    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let portal_id = runtime.portal_keys["exterior_entrance"];
    assert!(
        !world
            .space_registry()
            .get_portal(portal_id)
            .unwrap()
            .enabled,
        "profile-authored exterior door must still close its portal"
    );
}

/// IN-11b: no blueprint and no profile records an explicit skip, never silent success.
#[test]
fn no_blueprint_and_no_profile_records_explicit_skip() {
    let mut world = layout_world();
    let building_catalog = no_profile_building_catalog();
    let nav_catalog = BuildingNavigationBlueprintCatalog::default();
    let building_id = place_complete_hut(
        &mut world,
        &building_catalog,
        &nav_catalog,
        pos(80.0, 80.0),
        None,
    );

    let outcome = world
        .interior_activation_outcomes()
        .get(building_id)
        .expect("skip must be recorded, not silent");
    assert_eq!(
        outcome.status,
        InteriorActivationStatus::NoBlueprintNoProfile
    );
    assert_eq!(outcome.runtime_region_count, 0);
    assert!(
        !world.get_building(building_id).unwrap().interior.activated,
        "nothing was activated so the flag must stay false"
    );
}

/// IN-11b: a named-but-missing profile must not discard valid navigation topology.
#[test]
fn missing_profile_key_still_activates_navigation_with_warning() {
    let mut world = layout_world();
    let categories = crate::world::BuildingCategoryCatalog::default();
    let definitions = crate::world::starter_building_definitions()
        .into_iter()
        .map(|mut definition| {
            if definition.id == BuildingDefinitionId::new("hut") {
                definition.interior_profile_id = Some("absent_profile".to_string());
                definition.navigation_blueprint_id = None;
            }
            definition
        })
        .collect();
    let building_catalog =
        BuildingCatalog::from_definitions(definitions, &categories).expect("catalog");
    let nav_catalog = BuildingNavigationBlueprintCatalog::default();
    let building_id = place_complete_hut(
        &mut world,
        &building_catalog,
        &nav_catalog,
        pos(80.0, 80.0),
        Some(one_region_doorless_navigation_blueprint()),
    );

    let outcome = world
        .interior_activation_outcomes()
        .get(building_id)
        .expect("activation outcome");
    assert_eq!(
        outcome.status,
        InteriorActivationStatus::NavigationProfileMissing {
            profile_key: "absent_profile".to_string()
        },
        "missing presentation must be reported, not fatal"
    );
    assert_eq!(outcome.runtime_region_count, 1);
    assert_eq!(outcome.runtime_portal_count, 1);
}

/// IN-11b: profile-only buildings keep their existing behavior under the split flow.
#[test]
fn profile_only_building_preserves_legacy_activation() {
    let mut world = layout_world();
    let categories = crate::world::BuildingCategoryCatalog::default();
    let definitions = crate::world::starter_building_definitions()
        .into_iter()
        .map(|mut definition| {
            if definition.id == BuildingDefinitionId::new("hut") {
                definition.navigation_blueprint_id = None;
            }
            definition
        })
        .collect();
    let building_catalog =
        BuildingCatalog::from_definitions(definitions, &categories).expect("catalog");
    // Empty nav catalog: nothing can resolve, so only the profile activates.
    let nav_catalog = BuildingNavigationBlueprintCatalog::from_definitions(Vec::new())
        .expect("empty nav catalog");
    let building_id = place_complete_hut(
        &mut world,
        &building_catalog,
        &nav_catalog,
        pos(80.0, 80.0),
        None,
    );

    let outcome = world
        .interior_activation_outcomes()
        .get(building_id)
        .expect("activation outcome");
    assert_eq!(
        outcome.status,
        InteriorActivationStatus::ProfileWithoutNavigation
    );
    assert!(
        world
            .building_navigation_runtime()
            .get(building_id)
            .is_none(),
        "no blueprint means no blueprint runtime"
    );
    assert!(
        !world
            .space_registry()
            .building_space_ids(building_id)
            .is_empty(),
        "profile spaces must still be registered"
    );
}

#[test]
fn level1_one_region_blueprint_resolves_without_errors() {
    let blueprint = one_region_doorless_navigation_blueprint();
    let validation = validate_blueprint_for_inspection(&blueprint);
    assert!(validation.valid(), "{:?}", validation.diagnostics);
    assert_eq!(blueprint.floors.len(), 1);
    assert_eq!(blueprint.floors[0].regions.len(), 1);
    assert_eq!(blueprint.floors[0].regions[0].key, "main");
    assert_eq!(
        blueprint.floors[0].regions[0]
            .walkable_outline
            .vertices_xz
            .len(),
        4
    );
    assert_eq!(blueprint.entrances.len(), 1);
    assert_eq!(blueprint.entrances[0].region_key.as_deref(), Some("main"));
}

#[test]
fn level2_one_region_runtime_activation_registers_entrance_portal() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    assert_eq!(runtime.regions.len(), 1);
    assert_eq!(runtime.regions[0].region_key, "main");
    assert!(runtime.portal_keys.contains_key("exterior_entrance"));

    let portal_id = runtime.portal_keys["exterior_entrance"];
    let portal = world.space_registry().get_portal(portal_id).unwrap();
    assert_eq!(portal.portal_type, PortalType::ExteriorEntrance);
    assert_eq!(portal.from_space, SpaceId::SURFACE);
    assert_eq!(
        portal.to_space,
        region_space(&world, building_id, "ground", "main")
    );
    assert!(portal.enabled);
}

#[test]
fn level3_interior_click_resolves_to_move_target_not_blocked_area() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let main_space = region_space(&world, building_id, "ground", "main");
    let interior_click = local_xz_to_world(&world, building_id, Vec2::new(4.0, 3.5), 0.0);

    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let interaction_catalog = BuildingInteractionProfileCatalog::default();
    let unit_catalog = crate::world::UnitCatalog::default();
    let weapon_catalog = WeaponCatalog::default();
    let pile_settings = ItemPileSettings::default();
    let ctx = InteractionQueryContext::new(
        &world,
        &doodad_catalog,
        &building_catalog,
        &footprint,
        &interaction_catalog,
        &unit_catalog,
        &weapon_catalog,
        &pile_settings,
    );
    let interaction = query_world_interaction(&ctx, interior_click).expect("interaction");
    assert_eq!(interaction.interaction_type, InteractionType::MoveTarget);
    assert!(interaction.valid);

    let resolved_space = resolve_navigation_space_at_position(
        world.building_navigation_runtime(),
        world.space_registry(),
        world.layout(),
        interior_click,
    );
    assert_eq!(resolved_space, main_space);
    assert_eq!(
        resolve_navigation_start_space(
            world.building_navigation_runtime(),
            world.space_registry(),
            world.layout(),
            interior_click,
            SpaceId::SURFACE,
        ),
        SpaceId::SURFACE,
        "goal resolution must not mutate start-space tracking"
    );
}

#[test]
fn level4_surface_to_interior_path_uses_entrance_portal() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let goal_space = region_space(&world, building_id, "ground", "main");
    let start = exterior_approach(&world, building_id);
    let goal = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0), 0.0);

    let catalogs = PassabilityCatalogs {
        doodad: &DoodadCatalog::default(),
        building: &BuildingCatalog::default(),
        footprint: &FootprintCatalog::default(),
    };
    let path = find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        0.5,
        45.0,
        start,
        goal,
        SpaceId::SURFACE,
        goal_space,
        Some(crate::world::UnitOwnership::player_default()),
    )
    .expect("cross-space path");
    assert!(path.waypoints.iter().any(|wp| wp.portal_id.is_some()));
    let last = path.waypoints.last().unwrap();
    assert_eq!(last.space_id, goal_space);
    let goal_xz = goal.to_global(world.layout()).xz();
    let last_xz = last.position.to_global(world.layout()).xz();
    assert!(last_xz.distance(goal_xz) < 0.25);
}

#[test]
fn level5_robot_enters_one_region_hut_through_entrance() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let unit_catalog = crate::world::UnitCatalog::default();
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let goal_space = region_space(&world, building_id, "ground", "main");
    let start = exterior_approach(&world, building_id);
    let goal = local_xz_to_world(&world, building_id, Vec2::new(6.5, 4.5), 0.0);

    let unit_id = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        start,
        UnitSource::Authored,
        crate::world::UnitOwnership::player_default(),
    )
    .unwrap()
    .id;

    world
        .command_buffer_mut()
        .enqueue(unit_id, UnitOrder::MoveTo { target: goal });
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let report = resolve_pending_unit_orders(
        &mut world,
        &unit_catalog,
        catalogs,
        &NavigationConfig::default(),
    );
    assert_eq!(report.resolved, 1);

    let layout = world.layout();
    let goal_xz = goal.to_global(layout).xz();
    let mut entered = false;
    for _ in 0..400 {
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        if record.current_space_id == goal_space {
            entered = true;
            if let UnitState::Idle = record.state {
                let pos_xz = record.placement.position.to_global(layout).xz();
                assert!(pos_xz.distance(goal_xz) < 1.0);
                break;
            }
        }
    }
    assert!(
        entered,
        "robot should enter interior through entrance portal"
    );
}

#[test]
fn level6_player_command_path_issues_interior_move_order() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let unit_catalog = crate::world::UnitCatalog::default();
    let weapon_catalog = WeaponCatalog::default();
    let interaction_catalog = BuildingInteractionProfileCatalog::default();
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let goal_space = region_space(&world, building_id, "ground", "main");
    let start = exterior_approach(&world, building_id);
    let interior_click = local_xz_to_world(&world, building_id, Vec2::new(5.0, 4.0), 0.0);

    let unit_id = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        start,
        UnitSource::Authored,
        crate::world::UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    let mut selected = SelectedUnits::default();
    selected.set_single(unit_id);

    let selected_units = [unit_id];
    let pile_settings = ItemPileSettings::default();
    let authored_relationships = crate::world::AuthoredRelationshipCatalog::default();
    let resolve_ctx = InteractionResolveContext::new(
        &world,
        &doodad_catalog,
        &building_catalog,
        &footprint,
        &interaction_catalog,
        &unit_catalog,
        &weapon_catalog,
        &pile_settings,
        &authored_relationships,
        &selected_units,
    );
    let plan = resolve_world_click_to_order(&resolve_ctx, interior_click).expect("plan");
    assert!(
        matches!(plan, InteractionOrderPlan::MoveTo { .. }),
        "interior click must produce MoveTo, got {plan:?}"
    );
    let InteractionOrderPlan::MoveTo { target } = plan else {
        unreachable!();
    };

    let move_report = issue_move_orders_to_selection(
        &mut world,
        &selected,
        &unit_catalog,
        &weapon_catalog,
        &doodad_catalog,
        &NavigationConfig::default(),
        target,
        AttackTargetingPolicy::default(),
    );
    assert_eq!(move_report.issued, 1, "player move should be accepted");

    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let resolve_report = resolve_pending_unit_orders(
        &mut world,
        &unit_catalog,
        catalogs,
        &NavigationConfig::default(),
    );
    assert_eq!(
        resolve_report.resolved, 1,
        "player move path should resolve"
    );

    let layout = world.layout();
    let goal_xz = target.to_global(layout).xz();
    let mut entered = false;
    for _ in 0..400 {
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        if record.current_space_id == goal_space {
            entered = true;
            if matches!(record.state, UnitState::Idle) {
                let pos_xz = record.placement.position.to_global(layout).xz();
                assert!(pos_xz.distance(goal_xz) < 1.5);
                break;
            }
        }
    }
    assert!(
        entered,
        "player-commanded move should traverse entrance into {building_id:?}"
    );
}

#[test]
fn surface_unit_near_interior_goal_xz_still_requires_portal() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let unit_catalog = crate::world::UnitCatalog::default();
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let goal_space = region_space(&world, building_id, "ground", "main");
    let goal = local_xz_to_world(&world, building_id, Vec2::new(4.0, 3.0), 0.0);
    let layout = world.layout();
    let goal_xz = goal.to_global(layout).xz();
    let near_surface =
        WorldPosition::from_global(Vec3::new(goal_xz.x + 1.0, 0.0, goal_xz.y + 0.5), layout);

    let unit_id = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        near_surface,
        UnitSource::Authored,
        crate::world::UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    assert_eq!(
        world.get_unit(unit_id).unwrap().current_space_id,
        SpaceId::SURFACE
    );

    world
        .command_buffer_mut()
        .enqueue(unit_id, UnitOrder::MoveTo { target: goal });
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let report = resolve_pending_unit_orders(
        &mut world,
        &unit_catalog,
        catalogs,
        &NavigationConfig::default(),
    );
    assert_eq!(report.resolved, 1);
    let unit = world.get_unit(unit_id).unwrap();
    let UnitState::Moving { ref path, .. } = unit.state else {
        panic!("expected moving state with portal path");
    };
    assert!(
        path.waypoints.iter().any(|wp| wp.portal_id.is_some()),
        "nearby surface unit must still path through portal"
    );
    assert_eq!(
        world.get_unit(unit_id).unwrap().current_space_id,
        SpaceId::SURFACE,
        "unit must remain on surface until portal traversal begins"
    );

    for _ in 0..3 {
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
    }
    let record = world.get_unit(unit_id).unwrap();
    assert!(
        !matches!(record.state, UnitState::Idle),
        "must not falsely arrive at interior goal while still on surface"
    );
    assert_eq!(
        record.current_space_id,
        SpaceId::SURFACE,
        "must not change space without portal traversal"
    );
}

#[test]
fn interior_click_before_fix_would_have_been_blocked_or_interactable() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let interior_click = local_xz_to_world(&world, building_id, Vec2::new(4.0, 3.0), 0.0);
    assert!(
        interior_navigation_move_target_at_position(
            world.building_navigation_runtime(),
            world.space_registry(),
            world.layout(),
            interior_click,
        )
        .is_some()
    );

    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let interaction_catalog = BuildingInteractionProfileCatalog::default();
    let unit_catalog = crate::world::UnitCatalog::default();
    let weapon_catalog = WeaponCatalog::default();
    let pile_settings = ItemPileSettings::default();
    let ctx = InteractionQueryContext::new(
        &world,
        &doodad_catalog,
        &building_catalog,
        &footprint,
        &interaction_catalog,
        &unit_catalog,
        &weapon_catalog,
        &pile_settings,
    );
    let interaction = query_world_interaction(&ctx, interior_click).unwrap();
    let plan = resolve_interaction_to_order(&interaction);
    assert!(
        matches!(plan, InteractionOrderPlan::MoveTo { .. }),
        "regression: interior clicks must not become NoOp"
    );
}

fn oversized_concave_hut_blueprint() -> BuildingNavigationBlueprint {
    use super::definition::{
        NavigationEntranceDefinition, NavigationFloorDefinition, NavigationPolygon2d,
        NavigationRegionDefinition,
    };
    BuildingNavigationBlueprint::new("oversized_concave_hut", "Oversized Concave Hut")
        .with_floors(vec![NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 1.27,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![NavigationRegionDefinition {
                key: "main".to_string(),
                display_label: "Main".to_string(),
                room_tag: None,
                walkable_outline: NavigationPolygon2d {
                    vertices_xz: vec![
                        [0.0, 0.0],
                        [14.0, 0.0],
                        [14.0, 14.0],
                        [6.0, 14.0],
                        [6.0, 6.0],
                        [0.0, 6.0],
                    ],
                },
            }],
        }])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "exterior_entrance".to_string(),
            floor_key: "ground".to_string(),
            region_key: Some("main".to_string()),
            local_position_xz: [7.0, 0.0],
            radius_meters: 1.5,
            interior_spawn_local: [7.0, 1.27, 1.5],
            bidirectional: true,
            door_key: None,
        }])
}

fn place_interior_unit(
    world: &mut WorldData,
    unit_catalog: &crate::world::UnitCatalog,
    building_id: crate::world::BuildingId,
    interior_space: SpaceId,
    local_xz: Vec2,
) -> crate::world::UnitId {
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .map(|space| space.floor_y_global)
        .unwrap_or(1.27);
    let position = local_xz_to_world(world, building_id, local_xz, floor_y);
    let unit_id = create_unit_with_ownership(
        unit_catalog,
        world,
        &UnitDefinitionId::new("wolf"),
        position,
        UnitSource::Authored,
        crate::world::UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    world
        .set_unit_current_space(unit_id, interior_space)
        .expect("interior space");
    unit_id
}

fn issue_player_move(
    world: &mut WorldData,
    selected: &SelectedUnits,
    unit_catalog: &crate::world::UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    target: WorldPosition,
) {
    let building_catalog = no_profile_building_catalog();
    let footprint = FootprintCatalog::default();
    let report = issue_move_orders_to_selection(
        world,
        selected,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        &NavigationConfig::default(),
        target,
        AttackTargetingPolicy::default(),
    );
    assert_eq!(report.issued, 1, "player move should be accepted");
    let catalogs = PassabilityCatalogs {
        doodad: doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let resolve_report =
        resolve_pending_unit_orders(world, unit_catalog, catalogs, &NavigationConfig::default());
    assert_eq!(
        resolve_report.resolved, 1,
        "player move path should resolve: failures={:?}",
        resolve_report.failures
    );
}

/// IN-11eR: full player-command path retains interior authority, crosses footprint, blocks boundary.
#[test]
fn player_command_interior_footprint_cross_and_boundary_enforcement() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        oversized_concave_hut_blueprint(),
        pos(80.0, 80.0),
    );
    let interior_space = region_space(&world, building_id, "ground", "main");
    let unit_catalog = crate::world::UnitCatalog::default();
    let weapon_catalog = WeaponCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let building_catalog = no_profile_building_catalog();
    let footprint = FootprintCatalog::default();
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };

    let unit_id = place_interior_unit(
        &mut world,
        &unit_catalog,
        building_id,
        interior_space,
        Vec2::new(3.0, 3.0),
    );
    let mut selected = SelectedUnits::default();
    selected.set_single(unit_id);

    let layout = world.layout();
    let building_center = world.get_building(building_id).unwrap().placement.position;
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .map(|space| space.floor_y_global)
        .unwrap_or(1.27);
    let unit_pos = world.get_unit(unit_id).unwrap().placement.position;
    assert!(
        matches!(
            crate::world::query_passability_in_space(
                &world,
                catalogs,
                unit_pos,
                crate::world::PassabilityAgent {
                    radius_meters: 0.6,
                    max_slope_degrees: 45.0,
                },
                interior_space,
            ),
            crate::world::PassabilityResult::Passable { .. }
        ),
        "interior unit position must be passable inside owning footprint"
    );
    let center_global = building_center.to_global(layout);
    let footprint_click =
        WorldPosition::from_global(Vec3::new(center_global.x, 0.0, center_global.z), layout);
    let move_goal_click = local_xz_to_world(&world, building_id, Vec2::new(10.0, 10.0), 0.0);
    let interaction_catalog = BuildingInteractionProfileCatalog::default();
    let selected_units = [unit_id];
    let pile_settings = ItemPileSettings::default();
    let authored_relationships = crate::world::AuthoredRelationshipCatalog::default();
    let resolve_ctx = InteractionResolveContext::new(
        &world,
        &doodad_catalog,
        &building_catalog,
        &footprint,
        &interaction_catalog,
        &unit_catalog,
        &weapon_catalog,
        &pile_settings,
        &authored_relationships,
        &selected_units,
    );
    let plan =
        resolve_world_click_to_order(&resolve_ctx, footprint_click).expect("interior move plan");
    assert!(
        matches!(plan, InteractionOrderPlan::MoveTo { .. }),
        "interior unit clicking hut volume must MoveTo, not NoOp"
    );

    issue_player_move(
        &mut world,
        &selected,
        &unit_catalog,
        &weapon_catalog,
        &doodad_catalog,
        move_goal_click,
    );

    let command = world
        .movement_authority_trace()
        .latest_command_for_unit(unit_id)
        .expect("command trace");
    assert_eq!(command.start_space, interior_space);
    assert_eq!(command.goal_space, interior_space);
    assert!(
        command
            .waypoint_spaces
            .iter()
            .all(|space| *space == interior_space),
        "interior path must retain interior waypoint spaces"
    );

    let layout = world.layout();
    let goal_xz = world
        .movement_authority_trace()
        .latest_command_for_unit(unit_id)
        .map(|cmd| cmd.grounded_goal.to_global(layout).xz())
        .unwrap_or_else(|| move_goal_click.to_global(layout).xz());
    let mut arrived = false;
    for _ in 0..600 {
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        if record.current_space_id != interior_space {
            panic!("interior move must not change space without portal");
        }
        let pos_xz = record.placement.position.to_global(layout).xz();
        if pos_xz.distance(goal_xz) < 1.5 && matches!(record.state, UnitState::Idle) {
            arrived = true;
            break;
        }
    }
    assert!(
        arrived,
        "interior move through footprint axis must arrive at interior goal"
    );

    world
        .relocate_unit(
            unit_id,
            local_xz_to_world(&world, building_id, Vec2::new(2.0, 2.0), 1.27),
        )
        .unwrap();
    world.set_unit_state(unit_id, UnitState::Idle).unwrap();

    let from = local_xz_to_world(&world, building_id, Vec2::new(2.0, 2.0), 1.27);
    let concave_target = local_xz_to_world(&world, building_id, Vec2::new(9.0, 9.0), 1.27);
    assert!(
        !crate::world::interior_segment_respects_region_boundary(
            world.building_navigation_runtime(),
            world.space_registry(),
            layout,
            from,
            concave_target,
            interior_space,
            0.6,
        ),
        "direct concave diagonal must cross the region boundary"
    );

    let outside_click = local_xz_to_world(&world, building_id, Vec2::new(-3.0, 3.0), 0.0);
    let _ = issue_move_orders_to_selection(
        &mut world,
        &selected,
        &unit_catalog,
        &weapon_catalog,
        &doodad_catalog,
        &NavigationConfig::default(),
        outside_click,
        AttackTargetingPolicy::default(),
    );
    let _ = resolve_pending_unit_orders(
        &mut world,
        &unit_catalog,
        catalogs,
        &NavigationConfig::default(),
    );
    let outside_xz = outside_click.to_global(layout).xz();
    let mut exited_illegally = false;
    for _ in 0..400 {
        let _ = step_unit_movement(&mut world, &unit_catalog, catalogs, unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        let pos_xz = record.placement.position.to_global(layout).xz();
        if pos_xz.distance(outside_xz) < 1.5 && matches!(record.state, UnitState::Idle) {
            exited_illegally = true;
            break;
        }
    }
    assert!(
        !exited_illegally,
        "interior unit must not reach a position outside the region polygon"
    );

    world
        .set_unit_current_space(unit_id, SpaceId::SURFACE)
        .unwrap();
    world
        .relocate_unit(unit_id, exterior_approach(&world, building_id))
        .unwrap();
    world.set_unit_state(unit_id, UnitState::Idle).unwrap();
    let building_center = world.get_building(building_id).unwrap().placement.position;
    let agent = crate::world::PassabilityAgent {
        radius_meters: 0.6,
        max_slope_degrees: 45.0,
    };
    assert!(
        matches!(
            crate::world::query_passability_at(&world, catalogs, building_center, agent),
            crate::world::PassabilityResult::Passable { .. }
        ),
        "blueprint-controlled building must not block surface via legacy footprint after exit"
    );
}
