//! Unit-unit separation steering (ADR-036 U11).

use bevy::prelude::Vec2;

use super::SteeringSettings;
use super::SteeringNeighbor;

/// Repulsion away from overlapping neighbors (XZ plane).
pub fn separation_force(
    self_xz: Vec2,
    self_radius: f32,
    neighbors: &[SteeringNeighbor],
    settings: &SteeringSettings,
) -> Vec2 {
    let mut force = Vec2::ZERO;
    for neighbor in neighbors {
        let delta = self_xz - neighbor.position_xz;
        let distance = delta.length();
        let combined = self_radius + neighbor.collision_radius + settings.separation_radius_padding;
        if distance >= combined {
            continue;
        }
        let away = if distance > settings.min_separation_distance {
            delta / distance
        } else {
            deterministic_fallback_away(self_xz, neighbor.position_xz)
        };
        let overlap = (combined - distance).max(0.0);
        let magnitude = (overlap / combined) * settings.separation_strength;
        force += away * magnitude;
    }
    clamp_force(force, settings.max_separation_force)
}

fn deterministic_fallback_away(self_xz: Vec2, neighbor_xz: Vec2) -> Vec2 {
    let delta = self_xz - neighbor_xz;
    if delta.length_squared() > 1e-8 {
        return delta.normalize();
    }
    Vec2::new(1.0, 0.0)
}

fn clamp_force(force: Vec2, max_magnitude: f32) -> Vec2 {
    let len = force.length();
    if len <= max_magnitude || len <= 1e-8 {
        force
    } else {
        force * (max_magnitude / len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::UnitId;

    fn neighbor(x: f32, z: f32, radius: f32) -> SteeringNeighbor {
        SteeringNeighbor {
            unit_id: UnitId::new(1),
            position_xz: Vec2::new(x, z),
            velocity_xz: Vec2::ZERO,
            collision_radius: radius,
            formation_target_xz: None,
        }
    }

    #[test]
    fn separation_pushes_overlapping_units_apart() {
        let settings = SteeringSettings::default();
        let force = separation_force(
            Vec2::new(0.0, 0.0),
            0.6,
            &[neighbor(0.1, 0.0, 0.6)],
            &settings,
        );
        assert!(force.x < 0.0);
        assert!(force.length() > 0.0);
    }

    #[test]
    fn no_separation_when_beyond_combined_radius() {
        let settings = SteeringSettings::default();
        let force = separation_force(
            Vec2::new(0.0, 0.0),
            0.6,
            &[neighbor(5.0, 0.0, 0.6)],
            &settings,
        );
        assert_eq!(force, Vec2::ZERO);
    }
}
