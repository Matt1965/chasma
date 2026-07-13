//! Occupancy cell coordinates and resolution contract (ADR-080 B3).
//!
//! Occupancy cells are **2 m** on a side. The navigation grid defaults to **4 m**,
//! so each navigation cell spans a 2×2 occupancy-cell block deterministically.

use bevy::prelude::*;

use crate::world::ChunkCoord;
use crate::world::ChunkLayout;
use crate::world::WorldPosition;

/// Horizontal occupancy cell size (meters). Shared by baker, registration, and queries.
pub const OCCUPANCY_CELL_SIZE_METERS: f32 = 2.0;

/// Default surface space for B3. Multi-space baking arrives in B6.
pub const SURFACE_SPACE_ID: u32 = 0;

/// Maximum baked mask dimension per axis (safety guard).
pub const MAX_MASK_CELLS_PER_AXIS: u32 = 512;

/// Integer occupancy cell in global XZ space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct OccupancyCellCoord {
    pub x: i32,
    pub z: i32,
}

impl OccupancyCellCoord {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    /// Cell center in global XZ (Y = 0).
    pub fn center_global(self) -> Vec2 {
        let size = OCCUPANCY_CELL_SIZE_METERS;
        Vec2::new(
            self.x as f32 * size + size * 0.5,
            self.z as f32 * size + size * 0.5,
        )
    }

    /// Inclusive cell AABB in global XZ.
    pub fn bounds_global(self) -> (Vec2, Vec2) {
        let size = OCCUPANCY_CELL_SIZE_METERS;
        let min = Vec2::new(self.x as f32 * size, self.z as f32 * size);
        let max = min + Vec2::splat(size);
        (min, max)
    }
}

/// Convert global XZ to the containing occupancy cell.
pub fn occupancy_cell_at_global_xz(global_xz: Vec2) -> OccupancyCellCoord {
    let size = OCCUPANCY_CELL_SIZE_METERS;
    OccupancyCellCoord::new(
        (global_xz.x / size).floor() as i32,
        (global_xz.y / size).floor() as i32,
    )
}

/// Owning chunk for an occupancy cell center (for chunk-keyed storage).
pub fn chunk_for_occupancy_cell(cell: OccupancyCellCoord, layout: ChunkLayout) -> ChunkCoord {
    let center = cell.center_global();
    WorldPosition::from_global(Vec3::new(center.x, 0.0, center.y), layout).chunk
}

/// Quantized yaw in 90-degree steps accepted by B3 footprints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuantizedRotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl QuantizedRotation {
    pub fn degrees(self) -> u16 {
        match self {
            Self::Deg0 => 0,
            Self::Deg90 => 90,
            Self::Deg180 => 180,
            Self::Deg270 => 270,
        }
    }

    pub fn radians(self) -> f32 {
        self.degrees() as f32 * std::f32::consts::PI / 180.0
    }

    pub fn from_degrees_snapped(yaw_degrees: f32) -> Result<Self, super::OccupancyError> {
        let normalized = yaw_degrees.rem_euclid(360.0);
        let snapped = ((normalized / 90.0).round() as i32).rem_euclid(4);
        let expected = snapped as f32 * 90.0;
        if (normalized - expected).abs() > 0.5 && (normalized - expected - 360.0).abs() > 0.5 {
            return Err(super::OccupancyError::InvalidRotation {
                yaw_degrees: normalized,
            });
        }
        Ok(match snapped {
            0 => Self::Deg0,
            1 => Self::Deg90,
            2 => Self::Deg180,
            3 => Self::Deg270,
            _ => Self::Deg0,
        })
    }

    pub fn from_quat(rotation: Quat) -> Result<Self, super::OccupancyError> {
        let tilt_sq = rotation.x * rotation.x + rotation.z * rotation.z;
        if tilt_sq > 0.0001 {
            return Err(super::OccupancyError::InvalidRotation {
                yaw_degrees: rotation.to_euler(EulerRot::YXZ).1.to_degrees(),
            });
        }
        let yaw = 2.0 * rotation.y.atan2(rotation.w);
        Self::from_degrees_snapped(yaw.to_degrees())
    }
}

/// Whether a circle intersects an axis-aligned cell (conservative for registration).
pub fn circle_intersects_cell(
    circle_center: Vec2,
    circle_radius: f32,
    cell: OccupancyCellCoord,
) -> bool {
    let (min, max) = cell.bounds_global();
    let closest = Vec2::new(
        circle_center.x.clamp(min.x, max.x),
        circle_center.y.clamp(min.y, max.y),
    );
    closest.distance(circle_center) <= circle_radius
}

/// Inclusive circle overlap for agent queries (REVIEW-B6 / ADR-031 parity).
pub fn circle_overlap_blocked(
    center_a: Vec2,
    center_b: Vec2,
    radius_a: f32,
    radius_b: f32,
) -> bool {
    center_a.distance(center_b) <= radius_a + radius_b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occupancy_cells_align_with_navigation_grid() {
        let nav_spacing = 4.0;
        let occ = OCCUPANCY_CELL_SIZE_METERS;
        assert!((nav_spacing / occ).fract() < f32::EPSILON);
        assert_eq!(nav_spacing / occ, 2.0);
    }

    #[test]
    fn from_quat_rejects_oblique_yaw() {
        assert!(
            QuantizedRotation::from_quat(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4))
                .is_err()
        );
    }

    #[test]
    fn quantize_rotation_rejects_free_yaw() {
        assert!(QuantizedRotation::from_degrees_snapped(45.0).is_err());
        assert_eq!(
            QuantizedRotation::from_degrees_snapped(90.0).unwrap(),
            QuantizedRotation::Deg90
        );
    }
}
