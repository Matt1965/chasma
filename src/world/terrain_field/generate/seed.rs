//! Deterministic seed composition for field generation (ADR-102).

use super::super::id::{TerrainFieldId, TerrainFieldSourceProfileId};

pub fn compose_field_seed(
    world_seed: u64,
    field_id: &TerrainFieldId,
    profile_id: &TerrainFieldSourceProfileId,
    generator_version: u32,
) -> u64 {
    let mut h = world_seed ^ 0xD1B5_4A32_D192_ED03;
    h = hash_mix(h, hash_str(field_id.as_str()));
    h = hash_mix(h, hash_str(profile_id.as_str()));
    h = hash_mix(h, generator_version as u64);
    h
}

pub fn hash_mix(seed: u64, value: u64) -> u64 {
    let mut x = seed ^ value.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 33;
    x = x.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 29;
    x
}

pub fn hash_str(s: &str) -> u64 {
    let mut h = 0u64;
    for byte in s.bytes() {
        h = hash_mix(h, byte as u64);
    }
    h
}

pub fn hash_position(seed: u64, x_meters: f32, z_meters: f32) -> u64 {
    let x_bits = x_meters.to_bits() as u64;
    let z_bits = z_meters.to_bits() as u64;
    hash_mix(hash_mix(seed, x_bits), z_bits)
}
