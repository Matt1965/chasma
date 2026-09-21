use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::terrain::{Heightfield, TerrainQueryError};

/// Per-chunk road height delta samples aligned to the base heightfield grid.
///
/// Deltas are added to base terrain height at query and mesh-build time.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct RoadHeightDeltaTile {
    pub samples_per_edge: u32,
    pub spacing_meters: f32,
    pub deltas: Vec<f32>,
}

impl RoadHeightDeltaTile {
    pub fn zero_for_heightfield(heightfield: &Heightfield) -> Self {
        let count = (heightfield.samples_per_edge() as usize).pow(2);
        Self {
            samples_per_edge: heightfield.samples_per_edge(),
            spacing_meters: heightfield.spacing_meters(),
            deltas: vec![0.0; count],
        }
    }

    pub fn is_effectively_zero(&self) -> bool {
        self.deltas.iter().all(|delta| delta.abs() <= 1e-6)
    }

    pub fn sample_delta(&self, local_x: f32, local_z: f32) -> Result<f32, TerrainQueryError> {
        let size = (self.samples_per_edge - 1) as f32 * self.spacing_meters;
        if local_x < -1e-4 || local_z < -1e-4 || local_x > size + 1e-4 || local_z > size + 1e-4 {
            return Err(TerrainQueryError::InvalidTerrainCoordinate);
        }
        Ok(bilinear_sample(
            &self.deltas,
            self.samples_per_edge,
            self.spacing_meters,
            local_x,
            local_z,
        ))
    }

    pub fn delta_at_vertex(&self, col: u32, row: u32) -> f32 {
        let stride = self.samples_per_edge as usize;
        self.deltas[row as usize * stride + col as usize]
    }

    pub fn effective_samples(&self, base: &Heightfield) -> Vec<f32> {
        base.samples()
            .iter()
            .zip(self.deltas.iter())
            .map(|(base_height, delta)| base_height + delta)
            .collect()
    }
}

fn bilinear_sample(
    samples: &[f32],
    samples_per_edge: u32,
    spacing_meters: f32,
    local_x: f32,
    local_z: f32,
) -> f32 {
    let size = (samples_per_edge - 1) as f32 * spacing_meters;
    let x = local_x.clamp(0.0, size);
    let z = local_z.clamp(0.0, size);
    let fx = x / spacing_meters;
    let fz = z / spacing_meters;
    let col = fx.floor() as i32;
    let row = fz.floor() as i32;
    let max = samples_per_edge as i32 - 1;
    let col1 = (col + 1).min(max);
    let row1 = (row + 1).min(max);
    let tx = fx - col as f32;
    let tz = fz - row as f32;
    let stride = samples_per_edge as usize;
    let h00 = samples[row as usize * stride + col as usize];
    let h10 = samples[row as usize * stride + col1 as usize];
    let h01 = samples[row1 as usize * stride + col as usize];
    let h11 = samples[row1 as usize * stride + col1 as usize];
    let hx0 = h00.lerp(h10, tx);
    let hx1 = h01.lerp(h11, tx);
    hx0.lerp(hx1, tz)
}
