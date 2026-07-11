use bevy::prelude::*;

use super::TerrainDataError;

/// The authoritative height data for a single chunk (ADR-003, ADR-008).
///
/// Samples are stored row-major in a flat `f32` buffer of length
/// `samples_per_edge * samples_per_edge`. Column index advances along +X and row
/// index advances along +Z, consistent with the XZ grid and minimum-corner chunk
/// origin (ADR-001 addendum, ADR-006).
///
/// Adjacent chunks share their boundary samples: a chunk stores `N + 1` samples
/// per edge where `N = chunk_size / spacing`, so neighboring tiles meet exactly
/// (ADR-008). Heights are authoritative and never quantized.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct Heightfield {
    samples_per_edge: u32,
    spacing_meters: f32,
    samples: Vec<f32>,
}

impl Heightfield {
    /// Build a heightfield from raw row-major samples.
    ///
    /// This is independent of any file format, so import, synthetic data, and
    /// tests can all construct heightfields directly (ADR-009).
    pub fn from_samples(
        samples_per_edge: u32,
        spacing_meters: f32,
        samples: Vec<f32>,
    ) -> Result<Self, TerrainDataError> {
        if samples_per_edge < 2 {
            return Err(TerrainDataError::TooSmall { samples_per_edge });
        }
        if !(spacing_meters > 0.0) || !spacing_meters.is_finite() {
            return Err(TerrainDataError::NonPositiveSpacing { spacing_meters });
        }
        let expected = (samples_per_edge as usize) * (samples_per_edge as usize);
        if samples.len() != expected {
            return Err(TerrainDataError::SampleCountMismatch {
                expected,
                actual: samples.len(),
            });
        }
        for (index, sample) in samples.iter().enumerate() {
            if !sample.is_finite() {
                return Err(TerrainDataError::NonFiniteHeightSample { index });
            }
        }
        Ok(Self {
            samples_per_edge,
            spacing_meters,
            samples,
        })
    }

    pub fn samples_per_edge(&self) -> u32 {
        self.samples_per_edge
    }

    pub fn spacing_meters(&self) -> f32 {
        self.spacing_meters
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Height at a grid vertex (column `col`, row `row`).
    pub fn height_at_vertex(&self, col: u32, row: u32) -> f32 {
        let stride = self.samples_per_edge as usize;
        self.samples[row as usize * stride + col as usize]
    }

    /// Column `col` across all rows (for seam normal stitching).
    pub fn column_heights(&self, col: u32) -> Vec<f32> {
        let spe = self.samples_per_edge as usize;
        let col = col as usize;
        (0..spe).map(|row| self.samples[row * spe + col]).collect()
    }

    /// Row `row` across all columns (for seam normal stitching).
    pub fn row_heights(&self, row: u32) -> Vec<f32> {
        let spe = self.samples_per_edge as usize;
        let row = row as usize;
        (0..spe).map(|col| self.samples[row * spe + col]).collect()
    }

    /// Chunk edge length in world units, derived from the sample grid.
    pub fn chunk_size_meters(&self) -> f32 {
        (self.samples_per_edge - 1) as f32 * self.spacing_meters
    }

    /// Whether chunk-local XZ lies inside the heightfield domain `[0, chunk_size]`.
    pub fn is_within_domain(&self, local_x: f32, local_z: f32) -> bool {
        let size = self.chunk_size_meters();
        local_x >= -1e-4 && local_z >= -1e-4 && local_x <= size + 1e-4 && local_z <= size + 1e-4
    }

    /// Sample height when `local_x` / `local_z` are inside the chunk domain.
    ///
    /// Simulation callers must use this (or [`super::try_sample_height_at_position`]) instead
    /// of [`Self::sample`] to avoid silent edge clamping (REVIEW-B4).
    pub fn try_sample(&self, local_x: f32, local_z: f32) -> Result<f32, super::TerrainQueryError> {
        if !self.is_within_domain(local_x, local_z) {
            return Err(super::TerrainQueryError::InvalidTerrainCoordinate);
        }
        Ok(self.sample(local_x, local_z))
    }

    fn height(&self, col: i32, row: i32) -> f32 {
        let stride = self.samples_per_edge as usize;
        self.samples[row as usize * stride + col as usize]
    }

    /// Sample the height at a chunk-relative position via bilinear
    /// interpolation (ADR-008).
    ///
    /// `local_x` and `local_z` are in world units relative to the chunk's
    /// minimum corner and are clamped to the chunk's `[0, chunk_size]` domain.
    ///
    /// **Simulation code** must use [`Self::try_sample`] or
    /// [`super::try_sample_height_at_position`] instead — clamping is render/import
    /// convenience only (REVIEW-B4).
    pub fn sample(&self, local_x: f32, local_z: f32) -> f32 {
        let last = self.samples_per_edge as i32 - 1;
        let size = self.chunk_size_meters();

        let lx = local_x.clamp(0.0, size);
        let lz = local_z.clamp(0.0, size);

        let gx = lx / self.spacing_meters;
        let gz = lz / self.spacing_meters;

        let col0 = (gx.floor() as i32).clamp(0, last - 1);
        let row0 = (gz.floor() as i32).clamp(0, last - 1);
        let tx = (gx - col0 as f32).clamp(0.0, 1.0);
        let tz = (gz - row0 as f32).clamp(0.0, 1.0);

        let h00 = self.height(col0, row0);
        let h10 = self.height(col0 + 1, row0);
        let h01 = self.height(col0, row0 + 1);
        let h11 = self.height(col0 + 1, row0 + 1);

        let h0 = h00 + (h10 - h00) * tx;
        let h1 = h01 + (h11 - h01) * tx;
        h0 + (h1 - h0) * tz
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "{a} != {b}");
    }

    /// 2x2 unit tile: corners 0,1,2,3 (row-major).
    fn unit_tile() -> Heightfield {
        Heightfield::from_samples(2, 1.0, vec![0.0, 1.0, 2.0, 3.0]).unwrap()
    }

    #[test]
    fn samples_corners_exactly() {
        let hf = unit_tile();
        approx(hf.sample(0.0, 0.0), 0.0);
        approx(hf.sample(1.0, 0.0), 1.0);
        approx(hf.sample(0.0, 1.0), 2.0);
        approx(hf.sample(1.0, 1.0), 3.0);
    }

    #[test]
    fn interpolates_center_bilinearly() {
        let hf = unit_tile();
        approx(hf.sample(0.5, 0.5), 1.5);
        approx(hf.sample(0.5, 0.0), 0.5);
        approx(hf.sample(0.0, 0.5), 1.0);
    }

    #[test]
    fn interpolates_with_nonunit_spacing() {
        // 3x3 tile, spacing 2 -> chunk size 4.
        let samples = vec![0.0, 2.0, 4.0, 0.0, 2.0, 4.0, 0.0, 2.0, 4.0];
        let hf = Heightfield::from_samples(3, 2.0, samples).unwrap();
        approx(hf.chunk_size_meters(), 4.0);
        // Height varies linearly with X at rate 1 per meter, flat in Z.
        approx(hf.sample(1.0, 0.0), 1.0);
        approx(hf.sample(3.0, 2.0), 3.0);
        approx(hf.sample(2.5, 3.0), 2.5);
    }

    #[test]
    fn try_sample_rejects_out_of_domain() {
        let hf = unit_tile();
        assert!(hf.try_sample(-0.1, 0.5).is_err());
        assert!(hf.try_sample(0.5, 99.0).is_err());
        assert_eq!(hf.try_sample(0.5, 0.5).unwrap(), 1.5);
    }

    #[test]
    fn clamps_outside_domain() {
        let hf = unit_tile();
        approx(hf.sample(-5.0, -5.0), 0.0);
        approx(hf.sample(99.0, 99.0), 3.0);
    }

    #[test]
    fn rejects_mismatched_sample_count() {
        let err = Heightfield::from_samples(3, 1.0, vec![0.0; 4]).unwrap_err();
        assert_eq!(
            err,
            TerrainDataError::SampleCountMismatch {
                expected: 9,
                actual: 4,
            }
        );
    }

    #[test]
    fn rejects_degenerate_parameters() {
        assert_eq!(
            Heightfield::from_samples(1, 1.0, vec![0.0]).unwrap_err(),
            TerrainDataError::TooSmall {
                samples_per_edge: 1,
            }
        );
        assert_eq!(
            Heightfield::from_samples(2, 0.0, vec![0.0; 4]).unwrap_err(),
            TerrainDataError::NonPositiveSpacing {
                spacing_meters: 0.0
            }
        );
    }

    #[test]
    fn rejects_non_finite_samples() {
        let mut samples = vec![0.0; 4];
        samples[2] = f32::NAN;
        assert_eq!(
            Heightfield::from_samples(2, 1.0, samples.clone()).unwrap_err(),
            TerrainDataError::NonFiniteHeightSample { index: 2 }
        );
        let mut samples = vec![0.0; 4];
        samples[2] = f32::INFINITY;
        assert_eq!(
            Heightfield::from_samples(2, 1.0, samples).unwrap_err(),
            TerrainDataError::NonFiniteHeightSample { index: 2 }
        );
        let mut samples = vec![0.0; 4];
        samples[2] = f32::NEG_INFINITY;
        assert_eq!(
            Heightfield::from_samples(2, 1.0, samples).unwrap_err(),
            TerrainDataError::NonFiniteHeightSample { index: 2 }
        );
    }

    #[test]
    fn rejects_non_finite_spacing() {
        assert!(matches!(
            Heightfield::from_samples(2, f32::NAN, vec![0.0; 4]),
            Err(TerrainDataError::NonPositiveSpacing { .. })
        ));
    }
}
