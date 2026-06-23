//! Formation offset spacing helpers (ADR-035 U10).

use bevy::prelude::Vec2;

use crate::world::{ChunkLayout, UnitId, WorldPosition};

/// Minimum center-to-center spacing when catalog radii are very small (meters).
pub const FORMATION_MIN_SPACING_METERS: f32 = 1.0;

/// Maximum radial jitter applied to ring slots (meters).
pub const FORMATION_JITTER_METERS: f32 = 0.15;

/// XZ offset from the formation center in global space (meters).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormationOffset {
    pub xz: Vec2,
}

impl FormationOffset {
    pub const ZERO: Self = Self { xz: Vec2::ZERO };

    pub const fn new(x: f32, z: f32) -> Self {
        Self {
            xz: Vec2::new(x, z),
        }
    }
}

/// Minimum spacing between adjacent formation slots for a unit type.
pub fn unit_spacing_meters(collision_radius_meters: f32) -> f32 {
    (collision_radius_meters * 2.0).max(FORMATION_MIN_SPACING_METERS)
}

/// Deterministic micro-jitter so repeated groups do not stack on identical slots.
pub fn formation_jitter(unit_id: UnitId, target: WorldPosition, layout: ChunkLayout) -> Vec2 {
    let global = target.to_global(layout);
    let mut hash = unit_id.raw().wrapping_mul(0x9E37_79B9_7F4A_7C15);
    hash ^= (global.x.to_bits() as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
    hash ^= (global.z.to_bits() as u64).wrapping_mul(0x6c07_8965_e494_4629);
    let t = (hash as f32 / u64::MAX as f32) * std::f32::consts::TAU;
    Vec2::new(t.cos(), t.sin()) * FORMATION_JITTER_METERS
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Vec3;
    use crate::world::{ChunkCoord, LocalPosition};

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

    #[test]
    fn spacing_respects_collision_radius() {
        assert!((unit_spacing_meters(0.2) - FORMATION_MIN_SPACING_METERS).abs() < 1e-4);
        assert!((unit_spacing_meters(0.6) - 1.2).abs() < 1e-4);
    }

    #[test]
    fn jitter_is_deterministic_for_same_inputs() {
        let target = pos(40.0, 40.0);
        let a = formation_jitter(UnitId::new(7), target, layout());
        let b = formation_jitter(UnitId::new(7), target, layout());
        assert_eq!(a, b);
    }
}
