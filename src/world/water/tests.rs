use super::*;
use crate::terrain::render_height;
use crate::world::space::SpaceRecord;
use crate::world::unit::{
    UnitCatalog, UnitDefinitionId, UnitSource, UnitState, create_unit, issue_unit_order,
    unit_can_execute_actions,
};
use crate::world::{
    AppearanceProfileCatalog, AttackTargetingPolicy, BuildingId, ChunkCoord, ChunkData, ChunkId,
    ChunkLayout, DoodadCatalog, Heightfield, ItemCatalog, LocalPosition, NavigationConfig,
    SpaceId, TaskId, UnitOrder, WorldData, WorldPosition, WeaponCatalog,
};
use bevy::prelude::Vec3;

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn insert_terrain(world: &mut WorldData, height: f32) {
    let samples = vec![height; 9];
    let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
}

fn pos(x: f32, y: f32, z: f32) -> WorldPosition {
    WorldPosition::new(ChunkCoord::new(0, 0), LocalPosition::new(Vec3::new(x, y, z)))
}

fn spawn_wolf(world: &mut WorldData, catalog: &UnitCatalog, y: f32) -> crate::world::UnitId {
    create_unit(
        catalog,
        &AppearanceProfileCatalog::empty(),
        world,
        &UnitDefinitionId::new("wolf"),
        pos(64.0, y, 64.0),
        UnitSource::Authored,
    )
    .unwrap()
    .id
}

#[test]
fn presentation_y_is_not_copied_into_simulation() {
    let scale = 4.0;
    let presentation = DEFAULT_PRESENTATION_WATER_LEVEL;
    let sim = presentation_to_sim(presentation, scale);
    assert!((sim - 14.0).abs() < 1e-5);
    assert_ne!(sim, presentation);
    assert!((render_height(sim, scale) - presentation).abs() < 1e-4);
    assert!((sim_to_presentation(sim, scale) - presentation).abs() < 1e-4);
}

#[test]
fn apply_presentation_scale_converts_seed_and_thresholds() {
    let mut water = WorldWaterState::default();
    water.apply_presentation_scale(10.0);
    assert!(!water.has_presentation_seed());
    assert!((water.surface_y_sim - 5.6).abs() < 1e-4);
    assert!((water.enter_depth_sim - 0.1).abs() < 1e-4);
    assert!((water.exit_depth_sim - 0.06).abs() < 1e-4);
}

#[test]
fn below_enter_threshold_is_ground() {
    let mut water = WorldWaterState::default();
    water.configure_for_test(true, 10.0, 1.0, 0.6);
    assert_eq!(
        locomotion_surface_at(&water, Some(0.5), Some(LocomotionSurface::Ground)),
        LocomotionSurface::Ground
    );
}

#[test]
fn at_enter_threshold_is_water() {
    let mut water = WorldWaterState::default();
    water.configure_for_test(true, 10.0, 1.0, 0.6);
    assert_eq!(
        locomotion_surface_at(&water, Some(1.0), Some(LocomotionSurface::Ground)),
        LocomotionSurface::Water
    );
}

#[test]
fn hysteresis_keeps_water_between_exit_and_enter() {
    let mut water = WorldWaterState::default();
    water.configure_for_test(true, 10.0, 1.0, 0.6);
    assert_eq!(
        locomotion_surface_at(&water, Some(0.7), Some(LocomotionSurface::Water)),
        LocomotionSurface::Water
    );
    assert_eq!(
        locomotion_surface_at(&water, Some(0.7), Some(LocomotionSurface::Ground)),
        LocomotionSurface::Ground
    );
}

#[test]
fn hysteresis_exits_below_exit_threshold() {
    let mut water = WorldWaterState::default();
    water.configure_for_test(true, 10.0, 1.0, 0.6);
    assert_eq!(
        locomotion_surface_at(&water, Some(0.59), Some(LocomotionSurface::Water)),
        LocomotionSurface::Ground
    );
}

#[test]
fn disabled_water_is_always_ground() {
    let mut water = WorldWaterState::default();
    water.configure_for_test(false, 10.0, 1.0, 0.6);
    assert_eq!(
        locomotion_surface_at(&water, Some(50.0), None),
        LocomotionSurface::Ground
    );
}

#[test]
fn shallow_water_uses_terrain_support() {
    let mut world = WorldData::new(layout());
    insert_terrain(&mut world, 9.5);
    world.water_mut().configure_for_test(true, 10.0, 1.0, 0.6);
    let (y, surface) =
        sample_locomotion_support_height(&world, SpaceId::SURFACE, pos(64.0, 0.0, 64.0), None)
            .unwrap();
    assert_eq!(surface, LocomotionSurface::Ground);
    assert!((y - 9.5).abs() < 1e-5);
}

#[test]
fn deep_water_uses_water_support_not_seabed() {
    let mut world = WorldData::new(layout());
    insert_terrain(&mut world, 2.0);
    world.water_mut().configure_for_test(true, 10.0, 1.0, 0.6);
    world.water_mut().origin_offset_sim = -0.9;
    let (y, surface) =
        sample_locomotion_support_height(&world, SpaceId::SURFACE, pos(64.0, 2.0, 64.0), None)
            .unwrap();
    assert_eq!(surface, LocomotionSurface::Water);
    assert!((y - 9.1).abs() < 1e-5);
}

#[test]
fn underwater_slope_does_not_change_swimmer_y() {
    let mut world = WorldData::new(layout());
    insert_terrain(&mut world, 1.0);
    world.water_mut().configure_for_test(true, 10.0, 1.0, 0.6);
    world.water_mut().origin_offset_sim = 0.0;
    let low = sample_locomotion_support_height(
        &world,
        SpaceId::SURFACE,
        pos(64.0, 1.0, 64.0),
        Some(LocomotionSurface::Water),
    )
    .unwrap();
    insert_terrain(&mut world, 4.0);
    let high = sample_locomotion_support_height(
        &world,
        SpaceId::SURFACE,
        pos(64.0, 4.0, 64.0),
        Some(LocomotionSurface::Water),
    )
    .unwrap();
    assert_eq!(low.1, LocomotionSurface::Water);
    assert_eq!(high.1, LocomotionSurface::Water);
    assert!((low.0 - high.0).abs() < 1e-5);
    assert!((low.0 - 10.0).abs() < 1e-5);
}

#[test]
fn water_speed_is_half_catalog_speed() {
    let mut water = WorldWaterState::default();
    water.configure_for_test(true, 10.0, 1.0, 0.6);
    let ground = effective_move_speed_mps(4.0, LocomotionSurface::Ground, &water);
    let swim = effective_move_speed_mps(4.0, LocomotionSurface::Water, &water);
    assert!((ground - 4.0).abs() < 1e-6);
    assert!((swim - 2.0).abs() < 1e-6);
}

#[test]
fn idle_swimmer_stays_at_water_support() {
    let catalog = UnitCatalog::default();
    let weapons = WeaponCatalog::default();
    let mut world = WorldData::new(layout());
    insert_terrain(&mut world, 0.0);
    world.water_mut().configure_for_test(true, 8.0, 1.0, 0.6);
    world.water_mut().origin_offset_sim = -0.5;
    let unit_id = spawn_wolf(&mut world, &catalog, 0.0);
    refresh_all_unit_locomotion(&mut world, &catalog, &weapons);
    assert!(unit_is_swimming(&world, unit_id));
    assert!(
        (world.get_unit(unit_id).unwrap().placement.position.local.0.y - 7.5).abs() < 1e-4
    );
    refresh_all_unit_locomotion(&mut world, &catalog, &weapons);
    assert!(
        (world.get_unit(unit_id).unwrap().placement.position.local.0.y - 7.5).abs() < 1e-4
    );
}

#[test]
fn two_units_can_differ_in_surface() {
    let catalog = UnitCatalog::default();
    let weapons = WeaponCatalog::default();
    let mut world = WorldData::new(layout());
    insert_terrain(&mut world, 0.0);
    world.water_mut().configure_for_test(true, 8.0, 1.0, 0.6);
    let swimmer = spawn_wolf(&mut world, &catalog, 0.0);
    world.water_mut().configure_for_test(true, 0.2, 1.0, 0.6);
    let walker = create_unit(
        &catalog,
        &AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(80.0, 0.0, 80.0),
        UnitSource::Authored,
    )
    .unwrap()
    .id;
    world.water_mut().configure_for_test(true, 8.0, 1.0, 0.6);
    // Force walker previous/current by placing it on a raised cell... same heightfield.
    // Instead: set walker hysteresis to Ground with shallow override via stored surface
    // after refresh, both would swim. Use independent previous: mark walker Ground with
    // a depth below enter by temporarily lowering water for that sample.
    world.set_locomotion_surface(walker, LocomotionSurface::Ground);
    world.water_mut().surface_y_sim = 0.2;
    refresh_all_unit_locomotion(&mut world, &catalog, &weapons);
    assert!(!unit_is_swimming(&world, walker));
    world.water_mut().surface_y_sim = 8.0;
    world.set_locomotion_surface(swimmer, LocomotionSurface::Water);
    refresh_all_unit_locomotion(&mut world, &catalog, &weapons);
    assert!(unit_is_swimming(&world, swimmer));
}

#[test]
fn default_world_water_does_not_force_swimming() {
    let catalog = UnitCatalog::default();
    let weapons = WeaponCatalog::default();
    let mut world = WorldData::new(layout());
    insert_terrain(&mut world, 0.0);
    let unit_id = spawn_wolf(&mut world, &catalog, 0.0);
    refresh_all_unit_locomotion(&mut world, &catalog, &weapons);
    assert!(!unit_is_swimming(&world, unit_id));
    assert!(!unit_order_requires_normal_actions(UnitOrder::MoveTo {
        target: pos(1.0, 0.0, 1.0)
    }));
    assert!(unit_order_requires_normal_actions(UnitOrder::Attack {
        target: unit_id
    }));
    assert!(unit_can_perform_normal_actions(&world, unit_id));
}

#[test]
fn swimming_blocks_normal_actions_not_movement_orders() {
    let catalog = UnitCatalog::default();
    let weapons = WeaponCatalog::default();
    let mut world = WorldData::new(layout());
    insert_terrain(&mut world, 0.0);
    world.water_mut().configure_for_test(true, 8.0, 1.0, 0.6);
    let unit_id = spawn_wolf(&mut world, &catalog, 0.0);
    refresh_all_unit_locomotion(&mut world, &catalog, &weapons);
    assert!(unit_is_swimming(&world, unit_id));
    assert!(!unit_can_perform_normal_actions(&world, unit_id));
    assert!(unit_can_execute_actions(&world, unit_id));
}

#[test]
fn interior_floor_ignores_ocean_water() {
    let mut world = WorldData::new(layout());
    insert_terrain(&mut world, 0.0);
    world.water_mut().configure_for_test(true, 100.0, 1.0, 0.6);
    let interior = SpaceId::new(1);
    world.space_registry_mut().insert_space(SpaceRecord {
        id: interior,
        owning_building_id: Some(BuildingId::new(1)),
        display_floor_label: "Ground Floor".into(),
        visibility_group_id: 1,
        reference_elevation: 0.0,
        floor_y_global: 2.5,
        room_tag: None,
        enabled: true,
        walkable: true,
    });
    let (y, surface) =
        sample_locomotion_support_height(&world, interior, pos(64.0, 0.0, 64.0), None).unwrap();
    assert_eq!(surface, LocomotionSurface::Ground);
    assert!((y - 2.5).abs() < 1e-5);
}

#[test]
fn swimming_depth_skips_seabed_slope_check() {
    let mut world = WorldData::new(layout());
    insert_terrain(&mut world, 0.0);
    world.water_mut().configure_for_test(true, 8.0, 1.0, 0.6);
    let position = pos(64.0, 0.0, 64.0);
    assert!(water_skips_seabed_slope(&world, position));
}

#[test]
fn shallow_depth_does_not_skip_seabed_slope() {
    let mut world = WorldData::new(layout());
    insert_terrain(&mut world, 7.6);
    world.water_mut().configure_for_test(true, 8.0, 1.0, 0.6);
    let position = pos(64.0, 0.0, 64.0);
    assert!(!water_skips_seabed_slope(&world, position));
}

#[test]
fn swimming_rejects_attack_orders() {
    let catalog = UnitCatalog::default();
    let weapons = WeaponCatalog::default();
    let doodads = DoodadCatalog::default();
    let items = ItemCatalog::default();
    let nav = NavigationConfig::default();
    let mut world = WorldData::new(layout());
    insert_terrain(&mut world, 0.0);
    world.water_mut().configure_for_test(true, 8.0, 1.0, 0.6);
    let attacker = spawn_wolf(&mut world, &catalog, 0.0);
    let target = create_unit(
        &catalog,
        &AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(80.0, 0.0, 80.0),
        UnitSource::Authored,
    )
    .unwrap()
    .id;
    refresh_all_unit_locomotion(&mut world, &catalog, &weapons);
    assert!(unit_is_swimming(&world, attacker));
    let err = issue_unit_order(
        &mut world,
        &catalog,
        &weapons,
        &items,
        &doodads,
        &nav,
        attacker,
        UnitOrder::Attack { target },
        AttackTargetingPolicy::default(),
    );
    assert!(err.is_err());
}

#[test]
fn entering_water_cancels_working_state() {
    let catalog = UnitCatalog::default();
    let weapons = WeaponCatalog::default();
    let mut world = WorldData::new(layout());
    insert_terrain(&mut world, 0.0);
    world.water_mut().configure_for_test(true, 8.0, 1.0, 0.6);
    let unit_id = spawn_wolf(&mut world, &catalog, 0.0);
    world
        .set_unit_state(unit_id, UnitState::Working { task_id: TaskId::new(1) })
        .unwrap();
    refresh_all_unit_locomotion(&mut world, &catalog, &weapons);
    assert!(unit_is_swimming(&world, unit_id));
    assert!(matches!(
        world.get_unit(unit_id).unwrap().state,
        UnitState::Idle
    ));
}

#[test]
fn exit_water_restores_normal_action_eligibility() {
    let catalog = UnitCatalog::default();
    let weapons = WeaponCatalog::default();
    let mut world = WorldData::new(layout());
    insert_terrain(&mut world, 9.8);
    world.water_mut().configure_for_test(true, 10.0, 1.0, 0.6);
    let unit_id = spawn_wolf(&mut world, &catalog, 9.8);
    world.set_locomotion_surface(unit_id, LocomotionSurface::Water);
    refresh_all_unit_locomotion(&mut world, &catalog, &weapons);
    assert!(!unit_is_swimming(&world, unit_id));
    assert!(unit_can_perform_normal_actions(&world, unit_id));
}
