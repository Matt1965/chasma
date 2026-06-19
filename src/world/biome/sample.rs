use bevy::prelude::*;

use super::id::BiomeId;

/// Result of sampling the world biome mask at a position (ADR-024).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct BiomeSample {
    pub biome: BiomeId,
    pub pixel_x: u32,
    pub pixel_z: u32,
}

impl BiomeSample {
    pub const fn new(biome: BiomeId, pixel_x: u32, pixel_z: u32) -> Self {
        Self {
            biome,
            pixel_x,
            pixel_z,
        }
    }
}
