use bevy::prelude::*;

use super::heightfield::Heightfield;

/// Cheap, derived facts about a chunk's terrain, computed at construction
/// (ADR-008).
///
/// Kept intentionally minimal: only the height range, whose near-term consumers
/// are Phase 2 chunk bounds/culling and import-time validation. No biome,
/// material, or slope caches are stored (ADR-005 treats those as internal).
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct TerrainMetadata {
    pub height_min: f32,
    pub height_max: f32,
}

impl TerrainMetadata {
    /// Derive metadata by scanning a heightfield's samples.
    pub fn from_heightfield(heightfield: &Heightfield) -> Self {
        let mut height_min = f32::INFINITY;
        let mut height_max = f32::NEG_INFINITY;
        for &h in heightfield.samples() {
            if h < height_min {
                height_min = h;
            }
            if h > height_max {
                height_max = h;
            }
        }
        Self {
            height_min,
            height_max,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_height_range() {
        let hf = Heightfield::from_samples(2, 1.0, vec![-3.0, 5.0, 0.0, 2.5]).unwrap();
        let meta = TerrainMetadata::from_heightfield(&hf);
        assert_eq!(meta.height_min, -3.0);
        assert_eq!(meta.height_max, 5.0);
    }
}
