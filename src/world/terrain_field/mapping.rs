//! World position to terrain field tile sample mapping (ADR-101).

use bevy::prelude::*;

use super::contract::TERRAIN_FIELD_SAMPLE_SPACING_METERS;
use super::sample::TerrainFieldInterpolationDebug;
use crate::world::{ChunkLayout, WorldPosition};

/// Local field-grid coordinates within a chunk tile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldLocalSampleCoord {
    pub col: u32,
    pub row: u32,
    pub frac_x: u8,
    pub frac_z: u8,
}

/// Map authoritative world position to chunk-local field sample coordinates.
pub fn world_position_to_field_local(
    position: WorldPosition,
    layout: ChunkLayout,
) -> Result<(Vec2, FieldLocalSampleCoord), FieldMappingError> {
    let size = layout.chunk_size_units();
    let local = position.local.0;
    if local.x < -1e-4 || local.z < -1e-4 || local.x > size + 1e-4 || local.z > size + 1e-4 {
        return Err(FieldMappingError::OutsideChunkDomain);
    }
    let local_xz = Vec2::new(local.x.clamp(0.0, size), local.z.clamp(0.0, size));
    let spacing = TERRAIN_FIELD_SAMPLE_SPACING_METERS;
    let intervals = (size / spacing).round() as u32;
    let max_vertex = intervals;
    let continuous_col = local_xz.x / spacing;
    let continuous_row = local_xz.y / spacing;
    let col = continuous_col.floor() as u32;
    let row = continuous_row.floor() as u32;
    let col = col.min(max_vertex);
    let row = row.min(max_vertex);
    let frac_x = if col >= max_vertex {
        0
    } else {
        fraction_to_q8(continuous_col - col as f32)
    };
    let frac_z = if row >= max_vertex {
        0
    } else {
        fraction_to_q8(continuous_row - row as f32)
    };
    Ok((
        local_xz,
        FieldLocalSampleCoord {
            col,
            row,
            frac_x,
            frac_z,
        },
    ))
}

pub fn fraction_to_q8(fraction: f32) -> u8 {
    if fraction <= 0.0 {
        0
    } else if fraction >= 1.0 {
        255
    } else {
        (fraction * 256.0).round().clamp(0.0, 255.0) as u8
    }
}

pub fn field_local_to_debug(coord: FieldLocalSampleCoord) -> TerrainFieldInterpolationDebug {
    TerrainFieldInterpolationDebug {
        col: coord.col,
        row: coord.row,
        frac_x: coord.frac_x,
        frac_z: coord.frac_z,
        corner_values: [0; 4],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldMappingError {
    OutsideChunkDomain,
}

impl std::fmt::Display for FieldMappingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutsideChunkDomain => write!(f, "position outside chunk field domain"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition, WorldPosition};

    #[test]
    fn origin_maps_to_zero() {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let pos = WorldPosition::new(ChunkCoord::new(0, 0), LocalPosition::new(Vec3::ZERO));
        let (_, coord) = world_position_to_field_local(pos, layout).unwrap();
        assert_eq!(coord.col, 0);
        assert_eq!(coord.row, 0);
        assert_eq!(coord.frac_x, 0);
        assert_eq!(coord.frac_z, 0);
    }
}
