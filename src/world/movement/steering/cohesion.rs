//! Light formation cohesion steering (ADR-036 U11).

use bevy::prelude::Vec2;

use super::SteeringSettings;
use super::SteeringNeighbor;

/// Weak pull toward the local formation center derived from neighbor move targets.
pub fn cohesion_force(
    self_xz: Vec2,
    self_target_xz: Option<Vec2>,
    neighbors: &[SteeringNeighbor],
    settings: &SteeringSettings,
) -> Vec2 {
    let mut center = Vec2::ZERO;
    let mut count = 0_u32;

    if let Some(target) = self_target_xz {
        center += target;
        count += 1;
    }

    for neighbor in neighbors {
        if let Some(target) = neighbor.formation_target_xz {
            center += target;
            count += 1;
        }
    }

    if count == 0 {
        return Vec2::ZERO;
    }

    center /= count as f32;
    let to_center = center - self_xz;
    if to_center.length_squared() <= settings.cohesion_arrival_threshold_sq {
        return Vec2::ZERO;
    }
    to_center.normalize() * settings.cohesion_strength
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::UnitId;

    fn neighbor_with_target(x: f32, z: f32, target_x: f32, target_z: f32) -> SteeringNeighbor {
        SteeringNeighbor {
            unit_id: UnitId::new(2),
            position_xz: Vec2::new(x, z),
            velocity_xz: Vec2::ZERO,
            collision_radius: 0.6,
            formation_target_xz: Some(Vec2::new(target_x, target_z)),
        }
    }

    #[test]
    fn cohesion_pulls_toward_group_target_centroid() {
        let settings = SteeringSettings::default();
        let force = cohesion_force(
            Vec2::new(0.0, 0.0),
            Some(Vec2::new(10.0, 0.0)),
            &[neighbor_with_target(1.0, 0.0, 10.0, 2.0)],
            &settings,
        );
        assert!(force.x > 0.0);
    }
}
