use crate::world::terrain::Heightfield;

/// Estimate terrain slope in degrees at a chunk-local position (ADR-021).
///
/// Uses forward finite differences over one heightfield sample spacing.
/// Returns `None` when the neighborhood is not fully inside the heightfield domain.
pub fn estimate_slope_degrees(
    heightfield: &Heightfield,
    local_x: f32,
    local_z: f32,
) -> Option<f32> {
    let spacing = heightfield.spacing_meters();
    let size = heightfield.chunk_size_meters();

    if local_x < 0.0
        || local_z < 0.0
        || local_x + spacing > size + 1e-4
        || local_z + spacing > size + 1e-4
    {
        return None;
    }

    let h = heightfield.sample(local_x, local_z);
    let h_dx = heightfield.sample(local_x + spacing, local_z);
    let h_dz = heightfield.sample(local_x, local_z + spacing);
    let dhdx = (h_dx - h) / spacing;
    let dhdz = (h_dz - h) / spacing;
    Some(dhdx.hypot(dhdz).atan().to_degrees())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_terrain_has_zero_slope() {
        let samples = vec![10.0; 9];
        let hf = Heightfield::from_samples(3, 128.0, samples).unwrap();
        let slope = estimate_slope_degrees(&hf, 128.0, 128.0).unwrap();
        assert!(slope.abs() < 1e-4);
    }

    #[test]
    fn ramp_has_nonzero_slope() {
        let mut samples = Vec::new();
        for _row in 0..3 {
            for col in 0..3 {
                samples.push(col as f32 * 40.0);
            }
        }
        let hf = Heightfield::from_samples(3, 128.0, samples).unwrap();
        let slope = estimate_slope_degrees(&hf, 128.0, 128.0).unwrap();
        assert!(slope > 15.0);
    }
}
