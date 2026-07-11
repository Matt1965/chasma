//! Standoff destination for chase positioning (ADR-057 C4).

use bevy::prelude::Vec2;

#[cfg(test)]
use crate::world::navigation::xz_distance;
use crate::world::{WorldData, WorldPosition, ground_world_position};

use super::range::RangeCheck;

/// Why standoff placement failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandoffError {
    TerrainUnavailable,
}

/// Compute a grounded standoff point along target→attacker at weapon edge range.
pub fn compute_standoff_destination(
    world: &WorldData,
    attacker_pos: WorldPosition,
    target_pos: WorldPosition,
    check: &RangeCheck,
) -> Result<WorldPosition, StandoffError> {
    let layout = world.layout();
    let target_global = target_pos.to_global(layout);
    let attacker_global = attacker_pos.to_global(layout);
    let mut direction = Vec2::new(
        attacker_global.x - target_global.x,
        attacker_global.z - target_global.z,
    );
    if direction.length_squared() <= 1e-8 {
        direction = Vec2::X;
    } else {
        direction = direction.normalize();
    }

    let desired_center_distance =
        check.weapon_range_meters + check.attacker_radius_meters + check.target_radius_meters;
    let destination_global = bevy::prelude::Vec3::new(
        target_global.x + direction.x * desired_center_distance,
        attacker_global.y,
        target_global.z + direction.y * desired_center_distance,
    );
    let candidate = WorldPosition::from_global(destination_global, layout);
    ground_world_position(world, candidate).ok_or(StandoffError::TerrainUnavailable)
}

/// Verify standoff places attacker on the correct side of the target at weapon reach.
#[cfg(test)]
pub(crate) fn standoff_center_distance_matches_weapon_range(
    world: &WorldData,
    _attacker_pos: WorldPosition,
    target_pos: WorldPosition,
    check: &RangeCheck,
    standoff: WorldPosition,
) -> bool {
    let center = xz_distance(standoff, target_pos, world.layout());
    let edge = super::range::edge_distance_meters(
        center,
        check.attacker_radius_meters,
        check.target_radius_meters,
    );
    (edge - check.weapon_range_meters).abs() < 0.05
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::combat::range::RangeCheck;
    use crate::world::{ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition};
    use bevy::prelude::Vec3;

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
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

    #[test]
    fn standoff_is_on_attacker_side_at_weapon_reach() {
        let world = flat_world();
        let attacker = pos(20.0, 10.0);
        let target = pos(10.0, 10.0);
        let check = RangeCheck {
            center_distance_meters: 10.0,
            edge_distance_meters: 8.0,
            weapon_range_meters: 1.5,
            attacker_radius_meters: 0.6,
            target_radius_meters: 0.45,
        };
        let standoff = compute_standoff_destination(&world, attacker, target, &check).unwrap();
        assert!(standoff_center_distance_matches_weapon_range(
            &world, attacker, target, &check, standoff
        ));
        let standoff_x = standoff.to_global(world.layout()).x;
        assert!(standoff_x > target.to_global(world.layout()).x);
    }

    #[test]
    fn missing_terrain_blocks_standoff() {
        let world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let check = RangeCheck {
            center_distance_meters: 10.0,
            edge_distance_meters: 8.0,
            weapon_range_meters: 1.5,
            attacker_radius_meters: 0.6,
            target_radius_meters: 0.45,
        };
        assert_eq!(
            compute_standoff_destination(&world, pos(20.0, 10.0), pos(10.0, 10.0), &check),
            Err(StandoffError::TerrainUnavailable)
        );
    }
}
