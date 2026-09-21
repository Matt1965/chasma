//! Derived foundation skirt specification (presentation only).

use bevy::prelude::*;

use super::mode::{
    FOUNDATION_SKIRT_MIN_DEPTH, MAX_FOUNDATION_VISIBLE_DEPTH_METERS,
    foundation_slope_run_per_meter_drop,
};
use super::sample::{TerrainFootprintReport, TerrainFootprintSample, slope_degrees_from_plane_coefficients};
use crate::world::occupancy::FootprintShape;

/// One perimeter vertex for a derived foundation skirt (anchor-local space).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoundationPerimeterVertex {
    pub local_xz: Vec2,
    /// Terrain height in authoritative simulation space.
    pub terrain_world_y: f32,
}

/// Presentation-only foundation derived from footprint + floor elevation + terrain.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundationSkirtSpec {
    pub perimeter: Vec<FoundationPerimeterVertex>,
    /// Authoritative floor elevation (simulation Y).
    pub floor_world_y: f32,
    /// Maximum drop from floor to terrain under footprint (simulation meters).
    pub max_depth_meters: f32,
    /// Visible foundation depth used for skirt generation decisions (render meters).
    pub presentation_depth_meters: f32,
    /// Maximum outward expansion of the bottom perimeter (presentation meters).
    pub outward_expansion_meters: f32,
}

/// Horizontal span used to estimate visible slope depth across a footprint.
pub fn footprint_horizontal_span_meters(shape: &FootprintShape) -> f32 {
    match shape {
        FootprintShape::Rectangle {
            width_meters,
            depth_meters,
        } => width_meters.hypot(*depth_meters),
        FootprintShape::Circle { radius_meters } => radius_meters * 2.0,
        FootprintShape::Ellipse {
            radius_x_meters,
            radius_z_meters,
        } => radius_x_meters.hypot(*radius_z_meters) * 2.0,
        FootprintShape::BakedCellMask(mask) => {
            let width = mask.width_cells as f32 * mask.cell_size_meters;
            let depth = mask.depth_cells as f32 * mask.cell_size_meters;
            width.hypot(depth)
        }
    }
}

/// Visible foundation depth from simulation terrain + presentation vertical scale.
pub fn presentation_foundation_depth_meters(
    terrain: &TerrainFootprintReport,
    footprint_span_meters: f32,
    vertical_scale: f32,
) -> f32 {
    let scale = vertical_scale.max(1.0);
    let sample_presentation = terrain.height_range * scale;
    let slope_degrees = slope_degrees_from_plane_coefficients(
        terrain.plane_a * scale,
        terrain.plane_b * scale,
    );
    let slope_presentation = footprint_span_meters * slope_degrees.to_radians().tan();
    sample_presentation.max(slope_presentation)
}

impl FoundationSkirtSpec {
    pub fn from_level_placement(
        anchor_global_xz: Vec2,
        floor_world_y: f32,
        perimeter: &[TerrainFootprintSample],
        interior_min_height: f32,
        yaw_radians: f32,
        terrain: &TerrainFootprintReport,
        footprint_span_meters: f32,
        terrain_vertical_scale: f32,
    ) -> Option<Self> {
        let max_depth = floor_world_y - interior_min_height;
        let presentation_depth =
            presentation_foundation_depth_meters(terrain, footprint_span_meters, terrain_vertical_scale);
        if presentation_depth < FOUNDATION_SKIRT_MIN_DEPTH {
            return None;
        }
        if presentation_depth > MAX_FOUNDATION_VISIBLE_DEPTH_METERS {
            return None;
        }

        let outward_expansion_meters = presentation_depth * foundation_slope_run_per_meter_drop();
        let (sin, cos) = yaw_radians.sin_cos();
        let perimeter_vertices: Vec<FoundationPerimeterVertex> = perimeter
            .iter()
            .map(|sample| {
                let world = sample.global_xz - anchor_global_xz;
                let local_x = world.x * cos + world.y * sin;
                let local_z = -world.x * sin + world.y * cos;
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(local_x, local_z),
                    terrain_world_y: sample.height,
                }
            })
            .collect();

        if perimeter_vertices.is_empty() {
            return None;
        }

        Some(Self {
            perimeter: perimeter_vertices,
            floor_world_y,
            max_depth_meters: max_depth,
            presentation_depth_meters: presentation_depth,
            outward_expansion_meters,
        })
    }
}
