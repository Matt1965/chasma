//! Deterministic offline resampling for field import (ADR-102).

use super::super::source::import_config::{TerrainFieldImageOrientation, TerrainFieldResampling};
use super::super::source::remap::TerrainFieldValueRemap;
use super::super::source_error::TerrainFieldSourceError;
use super::png::DecodedFieldImage;

const FP_SCALE: u64 = 256;

/// Resample a decoded image to target shared-edge dimensions and remap to `u16`.
pub fn resample_imported_image(
    image: &DecodedFieldImage,
    target_width: u32,
    target_height: u32,
    orientation: TerrainFieldImageOrientation,
    resampling: TerrainFieldResampling,
    remap: &TerrainFieldValueRemap,
    allow_aspect_stretch: bool,
    image_aspect: f32,
    world_aspect: f32,
) -> Result<Vec<u16>, TerrainFieldSourceError> {
    if target_width < 2 || target_height < 2 {
        return Err(TerrainFieldSourceError::SourceImageDimensionMismatch {
            expected: (target_width, target_height),
            found: (image.width, image.height),
        });
    }
    if !allow_aspect_stretch && (image_aspect - world_aspect).abs() > 1e-3 {
        return Err(TerrainFieldSourceError::SourceImageAspectMismatch {
            image_aspect,
            world_aspect,
        });
    }

    let mut out = Vec::with_capacity((target_width * target_height) as usize);
    for row in 0..target_height {
        for col in 0..target_width {
            let raw = sample_source(
                image,
                col,
                row,
                target_width,
                target_height,
                orientation,
                resampling,
            )?;
            out.push(remap.apply(raw)?);
        }
    }
    Ok(out)
}

fn sample_source(
    image: &DecodedFieldImage,
    col: u32,
    row: u32,
    target_width: u32,
    target_height: u32,
    orientation: TerrainFieldImageOrientation,
    resampling: TerrainFieldResampling,
) -> Result<u32, TerrainFieldSourceError> {
    let max_x = target_width - 1;
    let max_z = target_height - 1;
    let src_x_fp = if max_x == 0 {
        0
    } else {
        (col as u64) * (image.width - 1) as u64 * FP_SCALE / max_x as u64
    };
    let src_z_fp = if max_z == 0 {
        0
    } else {
        (row as u64) * (image.height - 1) as u64 * FP_SCALE / max_z as u64
    };
    let src_z_fp = match orientation {
        TerrainFieldImageOrientation::RowZeroIsMinimumZ => src_z_fp,
        TerrainFieldImageOrientation::RowZeroIsMaximumZ => {
            (image.height - 1) as u64 * FP_SCALE - src_z_fp
        }
    };

    match resampling {
        TerrainFieldResampling::Nearest => {
            let sx = nearest_index(src_x_fp, image.width);
            let sz = nearest_index(src_z_fp, image.height);
            Ok(image.sample(sx, sz))
        }
        TerrainFieldResampling::Bilinear => bilinear_sample_image(image, src_x_fp, src_z_fp),
    }
}

fn nearest_index(fp_coord: u64, size: u32) -> u32 {
    let max = (size - 1) as u64;
    let idx = (fp_coord + FP_SCALE / 2) / FP_SCALE;
    idx.min(max) as u32
}

fn bilinear_sample_image(
    image: &DecodedFieldImage,
    src_x: u64,
    src_z: u64,
) -> Result<u32, TerrainFieldSourceError> {
    let max_col = image.width.saturating_sub(2) as u64;
    let max_row = image.height.saturating_sub(2) as u64;
    let col = (src_x / FP_SCALE).min(max_col) as u32;
    let row = (src_z / FP_SCALE).min(max_row) as u32;
    let frac_x = (src_x % FP_SCALE) as u32;
    let frac_z = (src_z % FP_SCALE) as u32;
    let inv_x = FP_SCALE - frac_x as u64;
    let inv_z = FP_SCALE - frac_z as u64;

    let c00 = image.sample(col, row) as u64;
    let c10 = image.sample(col + 1, row) as u64;
    let c01 = image.sample(col, row + 1) as u64;
    let c11 = image.sample(col + 1, row + 1) as u64;

    let w00 = inv_x * inv_z;
    let w10 = frac_x as u64 * inv_z;
    let w01 = inv_x * frac_z as u64;
    let w11 = frac_x as u64 * frac_z as u64;
    let sum = c00 * w00 + c10 * w10 + c01 * w01 + c11 * w11;
    let denom = FP_SCALE * FP_SCALE;
    let rounded = (sum + denom / 2) / denom;
    Ok(rounded as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain_field::import::png::expand_u8_to_u16;

    fn gradient_image(w: u32, h: u32) -> DecodedFieldImage {
        let mut samples = Vec::new();
        for row in 0..h {
            for col in 0..w {
                let t = (col * 255 / (w - 1)) as u8;
                let _ = row;
                samples.push(expand_u8_to_u16(t) as u32);
            }
        }
        DecodedFieldImage {
            width: w,
            height: h,
            samples,
        }
    }

    #[test]
    fn bilinear_half_value() {
        let image = DecodedFieldImage {
            width: 2,
            height: 2,
            samples: vec![0, 10_000, 0, 10_000],
        };
        let remap = TerrainFieldValueRemap::full_range();
        let out = resample_imported_image(
            &image,
            3,
            3,
            TerrainFieldImageOrientation::RowZeroIsMinimumZ,
            TerrainFieldResampling::Bilinear,
            &remap,
            true,
            1.0,
            1.0,
        )
        .unwrap();
        assert!(out[4] > 4_000 && out[4] < 6_000);
    }

    #[test]
    fn maximum_z_orientation_flips_rows() {
        let image = DecodedFieldImage {
            width: 2,
            height: 2,
            samples: vec![
                expand_u8_to_u16(33) as u32,
                expand_u8_to_u16(44) as u32,
                expand_u8_to_u16(11) as u32,
                expand_u8_to_u16(22) as u32,
            ],
        };
        let remap = TerrainFieldValueRemap::full_range();
        let out = resample_imported_image(
            &image,
            2,
            2,
            TerrainFieldImageOrientation::RowZeroIsMaximumZ,
            TerrainFieldResampling::Nearest,
            &remap,
            true,
            1.0,
            1.0,
        )
        .unwrap();
        assert_eq!(out[0], expand_u8_to_u16(11));
        assert_eq!(out[2], expand_u8_to_u16(33));
    }

    #[test]
    fn rejects_aspect_mismatch() {
        let image = gradient_image(4, 4);
        let remap = TerrainFieldValueRemap::full_range();
        assert!(matches!(
            resample_imported_image(
                &image,
                5,
                3,
                TerrainFieldImageOrientation::RowZeroIsMinimumZ,
                TerrainFieldResampling::Bilinear,
                &remap,
                false,
                1.0,
                2.0,
            ),
            Err(TerrainFieldSourceError::SourceImageAspectMismatch { .. })
        ));
    }
}
