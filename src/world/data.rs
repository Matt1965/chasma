use bevy::prelude::*;
use std::collections::HashMap;

use super::chunk::{ChunkData, ChunkId};
use super::config::WorldConfig;
use super::coordinates::{ChunkCoord, ChunkLayout, WorldPosition};

/// Inclusive bounds of the authored world (ADR-006, ADR-012).
///
/// Set once from the manifest catalog at startup. `WorldData::extent()` reports
/// this authored extent, not the bounds of currently resident chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct ChunkExtent {
    pub min: ChunkCoord,
    pub max: ChunkCoord,
}

/// The authoritative World Data Layer store (ADR-002, ADR-008).
///
/// `WorldData` maps each resident [`ChunkId`] to its [`ChunkData`] and tracks
/// the finite authored world extent separately from the resident set (ADR-012).
/// It holds the realized world's [`ChunkLayout`] (a snapshot derived from
/// [`WorldConfig`] at initialization) so position-based lookups do not require
/// the layout to be threaded through every call.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct WorldData {
    layout: ChunkLayout,
    chunks: HashMap<ChunkId, ChunkData>,
    authored_extent: Option<ChunkExtent>,
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
            authored_extent: None,
        }
    }

    /// The spatial layout this world was realized with.
    pub fn layout(&self) -> ChunkLayout {
        self.layout
    }

    /// Set the authored world extent (immutable for the session after catalog init).
    pub fn set_authored_extent(&mut self, extent: ChunkExtent) {
        self.authored_extent = Some(extent);
    }

    /// Insert (or replace) a resident chunk's data.
    ///
    /// Does not change [`Self::authored_extent`]; that is set from the manifest
    /// catalog at startup (ADR-012).
    pub fn insert(&mut self, chunk: ChunkId, data: ChunkData) {
        self.chunks.insert(chunk, data);
    }

    /// Evict a resident chunk. No-op if the chunk is not resident.
    ///
    /// Does not change authored extent or delete on-disk assets (ADR-012).
    pub fn remove(&mut self, chunk: ChunkId) {
        self.chunks.remove(&chunk);
    }

    /// The chunk that owns the given global position, regardless of whether it
    /// is resident (pure coordinate math; ADR-001, ADR-005).
    pub fn chunk_at(&self, global: Vec3) -> ChunkId {
        ChunkId::new(WorldPosition::from_global(global, self.layout).chunk)
    }

    /// Whether the given chunk currently has data resident.
    pub fn is_chunk_loaded(&self, chunk: ChunkId) -> bool {
        self.chunks.contains_key(&chunk)
    }

    /// Borrow a chunk's data, if resident.
    pub fn get(&self, chunk: ChunkId) -> Option<&ChunkData> {
        self.chunks.get(&chunk)
    }

    /// Iterate over resident chunks and their data.
    ///
    /// Iteration order is unspecified; callers that need determinism (e.g. the
    /// offline asset writer) must sort by [`ChunkId`].
    pub fn iter(&self) -> impl Iterator<Item = (ChunkId, &ChunkData)> {
        self.chunks.iter().map(|(id, data)| (*id, data))
    }

    /// Sample terrain height at a global position, if its chunk is resident
    /// (ADR-005). Returns `None` when the owning chunk is not resident.
    pub fn height_at(&self, global: Vec3) -> Option<f32> {
        let position = WorldPosition::from_global(global, self.layout);
        let data = self.chunks.get(&ChunkId::new(position.chunk))?;
        Some(data.heightfield.sample(position.local.0.x, position.local.0.z))
    }

    /// The inclusive authored bounds of the world, or `None` if not set yet.
    pub fn extent(&self) -> Option<ChunkExtent> {
        self.authored_extent
    }

    /// Inclusive bounds of currently resident chunks, if any.
    pub fn resident_extent(&self) -> Option<ChunkExtent> {
        let mut iter = self.chunks.keys().map(|id| id.coord());
        let first = iter.next()?;
        let mut min = first;
        let mut max = first;
        for coord in iter {
            min = ChunkCoord::new(min.x.min(coord.x), min.z.min(coord.z));
            max = ChunkCoord::new(max.x.max(coord.x), max.z.max(coord.z));
        }
        Some(ChunkExtent { min, max })
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkData, Heightfield};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn authored() -> ChunkExtent {
        ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(2, 3),
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
    fn authored_extent_is_independent_of_residents() {
        let mut world = WorldData::new(layout());
        world.set_authored_extent(authored());
        assert_eq!(world.extent(), Some(authored()));

        world.insert(ChunkId::new(ChunkCoord::new(0, 0)), sample_chunk());
        assert_eq!(world.extent(), Some(authored()));
        assert_eq!(
            world.resident_extent(),
            Some(ChunkExtent {
                min: ChunkCoord::new(0, 0),
                max: ChunkCoord::new(0, 0),
            })
        );
    }

    #[test]
    fn insert_does_not_expand_authored_extent() {
        let mut world = WorldData::new(layout());
        world.set_authored_extent(authored());
        world.insert(ChunkId::new(ChunkCoord::new(5, 5)), sample_chunk());
        assert_eq!(world.extent(), Some(authored()));
    }

    #[test]
    fn tracks_resident_chunks() {
        let mut world = WorldData::new(layout());
        world.set_authored_extent(authored());

        world.insert(ChunkId::new(ChunkCoord::new(0, 0)), sample_chunk());
        world.insert(ChunkId::new(ChunkCoord::new(2, 3)), sample_chunk());

        assert_eq!(world.len(), 2);
        assert!(world.is_chunk_loaded(ChunkId::new(ChunkCoord::new(0, 0))));
        assert!(!world.is_chunk_loaded(ChunkId::new(ChunkCoord::new(1, 0))));
    }

    #[test]
    fn remove_evicts_resident_without_changing_authored_extent() {
        let mut world = WorldData::new(layout());
        world.set_authored_extent(authored());
        let id = ChunkId::new(ChunkCoord::new(0, 0));
        world.insert(id, sample_chunk());

        world.remove(id);
        assert!(!world.is_chunk_loaded(id));
        assert_eq!(world.get(id), None);
        assert_eq!(world.extent(), Some(authored()));

        world.remove(id);
        assert!(!world.is_chunk_loaded(id));
    }

    #[test]
    fn height_at_samples_loaded_chunk() {
        let mut world = WorldData::new(layout());
        world.set_authored_extent(authored());
        world.insert(ChunkId::new(ChunkCoord::new(0, 0)), sample_chunk());

        assert_eq!(world.height_at(Vec3::new(128.0, 0.0, 128.0)), Some(11.0));
        assert_eq!(world.height_at(Vec3::new(0.0, 0.0, 0.0)), Some(0.0));
    }

    #[test]
    fn height_at_returns_none_for_unloaded_chunk() {
        let mut world = WorldData::new(layout());
        world.set_authored_extent(authored());
        world.insert(ChunkId::new(ChunkCoord::new(0, 0)), sample_chunk());
        assert_eq!(world.height_at(Vec3::new(300.0, 0.0, 0.0)), None);
    }
}
