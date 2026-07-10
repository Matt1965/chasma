//! Read-only capture from [`WorldData`] (ADR-048). Never mutates simulation.

use bevy::prelude::*;

use crate::ui::gameplay::combat_display::{
    attack_cycle_summary, combat_target_id, weapon_display_for_unit,
};
use crate::world::{
    alignment_force, blocking_doodad_at_position, cohesion_force, gather_steering_neighbors,
    ground_world_position, interaction_plan_to_unit_order, is_position_slope_walkable,
    query_world_interaction, resolve_interaction_to_order, separation_force, unit_spacing_meters,
    ChunkCoord, DoodadCatalog, InteractionQueryContext, NavigationPath, SteeringContext,
    SteeringSettings, UnitCatalog, UnitId, UnitState, WeaponCatalog, WorldData, WorldPosition,
};

use super::snapshot::{
    ChunkResidencySnapshot, CombatInspectorSnapshot, FormationInspectorSnapshot,
    InteractionInspectorSnapshot, PathInspectorSnapshot, ProjectileInspectorSnapshot,
    SteeringInspectorSnapshot, UnitInspectorSnapshot,
};

const STEERING_SETTINGS: SteeringSettings = SteeringSettings::DEFAULT;
const FORMATION_TARGET_EPSILON: f32 = 0.25;

/// Capture a full unit inspection snapshot. Returns `None` if the unit does not exist.
pub fn capture_unit_inspector_snapshot(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    unit_id: UnitId,
    simulation_tick: u64,
) -> Option<UnitInspectorSnapshot> {
    let record = world.get_unit(unit_id)?.clone();
    let definition = unit_catalog.get(&record.definition_id)?;
    let layout = world.layout();
    let position = record.placement.position;

    let path = capture_path_inspector(&record.state, position, layout);
    let formation = capture_formation_inspector(world, unit_id, &record.state, layout, definition.collision_radius_meters);
    let steering = capture_steering_inspector(
        world,
        unit_catalog,
        unit_id,
        position,
        &record.state,
        definition.collision_radius_meters,
        layout,
    );
    let block_reason = diagnose_block_reason(
        world,
        doodad_catalog,
        position,
        definition.collision_radius_meters,
    );
    let chunk = capture_chunk_residency(world, unit_id)?;

    let combat = capture_combat_inspector(&record, unit_catalog, weapon_catalog);
    let projectiles = capture_projectiles_for_unit(world, unit_id);

    Some(UnitInspectorSnapshot {
        unit_id,
        definition_id: record.definition_id.clone(),
        state_label: state_label(&record.state),
        current_hp: record.vitals.current_hp,
        max_hp: record.vitals.max_hp,
        combat_state_label: record.combat_state.label().to_string(),
        combat,
        projectiles,
        path,
        formation,
        steering,
        block_reason,
        chunk,
        simulation_tick,
    })
}

/// Capture interaction classification at a world click (U6 + U-UI5).
pub fn capture_interaction_inspector_snapshot(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    weapon_catalog: &crate::world::WeaponCatalog,
    click_position: WorldPosition,
) -> Option<InteractionInspectorSnapshot> {
    let ctx = InteractionQueryContext::new(world, doodad_catalog, unit_catalog, weapon_catalog);
    let terrain_hit = ground_world_position(world, click_position).is_some();
    let interaction = query_world_interaction(&ctx, click_position)?;
    let plan = resolve_interaction_to_order(&interaction);
    let order = interaction_plan_to_unit_order(plan);

    let doodad_hit = match &interaction.target {
        crate::world::InteractionTargetRef::Doodad(doodad_id) => world
            .get_doodad(*doodad_id)
            .map(|record| record.definition_id.clone()),
        _ => None,
    };

    Some(InteractionInspectorSnapshot {
        click_position,
        terrain_hit,
        doodad_hit,
        interaction_type: format!("{:?}", interaction.interaction_type),
        resolved_command: order.as_ref().map(|o| format!("{o:?}")),
        resolved_order: order,
    })
}

fn state_label(state: &UnitState) -> String {
    match state {
        UnitState::Idle => "Idle".into(),
        UnitState::Moving { .. } => "Moving".into(),
        UnitState::Dead => "Dead".into(),
    }
}

fn capture_combat_inspector(
    record: &crate::world::UnitRecord,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) -> CombatInspectorSnapshot {
    CombatInspectorSnapshot {
        weapon_name: weapon_display_for_unit(record, unit_catalog, weapon_catalog)
            .map(|w| w.name),
        target_unit_id: combat_target_id(&record.combat_state),
        attack_phase: record
            .attack_cycle
            .as_ref()
            .map(attack_cycle_summary),
    }
}

fn capture_projectiles_for_unit(world: &WorldData, unit_id: UnitId) -> Vec<ProjectileInspectorSnapshot> {
    world
        .sorted_projectile_ids()
        .into_iter()
        .filter_map(|id| world.get_projectile(id))
        .filter(|record| record.source_unit_id == unit_id)
        .map(projectile_inspector_from_record)
        .collect()
}

pub fn capture_projectile_inspector_snapshot(
    world: &WorldData,
    projectile_id: crate::world::ProjectileId,
) -> Option<ProjectileInspectorSnapshot> {
    world
        .get_projectile(projectile_id)
        .map(projectile_inspector_from_record)
}

fn projectile_inspector_from_record(record: &crate::world::ProjectileRecord) -> ProjectileInspectorSnapshot {
    ProjectileInspectorSnapshot {
        projectile_id: record.id,
        source_unit_id: record.source_unit_id,
        target_unit_id: record.target_unit_id,
        weapon_id: record.weapon_id.as_str().to_string(),
        position: record.position,
        speed_mps: record.speed_mps,
        status: projectile_status_label(record.status).to_string(),
    }
}

fn projectile_status_label(status: crate::world::ProjectileStatus) -> &'static str {
    match status {
        crate::world::ProjectileStatus::InFlight => "InFlight",
        crate::world::ProjectileStatus::Hit => "Hit",
        crate::world::ProjectileStatus::Expired => "Expired",
        crate::world::ProjectileStatus::Invalidated => "Invalidated",
    }
}

fn capture_path_inspector(
    state: &UnitState,
    unit_position: WorldPosition,
    layout: crate::world::ChunkLayout,
) -> PathInspectorSnapshot {
    let UnitState::Moving {
        path,
        waypoint_index,
        ..
    } = state
    else {
        return PathInspectorSnapshot::default();
    };

    let (segment_start, segment_end) = active_segment(*waypoint_index, unit_position, path);
    PathInspectorSnapshot {
        waypoints: path.waypoints.clone(),
        waypoint_index: *waypoint_index,
        segment_start,
        segment_end,
        length_meters: path.length_meters(layout),
        chunk_transitions: chunk_transitions_along_path(path),
    }
}

fn active_segment(
    waypoint_index: usize,
    unit_position: WorldPosition,
    path: &NavigationPath,
) -> (Option<WorldPosition>, Option<WorldPosition>) {
    let start = if waypoint_index == 0 {
        Some(unit_position)
    } else {
        path.waypoints.get(waypoint_index.saturating_sub(1)).copied()
    };
    let end = path.waypoints.get(waypoint_index).copied();
    (start, end)
}

fn chunk_transitions_along_path(path: &NavigationPath) -> Vec<ChunkCoord> {
    let mut chunks = Vec::new();
    let mut last: Option<ChunkCoord> = None;
    for waypoint in &path.waypoints {
        if last != Some(waypoint.chunk) {
            chunks.push(waypoint.chunk);
            last = Some(waypoint.chunk);
        }
    }
    chunks
}

fn capture_formation_inspector(
    world: &WorldData,
    unit_id: UnitId,
    state: &UnitState,
    layout: crate::world::ChunkLayout,
    collision_radius: f32,
) -> FormationInspectorSnapshot {
    let UnitState::Moving { target, .. } = state else {
        return FormationInspectorSnapshot {
            spacing_meters: unit_spacing_meters(collision_radius),
            ..Default::default()
        };
    };

    let unit_global = world
        .get_unit(unit_id)
        .map(|r| r.placement.position.to_global(layout))
        .unwrap_or(Vec3::ZERO);
    let target_global = target.to_global(layout);
    let offset_xz = Vec2::new(
        target_global.x - unit_global.x,
        target_global.z - unit_global.z,
    );

    let mut peers: Vec<UnitId> = world
        .sorted_unit_ids()
        .into_iter()
        .filter(|id| {
            world
                .get_unit(*id)
                .and_then(|record| match &record.state {
                    UnitState::Moving { target: peer_target, .. } => {
                        Some(positions_close(*peer_target, *target, layout))
                    }
                    _ => None,
                })
                .unwrap_or(false)
        })
        .collect();
    peers.sort_unstable();
    let slot_index = peers.iter().position(|id| *id == unit_id);

    FormationInspectorSnapshot {
        slot_index,
        offset_xz,
        target: Some(*target),
        spacing_meters: unit_spacing_meters(collision_radius),
        peers_sharing_target: peers.len() as u32,
    }
}

fn positions_close(a: WorldPosition, b: WorldPosition, layout: crate::world::ChunkLayout) -> bool {
    let ga = a.to_global(layout);
    let gb = b.to_global(layout);
    Vec2::new(ga.x - gb.x, ga.z - gb.z).length() <= FORMATION_TARGET_EPSILON
}

fn capture_steering_inspector(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    unit_id: UnitId,
    position: WorldPosition,
    state: &UnitState,
    collision_radius: f32,
    layout: crate::world::ChunkLayout,
) -> SteeringInspectorSnapshot {
    let UnitState::Moving {
        target,
        path,
        waypoint_index,
        ..
    } = state
    else {
        return SteeringInspectorSnapshot::default();
    };

    let path_direction = path_direction_xz(position, path, *waypoint_index, layout);
    let global = position.to_global(layout);
    let position_xz = Vec2::new(global.x, global.z);
    let target_global = target.to_global(layout);
    let formation_target_xz = Vec2::new(target_global.x, target_global.z);

    let neighbors = gather_steering_neighbors(
        world,
        unit_catalog,
        unit_id,
        position,
        STEERING_SETTINGS.neighbor_query_radius,
    );

    let separation = separation_force(
        position_xz,
        collision_radius,
        &neighbors,
        &STEERING_SETTINGS,
    );
    let cohesion = cohesion_force(
        position_xz,
        Some(formation_target_xz),
        &neighbors,
        &STEERING_SETTINGS,
    );
    let alignment = alignment_force(&neighbors, &STEERING_SETTINGS);

    let context = SteeringContext {
        unit_id,
        position_xz,
        path_direction_xz: path_direction,
        collision_radius,
        formation_target_xz: Some(formation_target_xz),
        neighbors: neighbors.clone(),
        delta_seconds: 1.0 / 60.0,
        settings: STEERING_SETTINGS,
    };
    let final_direction = context.steered_direction_xz();

    SteeringInspectorSnapshot {
        separation,
        cohesion,
        alignment,
        final_direction,
        neighbor_count: neighbors.len() as u32,
        path_direction,
    }
}

fn path_direction_xz(
    position: WorldPosition,
    path: &NavigationPath,
    waypoint_index: usize,
    layout: crate::world::ChunkLayout,
) -> Vec2 {
    let Some(waypoint) = path.waypoints.get(waypoint_index).copied() else {
        return Vec2::ZERO;
    };
    let current = position.to_global(layout);
    let waypoint_global = waypoint.to_global(layout);
    let delta = Vec2::new(
        waypoint_global.x - current.x,
        waypoint_global.z - current.z,
    );
    if delta.length_squared() <= 1e-8 {
        return Vec2::ZERO;
    }
    delta.normalize()
}

fn diagnose_block_reason(
    world: &WorldData,
    doodad_catalog: &DoodadCatalog,
    position: WorldPosition,
    radius: f32,
) -> Option<String> {
    if ground_world_position(world, position).is_none() {
        return Some("Missing terrain / chunk not resident".into());
    }
    if let Some(doodad_id) = blocking_doodad_at_position(world, doodad_catalog, position, radius) {
        return Some(format!("Blocked by doodad #{}", doodad_id.raw()));
    }
    if !is_position_slope_walkable(world, position, 40.0) {
        return Some("Unwalkable slope".into());
    }
    None
}

fn capture_chunk_residency(world: &WorldData, unit_id: UnitId) -> Option<ChunkResidencySnapshot> {
    let chunk_id = world.unit_chunk(unit_id)?;
    let coord = chunk_id.coord();
    let terrain_loaded = world.is_chunk_loaded(chunk_id);
    let doodads_in_chunk = world
        .doodads_in_chunk(chunk_id)
        .map(|store| store.len() as u32)
        .unwrap_or(0);
    let units_in_chunk = world
        .units_in_chunk(chunk_id)
        .map(|store| store.len() as u32)
        .unwrap_or(0);
    Some(ChunkResidencySnapshot {
        unit_chunk: coord,
        terrain_loaded,
        doodads_in_chunk,
        units_in_chunk,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_unit, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog, Heightfield,
        LocalPosition, NavigationPath, UnitDefinitionId, UnitOrder, UnitSource, UnitState,
    };

    fn flat_chunk() -> ChunkData {
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        ChunkData::new(heightfield, Vec::new())
    }

    fn insert_flat(world: &mut WorldData) {
        let chunk = ChunkId::new(ChunkCoord::new(0, 0));
        world.insert(chunk, flat_chunk());
    }

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

    fn spawn_wolf(world: &mut WorldData, catalog: &UnitCatalog, position: WorldPosition) -> UnitId {
        create_unit(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            position,
            UnitSource::Authored,
        )
        .unwrap()
        .id
    }

    #[test]
    fn inspector_returns_correct_unit_state_snapshot() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(1.0, 2.0));
        world
            .set_unit_state(
                unit_id,
                UnitState::Moving {
                    target: pos(10.0, 10.0),
                    path: NavigationPath::new(vec![pos(5.0, 5.0), pos(10.0, 10.0)]),
                    waypoint_index: 0,
                },
            )
            .unwrap();

        let snap = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            unit_id,
            7,
        )
        .unwrap();
        assert_eq!(snap.unit_id, unit_id);
        assert_eq!(snap.state_label, "Moving");
        assert_eq!(snap.path.waypoints.len(), 2);
        assert_eq!(snap.simulation_tick, 7);
    }

    #[test]
    fn path_inspection_matches_world_data_path() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0.0, 0.0));
        let waypoints = vec![pos(20.0, 0.0), pos(20.0, 20.0)];
        world
            .set_unit_state(
                unit_id,
                UnitState::Moving {
                    target: pos(20.0, 20.0),
                    path: NavigationPath::new(waypoints.clone()),
                    waypoint_index: 1,
                },
            )
            .unwrap();

        let snap = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            unit_id,
            0,
        )
        .unwrap();
        assert_eq!(snap.path.waypoints, waypoints);
        assert_eq!(snap.path.waypoint_index, 1);
        assert!(snap.path.length_meters > 0.0);
    }

    #[test]
    fn steering_values_match_simulation_output() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0.0, 0.0));
        world
            .set_unit_state(
                unit_id,
                UnitState::Moving {
                    target: pos(10.0, 0.0),
                    path: NavigationPath::new(vec![pos(10.0, 0.0)]),
                    waypoint_index: 0,
                },
            )
            .unwrap();

        let snap = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            unit_id,
            0,
        )
        .unwrap();
        assert!(snap.steering.path_direction.length() > 0.0);
        assert_eq!(snap.steering.neighbor_count, 0);
    }

    #[test]
    fn formation_inspector_matches_moving_target() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0.0, 0.0));
        let target = pos(8.0, 0.0);
        world
            .set_unit_state(
                unit_id,
                UnitState::Moving {
                    target,
                    path: NavigationPath::new(vec![target]),
                    waypoint_index: 0,
                },
            )
            .unwrap();

        let snap = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            unit_id,
            0,
        )
        .unwrap();
        assert_eq!(snap.formation.target, Some(target));
        assert!((snap.formation.offset_xz.x - 8.0).abs() < 0.01);
    }

    #[test]
    fn interaction_inspector_resolves_move_target() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let click = pos(64.0, 64.0);
        let snap = capture_interaction_inspector_snapshot(
            &world,
            &catalog,
            &DoodadCatalog::default(),
            &crate::world::WeaponCatalog::default(),
            click,
        )
        .unwrap();
        assert!(snap.terrain_hit);
        assert!(snap.interaction_type.contains("MoveTarget"));
        assert!(matches!(snap.resolved_order, Some(UnitOrder::MoveTo { .. })));
    }

    #[test]
    fn inspector_does_not_mutate_simulation_state() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(3.0, 4.0));
        let before = world.get_unit(unit_id).unwrap().clone();
        let _ = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            unit_id,
            0,
        );
        assert_eq!(world.get_unit(unit_id).unwrap(), &before);
    }

    #[test]
    fn cached_snapshot_fields_remain_consistent_on_repeat_capture() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(1.0, 1.0));
        let a = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            unit_id,
            3,
        )
        .unwrap();
        let b = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            unit_id,
            3,
        )
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn inspector_reads_weapon_and_combat_fields() {
        let catalog = UnitCatalog::default();
        let weapons = crate::world::WeaponCatalog::from_definitions(
            crate::world::starter_weapon_definitions(),
        )
        .unwrap();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(1.0, 1.0));
        let snap = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            unit_id,
            0,
        )
        .unwrap();
        assert_eq!(snap.combat.weapon_name.as_deref(), Some("Wolf Bite"));
        assert!(snap.combat.attack_phase.is_none());
    }

    #[test]
    fn projectile_inspector_reads_projectile_record_only() {
        use crate::world::{
            DamageType, ProjectileId, ProjectileLaunchSnapshot, ProjectileRecord, WeaponDefinitionId,
        };
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let record = ProjectileRecord::new_in_flight(
            ProjectileId::new(1),
            UnitId::new(1),
            UnitId::new(2),
            WeaponDefinitionId::new("weapon_bow"),
            5.0,
            DamageType::Piercing,
            pos(0.0, 0.0),
            pos(10.0, 0.0),
            20.0,
            ProjectileLaunchSnapshot::render_test_placeholder(UnitId::new(1)),
        );
        world.insert_projectile(record.clone());
        let snap = capture_projectile_inspector_snapshot(&world, ProjectileId::new(1)).unwrap();
        assert_eq!(snap.projectile_id, ProjectileId::new(1));
        assert_eq!(snap.speed_mps, 20.0);
        assert_eq!(snap.status, "InFlight");
    }
}
