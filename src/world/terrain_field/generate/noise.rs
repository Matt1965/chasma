//! Small deterministic value noise for offline field generation (ADR-102).

use super::seed::{hash_mix, hash_position};

pub fn value_noise_01(seed: u64, x_meters: f32, z_meters: f32, scale_meters: f32) -> f32 {
    if scale_meters <= 0.0 {
        return 0.0;
    }
    let x = x_meters / scale_meters;
    let z = z_meters / scale_meters;
    let x0 = x.floor();
    let z0 = z.floor();
    let fx = (x - x0) as f32;
    let fz = (z - z0) as f32;

    let v00 = lattice(seed, x0, z0);
    let v10 = lattice(seed, x0 + 1.0, z0);
    let v01 = lattice(seed, x0, z0 + 1.0);
    let v11 = lattice(seed, x0 + 1.0, z0 + 1.0);

    let sx = smoothstep(fx);
    let sz = smoothstep(fz);
    let top = lerp(v00, v10, sx);
    let bottom = lerp(v01, v11, sx);
    lerp(top, bottom, sz)
}

pub fn fbm_01(
    seed: u64,
    x_meters: f32,
    z_meters: f32,
    scale_meters: f32,
    octaves: u8,
    persistence: f32,
    lacunarity: f32,
) -> f32 {
    let mut amplitude = 1.0f32;
    let mut frequency = 1.0f32;
    let mut sum = 0.0f32;
    let mut norm = 0.0f32;
    for i in 0..octaves.max(1) {
        let octave_seed = hash_mix(seed, i as u64 + 1);
        sum +=
            value_noise_01(octave_seed, x_meters, z_meters, scale_meters / frequency) * amplitude;
        norm += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }
    if norm > 0.0 { sum / norm } else { 0.0 }
}

pub fn ridged_01(seed: u64, x_meters: f32, z_meters: f32, scale_meters: f32) -> f32 {
    let n = value_noise_01(seed, x_meters, z_meters, scale_meters);
    1.0 - (n * 2.0 - 1.0).abs()
}

fn lattice(seed: u64, x: f32, z: f32) -> f32 {
    let h = hash_position(seed, x, z);
    (h as f32 / u64::MAX as f32).clamp(0.0, 1.0)
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn remap_to_u16(value_01: f32) -> u16 {
    (value_01.clamp(0.0, 1.0) * 65_535.0).round() as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_noise() {
        let a = value_noise_01(1, 100.0, 200.0, 64.0);
        let b = value_noise_01(1, 100.0, 200.0, 64.0);
        assert_eq!(a, b);
    }

    #[test]
    fn different_seeds_differ() {
        let a = value_noise_01(1, 50.0, 50.0, 32.0);
        let b = value_noise_01(2, 50.0, 50.0, 32.0);
        assert_ne!(a, b);
    }
}
