//! Minimal velocity alignment steering (ADR-036 U11).

use bevy::prelude::Vec2;

use super::SteeringSettings;
use super::SteeringNeighbor;

/// Very weak bias toward neighbors' movement directions.
pub fn alignment_force(neighbors: &[SteeringNeighbor], settings: &SteeringSettings) -> Vec2 {
    if neighbors.is_empty() {
        return Vec2::ZERO;
    }

    let mut average = Vec2::ZERO;
    let mut count = 0_u32;
    for neighbor in neighbors {
        if neighbor.velocity_xz.length_squared() > 1e-8 {
            average += neighbor.velocity_xz.normalize();
            count += 1;
        }
    }

    if count == 0 {
        return Vec2::ZERO;
    }

    average /= count as f32;
    if average.length_squared() <= 1e-8 {
        return Vec2::ZERO;
    }
    average.normalize() * settings.alignment_strength
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::UnitId;

    #[test]
    fn alignment_is_weak_and_zero_without_neighbor_velocity() {
        let settings = SteeringSettings::default();
        let neighbors = [SteeringNeighbor {
            unit_id: UnitId::new(1),
            position_xz: Vec2::ZERO,
            velocity_xz: Vec2::ZERO,
            collision_radius: 0.6,
            formation_target_xz: None,
        }];
        assert_eq!(alignment_force(&neighbors, &settings), Vec2::ZERO);
    }
}
