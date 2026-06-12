use bevy::prelude::*;
use std::collections::HashMap;

use super::chunk::{ChunkData, ChunkId};
use super::config::WorldConfig;
use super::coordinates::{ChunkCoord, ChunkLayout, WorldPosition};

/// Inclusive bounds of the chunks that currently exist in the world.
///
/// The world is finite (ADR-006); the extent is discovered as chunks are
/// inserted rather than assumed up front.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct ChunkExtent {
    pub min: ChunkCoord,
    pub max: ChunkCoord,
}

/// The authoritative World Data Layer store (ADR-002, ADR-008).
///
/// `WorldData` maps each [`ChunkId`] to its [`ChunkData`] and tracks the finite
/// world extent. It also holds the realized world's [`ChunkLayout`] (a snapshot
/// derived from [`WorldConfig`] at initialization) so position-based lookups do
/// not require the layout to be threaded through every call.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct WorldData {
    layout: ChunkLayout,
    chunks: HashMap<ChunkId, ChunkData>,
    extent: Option<ChunkExtent>,
}

impl FromWorld for WorldData {
    fn from_world(world: &mut World) -> Self {
        let layout = world.resource::<WorldConfig>().chunk_layout();
        Self::new(layout)
    }
}

impl WorldData {
    /// Create an empty world with the given spatial layout.
    pub fn new(layout: ChunkLayout) -> Self {
        Self {
            layout,
            chunks: HashMap::new(),
            extent: None,
        }
    }

    /// The spatial layout this world was realized with.
    pub fn layout(&self) -> ChunkLayout {
        self.layout
    }

    /// Insert (or replace) a chunk's data, expanding the world extent.
    pub fn insert(&mut self, chunk: ChunkId, data: ChunkData) {
        self.expand_extent(chunk.coord());
        self.chunks.insert(chunk, data);
    }

    /// The chunk that owns the given global position, regardless of whether it
    /// is loaded (pure coordinate math; ADR-001, ADR-005).
    pub fn chunk_at(&self, global: Vec3) -> ChunkId {
        ChunkId::new(WorldPosition::from_global(global, self.layout).chunk)
    }

    /// Whether the given chunk currently has data resident.
    pub fn is_chunk_loaded(&self, chunk: ChunkId) -> bool {
        self.chunks.contains_key(&chunk)
    }

    /// Borrow a chunk's data, if loaded.
    pub fn get(&self, chunk: ChunkId) -> Option<&ChunkData> {
        self.chunks.get(&chunk)
    }

    /// Iterate over the loaded chunks and their data.
    ///
    /// Iteration order is unspecified; callers that need determinism (e.g. the
    /// offline asset writer) must sort by [`ChunkId`].
    pub fn iter(&self) -> impl Iterator<Item = (ChunkId, &ChunkData)> {
        self.chunks.iter().map(|(id, data)| (*id, data))
    }

    /// Sample terrain height at a global position, if its chunk is loaded
    /// (ADR-005). Returns `None` when the owning chunk is not resident.
    pub fn height_at(&self, global: Vec3) -> Option<f32> {
        let position = WorldPosition::from_global(global, self.layout);
        let data = self.chunks.get(&ChunkId::new(position.chunk))?;
        Some(data.heightfield.sample(position.local.0.x, position.local.0.z))
    }

    /// The inclusive bounds of existing chunks, or `None` if the world is empty.
    pub fn extent(&self) -> Option<ChunkExtent> {
        self.extent
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    fn expand_extent(&mut self, coord: ChunkCoord) {
        self.extent = Some(match self.extent {
            None => ChunkExtent {
                min: coord,
                max: coord,
            },
            Some(current) => ChunkExtent {
                min: ChunkCoord::new(current.min.x.min(coord.x), current.min.z.min(coord.z)),
                max: ChunkCoord::new(current.max.x.max(coord.x), current.max.z.max(coord.z)),
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    /// 3x3 tile spanning a 256 m chunk (spacing 128 m), with heights encoding
    /// `row * 10 + col`.
    fn sample_chunk() -> ChunkData {
        let mut samples = Vec::new();
        for row in 0..3 {
            for col in 0..3 {
                samples.push((row * 10 + col) as f32);
            }
        }
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        ChunkData::new(heightfield, Vec::new())
    }

    #[test]
    fn chunk_at_maps_global_to_chunk() {
        let world = WorldData::new(layout());
        assert_eq!(
            world.chunk_at(Vec3::new(300.0, 0.0, 10.0)),
            ChunkId::new(ChunkCoord::new(1, 0))
        );
        assert_eq!(
            world.chunk_at(Vec3::new(-1.0, 0.0, -1.0)),
            ChunkId::new(ChunkCoord::new(-1, -1))
        );
    }

    #[test]
    fn tracks_loaded_chunks_and_extent() {
        let mut world = WorldData::new(layout());
        assert!(world.is_empty());
        assert_eq!(world.extent(), None);

        world.insert(ChunkId::new(ChunkCoord::new(0, 0)), sample_chunk());
        world.insert(ChunkId::new(ChunkCoord::new(2, 3)), sample_chunk());

        assert_eq!(world.len(), 2);
        assert!(world.is_chunk_loaded(ChunkId::new(ChunkCoord::new(0, 0))));
        assert!(!world.is_chunk_loaded(ChunkId::new(ChunkCoord::new(1, 0))));
        assert_eq!(
            world.extent(),
            Some(ChunkExtent {
                min: ChunkCoord::new(0, 0),
                max: ChunkCoord::new(2, 3),
            })
        );
    }

    #[test]
    fn height_at_samples_loaded_chunk() {
        let mut world = WorldData::new(layout());
        world.insert(ChunkId::new(ChunkCoord::new(0, 0)), sample_chunk());

        // At a sample node: local (128, 128) -> grid (1, 1) -> row*10+col = 11.
        assert_eq!(world.height_at(Vec3::new(128.0, 0.0, 128.0)), Some(11.0));
        // Origin sample.
        assert_eq!(world.height_at(Vec3::new(0.0, 0.0, 0.0)), Some(0.0));
    }

    #[test]
    fn height_at_returns_none_for_unloaded_chunk() {
        let mut world = WorldData::new(layout());
        world.insert(ChunkId::new(ChunkCoord::new(0, 0)), sample_chunk());
        // Falls in chunk (1, 0), which is not loaded.
        assert_eq!(world.height_at(Vec3::new(300.0, 0.0, 0.0)), None);
    }
}
