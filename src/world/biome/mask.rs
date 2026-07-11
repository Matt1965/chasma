use bevy::prelude::*;

use super::error::BiomeImportError;
use super::id::BiomeId;
use super::mapping::BiomeColorMapping;
use super::sample::BiomeSample;
use crate::world::{ChunkExtent, ChunkLayout};

/// World-space XZ bounds covered by a biome mask image (ADR-024).
///
/// Mapping convention:
/// - Origin (`origin_x`, `origin_z`) is the **southwest** corner (minimum X and Z).
/// - `extent_x` / `extent_z` span the full authored world width and depth in units.
/// - Image column `x` maps to world X; row `z` maps to world Z.
/// - Row `0` is the **south** edge (minimum Z); row `height - 1` is the **north** edge.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct BiomeMaskBounds {
    pub origin_x: f32,
    pub origin_z: f32,
    pub extent_x: f32,
    pub extent_z: f32,
}

impl BiomeMaskBounds {
    pub const fn new(origin_x: f32, origin_z: f32, extent_x: f32, extent_z: f32) -> Self {
        Self {
            origin_x,
            origin_z,
            extent_x,
            extent_z,
        }
    }

    /// Build bounds from inclusive chunk extent and layout (ADR-006, ADR-012).
    pub fn from_chunk_extent(extent: ChunkExtent, layout: ChunkLayout) -> Self {
        let chunk_size = layout.chunk_size_units();
        let chunk_count_x = (extent.max.x - extent.min.x + 1) as f32;
        let chunk_count_z = (extent.max.z - extent.min.z + 1) as f32;
        Self {
            origin_x: extent.min.x as f32 * chunk_size,
            origin_z: extent.min.z as f32 * chunk_size,
            extent_x: chunk_count_x * chunk_size,
            extent_z: chunk_count_z * chunk_size,
        }
    }

    pub fn max_x(&self) -> f32 {
        self.origin_x + self.extent_x
    }

    pub fn max_z(&self) -> f32 {
        self.origin_z + self.extent_z
    }

    /// Whether global XZ lies inside the mask bounds (half-open on the max edge).
    pub fn contains_global_xz(&self, global_x: f32, global_z: f32) -> bool {
        global_x >= self.origin_x
            && global_z >= self.origin_z
            && global_x < self.max_x()
            && global_z < self.max_z()
    }

    /// Map global XZ to pixel coordinates. Returns `None` when out of bounds.
    pub fn global_xz_to_pixel(
        &self,
        width: u32,
        height: u32,
        global_x: f32,
        global_z: f32,
    ) -> Option<(u32, u32)> {
        if !self.contains_global_xz(global_x, global_z) {
            return None;
        }
        if width == 0 || height == 0 {
            return None;
        }

        let u = (global_x - self.origin_x) / self.extent_x;
        let v = (global_z - self.origin_z) / self.extent_z;

        let pixel_x = (u * width as f32).floor() as u32;
        let pixel_z = (v * height as f32).floor() as u32;

        Some((pixel_x.min(width - 1), pixel_z.min(height - 1)))
    }

    /// Inverse mapping: pixel center to global XZ (for tests and tooling).
    pub fn pixel_center_to_global_xz(
        &self,
        width: u32,
        height: u32,
        pixel_x: u32,
        pixel_z: u32,
    ) -> (f32, f32) {
        let u = (pixel_x as f32 + 0.5) / width as f32;
        let v = (pixel_z as f32 + 0.5) / height as f32;
        (
            self.origin_x + u * self.extent_x,
            self.origin_z + v * self.extent_z,
        )
    }
}

/// World-scale authoritative biome classification grid (ADR-024).
///
/// Stores pre-classified [`BiomeId`] values in a compact row-major buffer.
/// Future channels (density, resource masks, etc.) may extend this type.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct BiomeMask {
    width: u32,
    height: u32,
    bounds: BiomeMaskBounds,
    /// Row-major `pixel_z * width + pixel_x`.
    pixels: Vec<BiomeId>,
}

impl BiomeMask {
    pub fn new(
        width: u32,
        height: u32,
        bounds: BiomeMaskBounds,
        pixels: Vec<BiomeId>,
    ) -> Result<Self, BiomeImportError> {
        if width == 0 || height == 0 {
            return Err(BiomeImportError::EmptyImage);
        }
        let expected = (width as usize) * (height as usize);
        if pixels.len() != expected {
            return Err(BiomeImportError::DimensionMismatch {
                expected_len: expected,
                actual_len: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            bounds,
            pixels,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn bounds(&self) -> BiomeMaskBounds {
        self.bounds
    }

    pub fn pixel_biome(&self, pixel_x: u32, pixel_z: u32) -> BiomeId {
        let index = pixel_z as usize * self.width as usize + pixel_x as usize;
        self.pixels
            .get(index)
            .copied()
            .unwrap_or(BiomeId::Unassigned)
    }

    /// Deterministic biome lookup from global render-space XZ (Y ignored).
    pub fn sample_at_global(&self, global: Vec3) -> BiomeSample {
        let Some((pixel_x, pixel_z)) =
            self.bounds
                .global_xz_to_pixel(self.width, self.height, global.x, global.z)
        else {
            return BiomeSample::new(BiomeId::Unassigned, 0, 0);
        };
        let biome = self.pixel_biome(pixel_x, pixel_z);
        BiomeSample::new(biome, pixel_x, pixel_z)
    }

    /// Count classified pixels per [`BiomeId`].
    pub fn biome_pixel_counts(&self) -> std::collections::BTreeMap<BiomeId, u32> {
        let mut counts = std::collections::BTreeMap::new();
        for biome in &self.pixels {
            *counts.entry(*biome).or_insert(0) += 1;
        }
        counts
    }

    /// Build from decoded RGBA/RGB rows (top row = south / min Z per ADR-024).
    pub fn from_rgba_rows(
        width: u32,
        height: u32,
        bounds: BiomeMaskBounds,
        rgba: &[u8],
        bytes_per_pixel: usize,
        mapping: &BiomeColorMapping,
    ) -> Result<Self, BiomeImportError> {
        if width == 0 || height == 0 {
            return Err(BiomeImportError::EmptyImage);
        }
        let expected = (width as usize) * (height as usize) * bytes_per_pixel;
        if rgba.len() != expected {
            return Err(BiomeImportError::DimensionMismatch {
                expected_len: expected,
                actual_len: rgba.len(),
            });
        }

        let mut pixels = Vec::with_capacity((width as usize) * (height as usize));
        for pixel in rgba.chunks(bytes_per_pixel) {
            let rgb = [pixel[0], pixel[1], pixel[2]];
            pixels.push(mapping.classify_rgb(rgb));
        }

        Self::new(width, height, bounds, pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, ChunkExtent, ChunkLayout, biome::mapping::BiomeColorMapping};

    fn test_bounds_4_chunks() -> BiomeMaskBounds {
        BiomeMaskBounds::from_chunk_extent(
            ChunkExtent {
                min: ChunkCoord::new(0, 0),
                max: ChunkCoord::new(1, 1),
            },
            ChunkLayout {
                chunk_size_meters: 256.0,
                units_per_meter: 1.0,
            },
        )
    }

    #[test]
    fn bounds_from_chunk_extent() {
        let bounds = test_bounds_4_chunks();
        assert_eq!(bounds.origin_x, 0.0);
        assert_eq!(bounds.origin_z, 0.0);
        assert_eq!(bounds.extent_x, 512.0);
        assert_eq!(bounds.extent_z, 512.0);
    }

    #[test]
    fn southwest_pixel_maps_to_origin() {
        let bounds = BiomeMaskBounds::new(0.0, 0.0, 512.0, 512.0);
        assert_eq!(
            bounds.global_xz_to_pixel(1024, 1024, 0.0, 0.0),
            Some((0, 0))
        );
    }

    #[test]
    fn northeast_interior_maps_to_last_pixel() {
        let bounds = BiomeMaskBounds::new(0.0, 0.0, 512.0, 512.0);
        assert_eq!(
            bounds.global_xz_to_pixel(1024, 1024, 511.9, 511.9),
            Some((1023, 1023))
        );
    }

    #[test]
    fn out_of_bounds_returns_none_for_pixel_mapping() {
        let bounds = BiomeMaskBounds::new(0.0, 0.0, 512.0, 512.0);
        assert!(bounds.global_xz_to_pixel(1024, 1024, -1.0, 0.0).is_none());
        assert!(bounds.global_xz_to_pixel(1024, 1024, 512.0, 0.0).is_none());
    }

    #[test]
    fn center_coordinate_is_stable() {
        let bounds = BiomeMaskBounds::new(0.0, 0.0, 512.0, 512.0);
        let (px, pz) = bounds.global_xz_to_pixel(1024, 1024, 256.0, 256.0).unwrap();
        assert_eq!((px, pz), (512, 512));
    }

    #[test]
    fn sample_is_deterministic() {
        let bounds = BiomeMaskBounds::new(0.0, 0.0, 4.0, 4.0);
        let mask = BiomeMask::from_rgba_rows(
            2,
            2,
            bounds,
            &[
                255, 0, 0, 255, 0, 255, 0, 255, //
                0, 0, 255, 255, 255, 255, 0, 255, //
            ],
            4,
            &BiomeColorMapping::starter(),
        )
        .unwrap();

        let first = mask.sample_at_global(Vec3::new(0.5, 0.0, 0.5));
        let second = mask.sample_at_global(Vec3::new(0.5, 0.0, 0.5));
        assert_eq!(first, second);
        assert_eq!(first.biome, BiomeId::Desert);
    }

    #[test]
    fn out_of_bounds_sample_is_unassigned() {
        let bounds = BiomeMaskBounds::new(0.0, 0.0, 4.0, 4.0);
        let mask =
            BiomeMask::from_rgba_rows(2, 2, bounds, &[0; 16], 4, &BiomeColorMapping::starter())
                .unwrap();
        let sample = mask.sample_at_global(Vec3::new(10.0, 0.0, 10.0));
        assert_eq!(sample.biome, BiomeId::Unassigned);
    }

    #[test]
    fn pixel_center_round_trip_is_consistent() {
        let bounds = BiomeMaskBounds::new(0.0, 0.0, 512.0, 512.0);
        let (gx, gz) = bounds.pixel_center_to_global_xz(1024, 1024, 512, 512);
        let (px, pz) = bounds.global_xz_to_pixel(1024, 1024, gx, gz).unwrap();
        assert_eq!((px, pz), (512, 512));
    }
}
