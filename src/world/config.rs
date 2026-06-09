use bevy::prelude::*;

use super::coordinates::ChunkLayout;

/// Default chunk edge length in meters (ADR-002).
pub const DEFAULT_CHUNK_SIZE_METERS: f32 = 256.0;
/// Default world units per meter (ADR-001 addendum: 1 unit = 1 meter).
pub const DEFAULT_UNITS_PER_METER: f32 = 1.0;
/// Provisional default terrain sample spacing in meters (ADR-003 addendum).
pub const DEFAULT_METERS_PER_SAMPLE: f32 = 1.0;

/// Authoritative world configuration.
///
/// `WorldConfig` is the single source of truth for the parameters that define
/// the world's spatial layout. Chunk size lives here (ADR-002 addendum) rather
/// than as a hard-coded constant, so it can be reconfigured without scattering
/// literals across systems. Coordinate conversions consume the derived
/// [`ChunkLayout`] rather than reading this resource directly.
#[derive(Debug, Clone, Copy, PartialEq, Resource, Reflect)]
#[reflect(Resource)]
pub struct WorldConfig {
    /// Chunk edge length in meters.
    pub chunk_size_meters: f32,
    /// World units per meter.
    pub units_per_meter: f32,
    /// Terrain sample spacing in meters (provisional; consumed in later phases).
    pub meters_per_sample: f32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            chunk_size_meters: DEFAULT_CHUNK_SIZE_METERS,
            units_per_meter: DEFAULT_UNITS_PER_METER,
            meters_per_sample: DEFAULT_METERS_PER_SAMPLE,
        }
    }
}

impl WorldConfig {
    /// The minimal, copyable layout that coordinate conversions consume.
    pub fn chunk_layout(&self) -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: self.chunk_size_meters,
            units_per_meter: self.units_per_meter,
        }
    }
}
