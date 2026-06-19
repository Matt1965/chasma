//! Deterministic seeded PRNG for procedural doodad placement (ADR-018).
//!
//! SplitMix64 — no external `rand` dependency; identical seed yields identical sequence.

/// Deterministic pseudo-random generator (SplitMix64).
#[derive(Debug, Clone)]
pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Uniform float in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16777216.0
    }
}

/// Derive a chunk-local seed from world seed and chunk coordinates.
pub fn chunk_seed(world_seed: u64, chunk_x: i32, chunk_z: i32) -> u64 {
    let mut h = world_seed;
    h = h
        .wrapping_mul(0x9E3779B97F4A7C15)
        .wrapping_add(chunk_x as u64);
    h = h
        .wrapping_mul(0x9E3779B97F4A7C15)
        .wrapping_add(chunk_z as u64);
    h ^ 0xD00D_AD18_0000_0001
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = DeterministicRng::new(42);
        let seq_a: Vec<_> = (0..4).map(|_| a.next_u64()).collect();
        let mut b = DeterministicRng::new(42);
        let seq_b: Vec<_> = (0..4).map(|_| b.next_u64()).collect();
        assert_eq!(seq_a, seq_b);
    }

    #[test]
    fn different_seeds_differ() {
        assert_ne!(
            DeterministicRng::new(1).next_u64(),
            DeterministicRng::new(2).next_u64()
        );
    }
}
