use bevy::prelude::*;

use super::id::BiomeId;

/// Maps authored PNG RGB colors to [`BiomeId`] (ADR-024).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub struct BiomeColorMapping {
    entries: Vec<BiomeColorEntry>,
}

/// One color → biome rule. Per-channel tolerance accommodates PNG compression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct BiomeColorEntry {
    pub rgb: [u8; 3],
    pub biome: BiomeId,
    pub tolerance: u8,
}

impl BiomeColorMapping {
    pub fn new(entries: Vec<BiomeColorEntry>) -> Self {
        Self { entries }
    }

    /// Starter mapping for Phase R1 (ADR-024).
    pub fn starter() -> Self {
        Self::new(vec![
            BiomeColorEntry {
                rgb: [255, 0, 0],
                biome: BiomeId::Desert,
                tolerance: 0,
            },
            BiomeColorEntry {
                rgb: [0, 255, 0],
                biome: BiomeId::Forest,
                tolerance: 0,
            },
            BiomeColorEntry {
                rgb: [0, 0, 255],
                biome: BiomeId::Marsh,
                tolerance: 0,
            },
            BiomeColorEntry {
                rgb: [255, 255, 0],
                biome: BiomeId::Plains,
                tolerance: 0,
            },
            BiomeColorEntry {
                rgb: [0, 0, 0],
                biome: BiomeId::Unassigned,
                tolerance: 0,
            },
        ])
    }

    pub fn entries(&self) -> &[BiomeColorEntry] {
        &self.entries
    }

    /// Classify an RGB pixel. Unmapped colors become [`BiomeId::Unassigned`].
    pub fn classify_rgb(&self, rgb: [u8; 3]) -> BiomeId {
        for entry in &self.entries {
            if color_matches(rgb, entry.rgb, entry.tolerance) {
                return entry.biome;
            }
        }
        BiomeId::Unassigned
    }
}

fn color_matches(sample: [u8; 3], target: [u8; 3], tolerance: u8) -> bool {
    sample
        .iter()
        .zip(target.iter())
        .all(|(s, t)| s.abs_diff(*t) <= tolerance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_maps_primary_colors() {
        let mapping = BiomeColorMapping::starter();
        assert_eq!(mapping.classify_rgb([255, 0, 0]), BiomeId::Desert);
        assert_eq!(mapping.classify_rgb([0, 255, 0]), BiomeId::Forest);
        assert_eq!(mapping.classify_rgb([0, 0, 255]), BiomeId::Marsh);
        assert_eq!(mapping.classify_rgb([255, 255, 0]), BiomeId::Plains);
        assert_eq!(mapping.classify_rgb([0, 0, 0]), BiomeId::Unassigned);
    }

    #[test]
    fn unmapped_color_is_unassigned() {
        let mapping = BiomeColorMapping::starter();
        assert_eq!(mapping.classify_rgb([128, 64, 32]), BiomeId::Unassigned);
    }

    #[test]
    fn tolerance_allows_near_matches() {
        let mapping = BiomeColorMapping::new(vec![BiomeColorEntry {
            rgb: [255, 0, 0],
            biome: BiomeId::Desert,
            tolerance: 2,
        }]);
        assert_eq!(mapping.classify_rgb([253, 1, 0]), BiomeId::Desert);
        assert_eq!(mapping.classify_rgb([250, 0, 0]), BiomeId::Unassigned);
    }
}
