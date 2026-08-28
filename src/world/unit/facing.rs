//! Authoritative unit facing from accepted horizontal travel (UNIT-FACING-1).
//!
//! Model forward convention: local **-Z** in world XZ.
//!
//! Presentation-only visual yaw interpolation (UNIT-TURN-1) also lives here.

use std::f32::consts::PI;

use bevy::prelude::*;

use super::combat_state::CombatState;
use super::id::UnitId;
use super::state::UnitState;
use crate::world::WorldData;
use crate::world::unit::UnitInsertError;
use crate::world::{ChunkLayout, WorldPosition};

/// Minimum accepted XZ travel before updating facing (matches movement progress guards).
pub const MOVEMENT_FACING_EPSILON_METERS: f32 = 1e-3;

/// Yaw-only rotation so local `-Z` aligns with normalized horizontal `direction`.
pub fn facing_rotation_from_direction_xz(direction: Vec2) -> Quat {
    debug_assert!(direction.length_squared() > 1e-8);
    // `direction.x` = world X, `direction.y` = world Z.
    // Bevy `from_rotation_y` maps local -Z to (-sin(yaw), 0, -cos(yaw)).
    Quat::from_rotation_y((-direction.x).atan2(-direction.y))
}

/// Horizontal displacement in world meters between two authoritative positions.
pub fn xz_displacement_meters(from: WorldPosition, to: WorldPosition, layout: ChunkLayout) -> Vec2 {
    let from_global = from.to_global(layout);
    let to_global = to.to_global(layout);
    Vec2::new(to_global.x - from_global.x, to_global.z - from_global.z)
}

/// Derive facing from accepted travel; `None` when XZ displacement is below epsilon.
pub fn facing_rotation_from_travel(
    from: WorldPosition,
    to: WorldPosition,
    layout: ChunkLayout,
) -> Option<Quat> {
    let delta = xz_displacement_meters(from, to, layout);
    if delta.length_squared() <= MOVEMENT_FACING_EPSILON_METERS * MOVEMENT_FACING_EPSILON_METERS {
        return None;
    }
    Some(facing_rotation_from_direction_xz(delta.normalize()))
}

/// Derive facing from attacker position toward a world target position.
pub fn facing_rotation_toward_position(
    from: WorldPosition,
    to: WorldPosition,
    layout: ChunkLayout,
) -> Option<Quat> {
    facing_rotation_from_travel(from, to, layout)
}

pub fn model_forward_xz(rotation: Quat) -> Vec2 {
    let forward = rotation * Vec3::NEG_Z;
    let xz = Vec2::new(forward.x, forward.z);
    if xz.length_squared() <= 1e-8 {
        Vec2::Y
    } else {
        xz.normalize()
    }
}

/// Extract yaw (radians) from a yaw-only world rotation.
pub fn yaw_radians_from_rotation(rotation: Quat) -> f32 {
    rotation.to_euler(EulerRot::YXZ).0
}

/// Yaw-only world rotation.
pub fn rotation_from_yaw_radians(yaw: f32) -> Quat {
    Quat::from_rotation_y(yaw)
}

/// Shortest signed yaw delta from `current` to `target`, in `(-PI, PI]`.
pub fn shortest_yaw_delta_radians(current: f32, target: f32) -> f32 {
    let mut delta = target - current;
    while delta > PI {
        delta -= 2.0 * PI;
    }
    while delta <= -PI {
        delta += 2.0 * PI;
    }
    delta
}

/// Step `current_yaw` toward `target_yaw` by at most `max_step_radians` along the shortest path.
///
/// At exactly 180°, rotates toward **+PI** (deterministic tie-break).
pub fn step_yaw_toward(current_yaw: f32, target_yaw: f32, max_step_radians: f32) -> f32 {
    if max_step_radians <= 0.0 {
        return current_yaw;
    }
    let delta = shortest_yaw_delta_radians(current_yaw, target_yaw);
    if delta.abs() <= max_step_radians {
        return target_yaw;
    }
    current_yaw + delta.signum() * max_step_radians
}

/// Rate-limit presentation yaw toward a target world-facing rotation (yaw-only).
pub fn step_rotation_yaw_toward(
    current: Quat,
    target: Quat,
    turn_speed_rad_per_sec: f32,
    delta_seconds: f32,
) -> Quat {
    if delta_seconds <= 0.0 {
        return current;
    }
    let current_yaw = yaw_radians_from_rotation(current);
    let target_yaw = yaw_radians_from_rotation(target);
    let max_step = turn_speed_rad_per_sec * delta_seconds;
    rotation_from_yaw_radians(step_yaw_toward(current_yaw, target_yaw, max_step))
}

/// Update authoritative facing toward a combat target while stationary and attacking (COMBAT-FACING-1).
///
/// Moving units keep travel-derived facing authority. Peaceful or non-attacking units are unchanged.
pub fn apply_attacking_combat_facing(
    world: &mut WorldData,
    unit_id: UnitId,
    target_id: UnitId,
) -> Result<(), UnitInsertError> {
    let Some(record) = world.get_unit(unit_id) else {
        return Err(UnitInsertError::UnitNotFound);
    };
    if matches!(record.state, UnitState::Moving { .. }) {
        return Ok(());
    }
    match record.combat_state {
        CombatState::Attacking { target } if target == target_id => {}
        _ => return Ok(()),
    }
    world.apply_unit_facing_toward_unit(unit_id, target_id)
}

#[cfg(test)]
mod turn_math_tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn aligned_yaw_unchanged() {
        let yaw = 1.2;
        assert_eq!(step_yaw_toward(yaw, yaw, 1.0), yaw);
    }

    #[test]
    fn partial_turn_with_insufficient_time() {
        let result = step_yaw_toward(0.0, FRAC_PI_2, FRAC_PI_2 * 0.25);
        assert!((result - FRAC_PI_2 * 0.25).abs() < 1e-5);
        assert_ne!(result, FRAC_PI_2);
    }

    #[test]
    fn sufficient_time_reaches_target() {
        assert!((step_yaw_toward(0.0, FRAC_PI_2, FRAC_PI_2) - FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn no_overshoot() {
        let target = 2.0;
        let result = step_yaw_toward(0.0, target, 10.0);
        assert_eq!(result, target);
    }

    #[test]
    fn shortest_path_near_wrap_positive() {
        let current = 179.0f32.to_radians();
        let target = -179.0f32.to_radians();
        let delta = shortest_yaw_delta_radians(current, target);
        assert!((delta - 2.0f32.to_radians()).abs() < 1e-4);
        let stepped = step_yaw_toward(current, target, 1.0_f32.to_radians());
        assert!((stepped - (current + 1.0f32.to_radians())).abs() < 1e-4);
    }

    #[test]
    fn shortest_path_near_wrap_negative() {
        let current = -179.0f32.to_radians();
        let target = 179.0f32.to_radians();
        let delta = shortest_yaw_delta_radians(current, target);
        assert!((delta - (-2.0f32.to_radians())).abs() < 1e-4);
    }

    #[test]
    fn exact_180_is_deterministic_positive() {
        let stepped = step_yaw_toward(0.0, PI, 0.5);
        assert!((stepped - 0.5).abs() < 1e-5);
        let stepped2 = step_yaw_toward(0.0, -PI, 0.5);
        assert!((stepped2 - 0.5).abs() < 1e-5);
    }

    #[test]
    fn frame_rate_independence_two_half_steps() {
        let target = FRAC_PI_2;
        let once = step_yaw_toward(0.0, target, target * 0.5);
        let twice = step_yaw_toward(
            step_yaw_toward(0.0, target, target * 0.25),
            target,
            target * 0.25,
        );
        assert!((once - twice).abs() < 1e-4);
    }

    #[test]
    fn zero_delta_seconds_unchanged() {
        let current = rotation_from_yaw_radians(0.75);
        let target = rotation_from_yaw_radians(2.0);
        assert_eq!(
            step_rotation_yaw_toward(current, target, 10.0, 0.0),
            current
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn pos(chunk_x: i32, chunk_z: i32, x: f32, y: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(chunk_x, chunk_z),
            LocalPosition::new(Vec3::new(x, y, z)),
        )
    }

    fn assert_forward_matches(rotation: Quat, expected: Vec2) {
        let forward = model_forward_xz(rotation);
        let expected = expected.normalize();
        assert!(
            (forward - expected).length() < 1e-4,
            "expected {expected:?}, got {forward:?}"
        );
    }

    #[test]
    fn cardinals_face_travel_direction() {
        assert_forward_matches(
            facing_rotation_from_direction_xz(Vec2::new(0.0, -1.0)),
            Vec2::new(0.0, -1.0),
        );
        assert_forward_matches(
            facing_rotation_from_direction_xz(Vec2::new(0.0, 1.0)),
            Vec2::new(0.0, 1.0),
        );
        assert_forward_matches(
            facing_rotation_from_direction_xz(Vec2::new(1.0, 0.0)),
            Vec2::new(1.0, 0.0),
        );
        assert_forward_matches(
            facing_rotation_from_direction_xz(Vec2::new(-1.0, 0.0)),
            Vec2::new(-1.0, 0.0),
        );
    }

    #[test]
    fn diagonals_face_travel_direction() {
        for direction in [
            Vec2::new(1.0, -1.0),
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ] {
            assert_forward_matches(facing_rotation_from_direction_xz(direction), direction);
        }
    }

    #[test]
    fn no_xz_displacement_preserves_facing() {
        let from = pos(0, 0, 10.0, 0.0, 10.0);
        let to = pos(0, 0, 10.0, 0.0, 10.0);
        assert!(facing_rotation_from_travel(from, to, layout()).is_none());
    }

    #[test]
    fn y_only_displacement_preserves_facing() {
        let from = pos(0, 0, 10.0, 1.0, 10.0);
        let to = pos(0, 0, 10.0, 5.0, 10.0);
        assert!(facing_rotation_from_travel(from, to, layout()).is_none());
    }

    #[test]
    fn tiny_displacement_under_epsilon_preserves_facing() {
        let from = pos(0, 0, 10.0, 0.0, 10.0);
        let to = pos(0, 0, 10.0005, 0.0, 10.0);
        assert!(facing_rotation_from_travel(from, to, layout()).is_none());
    }

    #[test]
    fn reversal_faces_new_direction() {
        let layout = layout();
        let start = pos(0, 0, 0.0, 0.0, 0.0);
        let east = pos(0, 0, 5.0, 0.0, 0.0);
        let west = pos(0, 0, -5.0, 0.0, 0.0);
        let east_rot = facing_rotation_from_travel(start, east, layout).unwrap();
        let west_rot = facing_rotation_from_travel(east, west, layout).unwrap();
        assert_forward_matches(east_rot, Vec2::new(1.0, 0.0));
        assert_forward_matches(west_rot, Vec2::new(-1.0, 0.0));
    }

    #[test]
    fn chunk_boundary_travel_uses_global_coordinates() {
        let layout = layout();
        let from = pos(0, 0, 250.0, 0.0, 128.0);
        let to = pos(1, 0, 10.0, 0.0, 128.0);
        let rotation = facing_rotation_from_travel(from, to, layout).unwrap();
        assert_forward_matches(rotation, Vec2::new(1.0, 0.0));
    }
}

#[cfg(test)]
mod combat_facing_tests {
    use super::*;
    use crate::world::navigation::NavigationPath;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, UnitCatalog,
        UnitDefinitionId, UnitSource, create_unit,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn flat_world() -> crate::world::WorldData {
        let mut world = crate::world::WorldData::new(layout());
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn spawn_unit(
        world: &mut crate::world::WorldData,
        catalog: &UnitCatalog,
        x: f32,
        z: f32,
        rotation: Quat,
    ) -> UnitId {
        create_unit(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            pos(x, z),
            UnitSource::Authored,
        )
        .map(|record| {
            world.set_unit_facing_for_test(record.id, rotation).unwrap();
            record.id
        })
        .unwrap()
    }

    fn assert_forward_toward(from: Quat, from_pos: WorldPosition, to_pos: WorldPosition) {
        let expected = facing_rotation_toward_position(from_pos, to_pos, layout()).unwrap();
        let forward = model_forward_xz(from);
        let expected_forward = model_forward_xz(expected);
        assert!(
            (forward - expected_forward).length() < 1e-4,
            "expected {expected_forward:?}, got {forward:?}"
        );
    }

    #[test]
    fn stationary_attacking_unit_rotates_toward_target() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let attacker = spawn_unit(&mut world, &catalog, 10.0, 10.0, Quat::from_rotation_y(0.0));
        let target = spawn_unit(&mut world, &catalog, 10.0, 20.0, Quat::from_rotation_y(0.0));
        world
            .set_unit_combat_state(attacker, CombatState::Attacking { target })
            .unwrap();
        apply_attacking_combat_facing(&mut world, attacker, target).unwrap();
        assert_forward_toward(
            world.get_unit(attacker).unwrap().placement.rotation,
            pos(10.0, 10.0),
            pos(10.0, 20.0),
        );
    }

    #[test]
    fn moving_target_updates_authoritative_attacking_facing() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let attacker = spawn_unit(&mut world, &catalog, 10.0, 10.0, Quat::from_rotation_y(0.0));
        let target = spawn_unit(&mut world, &catalog, 20.0, 10.0, Quat::from_rotation_y(0.0));
        world
            .set_unit_combat_state(attacker, CombatState::Attacking { target })
            .unwrap();
        apply_attacking_combat_facing(&mut world, attacker, target).unwrap();
        world.relocate_unit(target, pos(10.0, 20.0)).unwrap();
        apply_attacking_combat_facing(&mut world, attacker, target).unwrap();
        assert_forward_toward(
            world.get_unit(attacker).unwrap().placement.rotation,
            pos(10.0, 10.0),
            pos(10.0, 20.0),
        );
    }

    #[test]
    fn chasing_moving_unit_keeps_travel_facing_authority() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let initial = Quat::from_rotation_y(0.25);
        let attacker = spawn_unit(&mut world, &catalog, 10.0, 10.0, initial);
        let target = spawn_unit(&mut world, &catalog, 20.0, 10.0, Quat::from_rotation_y(0.0));
        world
            .set_unit_combat_state(attacker, CombatState::Chasing { target })
            .unwrap();
        world
            .set_unit_state(
                attacker,
                UnitState::Moving {
                    target: pos(30.0, 10.0),
                    path: NavigationPath::from_surface_positions(vec![pos(30.0, 10.0)]),
                    waypoint_index: 0,
                },
            )
            .unwrap();
        apply_attacking_combat_facing(&mut world, attacker, target).unwrap();
        assert_eq!(
            world.get_unit(attacker).unwrap().placement.rotation,
            initial
        );
    }

    #[test]
    fn peaceful_unit_does_not_track_former_target() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let initial = Quat::from_rotation_y(1.1);
        let unit = spawn_unit(&mut world, &catalog, 10.0, 10.0, initial);
        let target = spawn_unit(&mut world, &catalog, 20.0, 10.0, Quat::from_rotation_y(0.0));
        apply_attacking_combat_facing(&mut world, unit, target).unwrap();
        assert_eq!(world.get_unit(unit).unwrap().placement.rotation, initial);
    }

    #[test]
    fn authoritative_combat_facing_updates_before_visual_catchup() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let attacker = spawn_unit(&mut world, &catalog, 10.0, 10.0, Quat::from_rotation_y(0.0));
        let target = spawn_unit(&mut world, &catalog, 10.0, 20.0, Quat::from_rotation_y(0.0));
        world
            .set_unit_combat_state(attacker, CombatState::Attacking { target })
            .unwrap();
        apply_attacking_combat_facing(&mut world, attacker, target).unwrap();
        let authoritative = world.get_unit(attacker).unwrap().placement.rotation;
        let visual = Quat::from_rotation_y(0.0);
        let stepped =
            step_rotation_yaw_toward(visual, authoritative, 90.0_f32.to_radians(), 1.0 / 60.0);
        assert_ne!(stepped, authoritative);
        assert_forward_toward(authoritative, pos(10.0, 10.0), pos(10.0, 20.0));
    }
}
