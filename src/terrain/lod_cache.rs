//! Per-chunk lazy LOD mesh handle cache (ADR-013 Phase 2C).
//!
//! Terrain-runtime render state only; authoritative height data stays in
//! [`crate::world::WorldData`].

use bevy::prelude::*;

use super::mesh::ChunkLod;

/// Lazy cache of generated mesh handles keyed by [`ChunkLod`].
///
/// Discarded when the render entity despawns on chunk unload.
#[derive(Component, Debug, Clone, Default, Reflect)]
#[reflect(Component)]
pub struct TerrainChunkLodCache {
    pub full: Option<Handle<Mesh>>,
    pub half: Option<Handle<Mesh>>,
    pub quarter: Option<Handle<Mesh>>,
    pub eighth: Option<Handle<Mesh>>,
}

impl TerrainChunkLodCache {
    pub fn has_lod(&self, lod: ChunkLod) -> bool {
        self.get(lod).is_some()
    }

    pub fn get(&self, lod: ChunkLod) -> Option<&Handle<Mesh>> {
        match lod {
            ChunkLod::Full => self.full.as_ref(),
            ChunkLod::Half => self.half.as_ref(),
            ChunkLod::Quarter => self.quarter.as_ref(),
            ChunkLod::Eighth => self.eighth.as_ref(),
        }
    }

    pub fn set(&mut self, lod: ChunkLod, handle: Handle<Mesh>) {
        match lod {
            ChunkLod::Full => self.full = Some(handle),
            ChunkLod::Half => self.half = Some(handle),
            ChunkLod::Quarter => self.quarter = Some(handle),
            ChunkLod::Eighth => self.eighth = Some(handle),
        }
    }

    /// Returns how many LOD slots currently hold a mesh handle.
    pub fn cached_lod_count(&self) -> usize {
        [self.full.is_some(), self.half.is_some(), self.quarter.is_some(), self.eighth.is_some()]
            .into_iter()
            .filter(|present| *present)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::PrimitiveTopology;

    fn dummy_mesh_handle(meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        meshes.add(Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        ))
    }

    #[test]
    fn insert_get_has_per_lod() {
        let mut cache = TerrainChunkLodCache::default();
        let mut meshes = Assets::<Mesh>::default();
        let handle = dummy_mesh_handle(&mut meshes);

        for lod in [
            ChunkLod::Full,
            ChunkLod::Half,
            ChunkLod::Quarter,
            ChunkLod::Eighth,
        ] {
            assert!(!cache.has_lod(lod));
            assert!(cache.get(lod).is_none());
        }

        cache.set(ChunkLod::Half, handle.clone());
        assert!(cache.has_lod(ChunkLod::Half));
        assert_eq!(cache.get(ChunkLod::Half), Some(&handle));
        assert!(!cache.has_lod(ChunkLod::Full));
        assert_eq!(cache.cached_lod_count(), 1);

        cache.set(ChunkLod::Eighth, handle);
        assert_eq!(cache.cached_lod_count(), 2);
    }

    #[test]
    fn set_overwrites_existing_handle_for_lod() {
        let mut meshes = Assets::<Mesh>::default();
        let first = dummy_mesh_handle(&mut meshes);
        let second = dummy_mesh_handle(&mut meshes);

        let mut cache = TerrainChunkLodCache::default();
        cache.set(ChunkLod::Quarter, first);
        cache.set(ChunkLod::Quarter, second.clone());

        assert_eq!(cache.get(ChunkLod::Quarter), Some(&second));
        assert_eq!(cache.cached_lod_count(), 1);
    }
}
