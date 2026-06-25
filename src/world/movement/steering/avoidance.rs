//! Steering composition and movement integration (ADR-036 U11).

use bevy::prelude::Vec2;

use crate::world::{UnitCatalog, UnitId, UnitState, WorldData, WorldPosition};

use super::alignment::alignment_force;
use super::cohesion::cohesion_force;
use super::separation::separation_force;
use super::SteeringSettings;

/// Nearby unit sample for local steering.
#[derive(Debug, Clone, PartialEq)]
pub struct SteeringNeighbor {
    pub unit_id: UnitId,
    pub position_xz: Vec2,
    pub velocity_xz: Vec2,
    pub collision_radius: f32,
    pub formation_target_xz: Option<Vec2>,
}

/// Inputs for one steering evaluation tick.
#[derive(Debug, Clone, PartialEq)]
pub struct SteeringContext {
    pub unit_id: UnitId,
    pub position_xz: Vec2,
    pub path_direction_xz: Vec2,
    pub collision_radius: f32,
    pub formation_target_xz: Option<Vec2>,
    pub neighbors: Vec<SteeringNeighbor>,
    pub delta_seconds: f32,
    pub settings: SteeringSettings,
}

impl SteeringContext {
    /// Compute the steered XZ direction (unit length when path direction is non-zero).
    pub fn steered_direction_xz(&self) -> Vec2 {
        if self.neighbors.is_empty() {
            return self.path_direction_xz;
        }

        let base = if self.path_direction_xz.length_squared() > 1e-8 {
            self.path_direction_xz.normalize()
        } else {
            Vec2::ZERO
        };

        let separation = separation_force(
            self.position_xz,
            self.collision_radius,
            &self.neighbors,
            &self.settings,
        );
        let cohesion = cohesion_force(
            self.position_xz,
            self.formation_target_xz,
            &self.neighbors,
            &self.settings,
        );
        let alignment = alignment_force(&self.neighbors, &self.settings);

        let adjustment = separation + cohesion + alignment;
        if adjustment.length_squared() <= 1e-8 {
            return base;
        }

        let blended = base + adjustment * self.settings.max_steering_influence;
        if blended.length_squared() <= 1e-8 {
            return base;
        }

        clamp_angle_from(base, blended.normalize(), self.settings.max_steering_angle_radians)
    }
}

fn clamp_angle_from(base: Vec2, candidate: Vec2, max_angle_radians: f32) -> Vec2 {
    if base.length_squared() <= 1e-8 {
        return candidate;
    }
    let base = base.normalize();
    let dot = base.dot(candidate).clamp(-1.0, 1.0);
    let angle = dot.acos();
    if angle <= max_angle_radians {
        return candidate;
    }
    let cross = base.x * candidate.y - base.y * candidate.x;
    let sign = if cross >= 0.0 { 1.0 } else { -1.0 };
    rotate(base, sign * max_angle_radians)
}

fn rotate(vector: Vec2, angle_radians: f32) -> Vec2 {
    let (sin, cos) = angle_radians.sin_cos();
    Vec2::new(
        vector.x * cos - vector.y * sin,
        vector.x * sin + vector.y * cos,
    )
}

/// Build neighbor samples for steering from a radius query.
pub fn gather_steering_neighbors(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    unit_id: UnitId,
    position: WorldPosition,
    query_radius: f32,
) -> Vec<SteeringNeighbor> {
    let layout = world.layout();
    let neighbor_ids = world.query_units_in_radius(position, query_radius, Some(unit_id));

    neighbor_ids
        .into_iter()
        .filter_map(|neighbor_id| {
            let record = world.get_unit(neighbor_id)?;
            let definition = unit_catalog.get(&record.definition_id)?;
            let global = record.placement.position.to_global(layout);
            let position_xz = Vec2::new(global.x, global.z);
            let velocity_xz = unit_velocity_xz(record, layout, definition.move_speed_mps);
            let formation_target_xz = match record.state {
                UnitState::Moving { target, .. } => {
                    let target_global = target.to_global(layout);
                    Some(Vec2::new(target_global.x, target_global.z))
                }
                UnitState::Idle | UnitState::Dead => None,
            };
            Some(SteeringNeighbor {
                unit_id: neighbor_id,
                position_xz,
                velocity_xz,
                collision_radius: definition.collision_radius_meters,
                formation_target_xz,
            })
        })
        .collect()
}

fn unit_velocity_xz(
    record: &crate::world::UnitRecord,
    layout: crate::world::ChunkLayout,
    move_speed_mps: f32,
) -> Vec2 {
    let UnitState::Moving {
        path,
        waypoint_index,
        ..
    } = &record.state
    else {
        return Vec2::ZERO;
    };
    let Some(waypoint) = path.waypoints.get(*waypoint_index).copied() else {
        return Vec2::ZERO;
    };
    let current = record.placement.position.to_global(layout);
    let waypoint_global = waypoint.to_global(layout);
    let mut delta = Vec2::new(
        waypoint_global.x - current.x,
        waypoint_global.z - current.z,
    );
    if delta.length_squared() <= 1e-8 {
        return Vec2::ZERO;
    }
    delta = delta.normalize();
    delta * move_speed_mps
}

/// Apply local steering to a path-following direction.
pub fn apply_steering(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    unit_id: UnitId,
    position: WorldPosition,
    path_direction_xz: Vec2,
    collision_radius: f32,
    formation_target: WorldPosition,
    delta_seconds: f32,
    settings: &SteeringSettings,
    allow_steering: bool,
) -> Vec2 {
    if !allow_steering || path_direction_xz.length_squared() <= 1e-8 {
        return path_direction_xz;
    }
    let layout = world.layout();
    let global = position.to_global(layout);
    let position_xz = Vec2::new(global.x, global.z);
    let target_global = formation_target.to_global(layout);
    let formation_target_xz = Vec2::new(target_global.x, target_global.z);

    let neighbors = gather_steering_neighbors(
        world,
        unit_catalog,
        unit_id,
        position,
        settings.neighbor_query_radius,
    );

    let context = SteeringContext {
        unit_id,
        position_xz,
        path_direction_xz,
        collision_radius,
        formation_target_xz: Some(formation_target_xz),
        neighbors,
        delta_seconds,
        settings: *settings,
    };
    context.steered_direction_xz()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Vec3;
    use crate::world::{
        create_unit, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition,
        NavigationPath, UnitCatalog, UnitDefinitionId, UnitSource, UnitState,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(layout());
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
    fn single_unit_steering_matches_path_direction() {
        let catalog = UnitCatalog::default();
        let world = flat_world();
        let settings = SteeringSettings::default();
        let steered = apply_steering(
            &world,
            &catalog,
            UnitId::new(1),
            pos(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            0.6,
            pos(20.0, 0.0),
            0.1,
            &settings,
            true,
        );
        assert!((steered - Vec2::new(1.0, 0.0)).length() < 1e-4);
    }

    #[test]
    fn steering_output_is_deterministic() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let a = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(0.0, 0.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let b = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(0.5, 0.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        world
            .set_unit_state(
                b,
                UnitState::Moving {
                    target: pos(10.0, 0.0),
                    path: NavigationPath::new(vec![pos(10.0, 0.0)]),
                    waypoint_index: 0,
                },
            )
            .unwrap();

        let settings = SteeringSettings::default();
        let first = apply_steering(
            &world,
            &catalog,
            a,
            pos(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            0.6,
            pos(10.0, 0.0),
            0.1,
            &settings,
            true,
        );
        let second = apply_steering(
            &world,
            &catalog,
            a,
            pos(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            0.6,
            pos(10.0, 0.0),
            0.1,
            &settings,
            true,
        );
        assert_eq!(first, second);
    }

    #[test]
    fn steering_does_not_activate_without_valid_path_direction() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(0.0, 0.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        world
            .set_unit_state(
                unit_id,
                UnitState::Moving {
                    target: pos(10.0, 0.0),
                    path: NavigationPath::new(vec![pos(0.0, 0.0), pos(10.0, 0.0)]),
                    waypoint_index: 0,
                },
            )
            .unwrap();

        let settings = SteeringSettings::default();
        let steered = apply_steering(
            &world,
            &catalog,
            unit_id,
            pos(0.0, 0.0),
            Vec2::ZERO,
            0.6,
            pos(10.0, 0.0),
            0.1,
            &settings,
            false,
        );
        assert_eq!(steered, Vec2::ZERO);
    }
}
