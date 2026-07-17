//! Deterministic fixed-point bilinear interpolation (ADR-101).

use super::error::TerrainFieldQueryError;
use super::mapping::FieldLocalSampleCoord;
use super::sample::TerrainFieldInterpolationDebug;
use super::tile::TerrainFieldTile;

const INTERP_SCALE: u64 = 256;

/// Bilinear sample from one tile using quantized fractions in 0..=255.
pub fn bilinear_sample_u16(
    tile: &TerrainFieldTile,
    coord: FieldLocalSampleCoord,
) -> Result<(u16, TerrainFieldInterpolationDebug), TerrainFieldQueryError> {
    let col = coord.col;
    let row = coord.row;
    let c00 = tile
        .sample_at_vertex(col, row)
        .ok_or(TerrainFieldQueryError::InvalidWorldCoordinate)?;

    if col + 1 >= tile.samples_per_edge as u32 || row + 1 >= tile.samples_per_edge as u32 {
        return Ok((
            c00,
            TerrainFieldInterpolationDebug {
                col,
                row,
                frac_x: coord.frac_x,
                frac_z: coord.frac_z,
                corner_values: [c00, c00, c00, c00],
            },
        ));
    }

    let c10 = tile
        .sample_at_vertex(col + 1, row)
        .ok_or(TerrainFieldQueryError::InvalidWorldCoordinate)?;
    let c01 = tile
        .sample_at_vertex(col, row + 1)
        .ok_or(TerrainFieldQueryError::InvalidWorldCoordinate)?;
    let c11 = tile
        .sample_at_vertex(col + 1, row + 1)
        .ok_or(TerrainFieldQueryError::InvalidWorldCoordinate)?;

    let fx = coord.frac_x as u64;
    let fz = coord.frac_z as u64;
    let inv_fx = INTERP_SCALE - fx;
    let inv_fz = INTERP_SCALE - fz;

    let w00 = inv_fx * inv_fz;
    let w10 = fx * inv_fz;
    let w01 = inv_fx * fz;
    let w11 = fx * fz;
    let sum = c00 as u64 * w00 + c10 as u64 * w10 + c01 as u64 * w01 + c11 as u64 * w11;
    let denom = INTERP_SCALE * INTERP_SCALE;
    let rounded = (sum + denom / 2) / denom;
    if rounded > u16::MAX as u64 {
        return Err(TerrainFieldQueryError::FixedPointInterpolationOverflow);
    }

    Ok((
        rounded as u16,
        TerrainFieldInterpolationDebug {
            col,
            row,
            frac_x: coord.frac_x,
            frac_z: coord.frac_z,
            corner_values: [c00, c10, c01, c11],
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ChunkCoord;
    use crate::world::terrain_field::tile::TerrainFieldTile;

    #[test]
    fn constant_field_returns_constant() {
        let tile = TerrainFieldTile::new_constant(ChunkCoord::new(0, 0), 20_000, "t");
        let (value, _) = bilinear_sample_u16(
            &tile,
            FieldLocalSampleCoord {
                col: 1,
                row: 1,
                frac_x: 128,
                frac_z: 64,
            },
        )
        .unwrap();
        assert_eq!(value, 20_000);
    }

    #[test]
    fn half_interpolation_between_corners() {
        let mut tile = TerrainFieldTile::new_constant(ChunkCoord::new(0, 0), 0, "t");
        tile.samples[0] = 0;
        tile.samples[1] = 10_000;
        tile.samples[tile.samples_per_edge as usize] = 0;
        tile.samples[tile.samples_per_edge as usize + 1] = 10_000;
        let (value, _) = bilinear_sample_u16(
            &tile,
            FieldLocalSampleCoord {
                col: 0,
                row: 0,
                frac_x: 128,
                frac_z: 128,
            },
        )
        .unwrap();
        assert!((value as i32 - 5000).abs() < 50);
    }
}
