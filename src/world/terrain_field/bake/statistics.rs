//! Field build statistics (ADR-102).

use super::super::layer::TerrainFieldLayer;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerrainFieldStatistics {
    pub minimum: u16,
    pub maximum: u16,
    pub average: u64,
    pub sample_count: u64,
    pub zero_count: u64,
    pub histogram: [u32; 16],
    pub tile_count: usize,
    pub shared_edges_valid: bool,
}

impl TerrainFieldStatistics {
    pub fn from_layer(layer: &TerrainFieldLayer) -> Self {
        let mut stats = Self {
            minimum: u16::MAX,
            maximum: 0,
            ..Default::default()
        };
        for tile in layer.tiles.values() {
            for &value in &tile.samples {
                stats.sample_count += 1;
                stats.minimum = stats.minimum.min(value);
                stats.maximum = stats.maximum.max(value);
                stats.average += value as u64;
                if value == 0 {
                    stats.zero_count += 1;
                }
                let bucket = ((value as u32 * 16) / 65_536).min(15) as usize;
                stats.histogram[bucket] += 1;
            }
        }
        if stats.sample_count > 0 {
            stats.average /= stats.sample_count;
        }
        stats.tile_count = layer.tiles.len();
        stats.shared_edges_valid = layer.validate_shared_edges().is_ok();
        stats
    }

    pub fn zero_percent_basis_points(&self) -> u16 {
        if self.sample_count == 0 {
            return 0;
        }
        ((self.zero_count * 10_000) / self.sample_count) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ChunkCoord;
    use crate::world::terrain_field::fixtures::bootstrap_constant_field;
    use crate::world::terrain_field::{TerrainFieldId, TerrainFieldStore};

    #[test]
    fn constant_field_stats() {
        let mut store = TerrainFieldStore::new();
        bootstrap_constant_field(
            &mut store,
            TerrainFieldId::new("water"),
            ChunkCoord::new(0, 0),
            20_000,
        );
        let layer = store.get_layer(&TerrainFieldId::new("water")).unwrap();
        let stats = TerrainFieldStatistics::from_layer(layer);
        assert_eq!(stats.minimum, 20_000);
        assert_eq!(stats.maximum, 20_000);
        assert_eq!(stats.zero_count, 0);
    }
}
