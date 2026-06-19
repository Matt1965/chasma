use bevy::prelude::*;
use std::collections::HashMap;

use super::chunk::{ChunkData, ChunkId};
use super::config::WorldConfig;
use super::coordinates::{ChunkCoord, ChunkLayout, WorldPosition};
use super::doodad::{
    ChunkDoodadStore, DoodadExclusionZone, DoodadId, DoodadInsertError, DoodadRecord,
    ProceduralDoodadKey,
};

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
/// Doodad records live in a parallel chunk-keyed store (ADR-015), not in
/// [`ChunkData`]. A required [`DoodadId`] → [`ChunkId`] index enables O(1)
/// instance lookup (ADR-017); all doodad mutations must keep it synchronized
/// with the chunk stores. The layout is a snapshot derived from [`WorldConfig`]
/// at initialization so position-based lookups do not require threading layout
/// through every call.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct WorldData {
    layout: ChunkLayout,
    chunks: HashMap<ChunkId, ChunkData>,
    doodads: HashMap<ChunkId, ChunkDoodadStore>,
    /// Required O(1) index: doodad id → owning chunk (ADR-017).
    ///
    /// Must stay synchronized with [`Self::doodads`] on every insert, move, and
    /// remove. Not optional — [`Self::get_doodad`] and authoring move/remove
    /// depend on this map.
    doodad_locations: HashMap<DoodadId, ChunkId>,
    /// O(1) procedural duplicate prevention: [`ProceduralDoodadKey`] → [`DoodadId`] (ADR-019).
    procedural_doodads: HashMap<ProceduralDoodadKey, DoodadId>,
    exclusion_zones: Vec<DoodadExclusionZone>,
    next_doodad_id: u64,
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
            doodads: HashMap::new(),
            doodad_locations: HashMap::new(),
            procedural_doodads: HashMap::new(),
            exclusion_zones: Vec::new(),
            next_doodad_id: 1,
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

    /// Evict a resident chunk's terrain. No-op if the chunk is not resident.
    ///
    /// Does not change authored extent, doodad records, or on-disk assets
    /// (ADR-012, ADR-015).
    pub fn remove(&mut self, chunk: ChunkId) {
        self.chunks.remove(&chunk);
    }

    /// Allocate the next monotonic [`DoodadId`].
    pub fn allocate_doodad_id(&mut self) -> DoodadId {
        let id = DoodadId::new(self.next_doodad_id);
        self.next_doodad_id += 1;
        id
    }

    /// Insert a doodad into the chunk-local store and update the id index.
    ///
    /// The record's [`super::doodad::DoodadPlacement::position`] chunk must match
    /// `chunk`. Doodads may exist when terrain [`ChunkData`] is not resident.
    pub fn insert_doodad(
        &mut self,
        chunk: ChunkId,
        record: DoodadRecord,
    ) -> Result<(), DoodadInsertError> {
        if record.placement.position.chunk != chunk.coord() {
            return Err(DoodadInsertError::ChunkPlacementMismatch);
        }
        self.doodads
            .entry(chunk)
            .or_default()
            .insert(record.clone());
        self.doodad_locations.insert(record.id, chunk);
        Ok(())
    }

    /// Remove a doodad from a chunk store and the id index. Returns `true` when removed.
    pub fn remove_doodad(&mut self, chunk: ChunkId, id: DoodadId) -> bool {
        let record = match self.doodads.get_mut(&chunk).and_then(|store| store.take(id)) {
            Some(record) => record,
            None => return false,
        };
        self.doodad_locations.remove(&id);
        self.unregister_procedural_doodad(&record);
        if self
            .doodads
            .get(&chunk)
            .is_some_and(|store| store.is_empty())
        {
            self.doodads.remove(&chunk);
        }
        true
    }

    /// Remove a doodad by id alone, returning the removed record (ADR-017).
    ///
    /// Uses the id index for O(1) chunk resolution; clears the index entry.
    pub fn remove_doodad_by_id(&mut self, id: DoodadId) -> Option<DoodadRecord> {
        let chunk = self.doodad_locations.remove(&id)?;
        let store = self.doodads.get_mut(&chunk)?;
        let record = store.take(id)?;
        if store.is_empty() {
            self.doodads.remove(&chunk);
        }
        self.unregister_procedural_doodad(&record);
        Some(record)
    }

    /// Lookup a materialized procedural doodad by stable pre-instance key (ADR-019).
    pub fn procedural_doodad_id(&self, key: &ProceduralDoodadKey) -> Option<DoodadId> {
        self.procedural_doodads.get(key).copied()
    }

    /// Register a procedural doodad key after successful materialization.
    pub(crate) fn register_procedural_doodad(
        &mut self,
        key: ProceduralDoodadKey,
        id: DoodadId,
    ) {
        self.procedural_doodads.insert(key, id);
    }

    fn unregister_procedural_doodad(&mut self, record: &DoodadRecord) {
        if let Some(key) = ProceduralDoodadKey::from_record(record) {
            self.procedural_doodads.remove(&key);
        }
    }

    fn reregister_procedural_doodad(&mut self, record: &DoodadRecord) {
        if let Some(key) = ProceduralDoodadKey::from_record(record) {
            self.procedural_doodads.insert(key, record.id);
        }
    }

    /// Sample resident heightfield height at an authoritative [`WorldPosition`] (ADR-005).
    ///
    /// Returns `None` when the owning chunk is not resident. Used by procedural
    /// terrain validation and placement finalization.
    pub fn sample_height_at_position(&self, position: WorldPosition) -> Option<f32> {
        let chunk_id = ChunkId::new(position.chunk);
        let data = self.chunks.get(&chunk_id)?;
        Some(data.heightfield.sample(position.local.0.x, position.local.0.z))
    }

    /// Borrow a doodad record by id via the O(1) id index (ADR-017).
    pub fn get_doodad(&self, id: DoodadId) -> Option<&DoodadRecord> {
        let chunk = self.doodad_locations.get(&id)?;
        self.doodads.get(chunk)?.get(id)
    }

    /// The chunk that currently stores a doodad instance.
    pub fn doodad_chunk(&self, id: DoodadId) -> Option<ChunkId> {
        self.doodad_locations.get(&id).copied()
    }

    /// Move a doodad to a new [`WorldPosition`], including cross-chunk moves (ADR-017).
    ///
    /// Preserves id, definition, source, metadata, rotation, and scale. Updates
    /// the id index when the owning chunk changes. For [`DoodadSource::Procedural`]
    /// records, re-keys [`ProceduralDoodadKey`] to the new owning chunk (ADR-019).
    pub fn relocate_doodad(
        &mut self,
        id: DoodadId,
        new_position: WorldPosition,
    ) -> Result<DoodadRecord, DoodadInsertError> {
        let old_chunk = self
            .doodad_locations
            .get(&id)
            .copied()
            .ok_or(DoodadInsertError::DoodadNotFound)?;

        let new_chunk = ChunkId::new(new_position.chunk);
        let mut record = self
            .doodads
            .get_mut(&old_chunk)
            .and_then(|store| store.take(id))
            .ok_or(DoodadInsertError::DoodadNotFound)?;

        if self
            .doodads
            .get(&old_chunk)
            .is_some_and(|store| store.is_empty())
        {
            self.doodads.remove(&old_chunk);
        }

        self.unregister_procedural_doodad(&record);

        record.placement.position = new_position;
        let moved = record.clone();
        self.insert_doodad(new_chunk, record)?;
        self.reregister_procedural_doodad(&moved);
        Ok(moved)
    }

    /// Borrow the doodad store for a chunk, if any records exist.
    pub fn doodads_in_chunk(&self, chunk: ChunkId) -> Option<&ChunkDoodadStore> {
        self.doodads.get(&chunk)
    }

    /// Append a world-scoped exclusion zone (data only; ADR-015).
    pub fn add_doodad_exclusion_zone(&mut self, zone: DoodadExclusionZone) {
        self.exclusion_zones.push(zone);
    }

    /// All registered doodad exclusion zones.
    pub fn doodad_exclusion_zones(&self) -> &[DoodadExclusionZone] {
        &self.exclusion_zones
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
        self.sample_height_at_position(position)
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
impl WorldData {
    /// Verify the id index matches all chunk stores (both directions + counts).
    pub(crate) fn assert_doodad_index_consistent(&self) {
        let indexed = self.doodad_locations.len();
        let stored: usize = self.doodads.values().map(|store| store.len()).sum();
        assert_eq!(
            indexed, stored,
            "index len {indexed} != stored record count {stored}"
        );

        for (chunk, store) in &self.doodads {
            for record in store.records() {
                assert_eq!(
                    self.doodad_chunk(record.id),
                    Some(*chunk),
                    "index missing or wrong for doodad {:?}",
                    record.id
                );
                assert!(
                    self.get_doodad(record.id).is_some(),
                    "get_doodad failed for indexed record {:?}",
                    record.id
                );
            }
        }

        for (id, chunk) in &self.doodad_locations {
            assert!(
                self.doodads
                    .get(chunk)
                    .and_then(|store| store.get(*id))
                    .is_some(),
                "index entry {:?} -> {:?} has no matching store record",
                id,
                chunk
            );
        }
    }

    /// Verify procedural duplicate keys point at live records with matching identity.
    pub(crate) fn assert_procedural_doodad_index_consistent(&self) {
        for (key, id) in &self.procedural_doodads {
            let record = self
                .get_doodad(*id)
                .unwrap_or_else(|| panic!("procedural key {:?} points at missing id {:?}", key, id));
            assert_eq!(
                ProceduralDoodadKey::from_record(record),
                Some(key.clone()),
                "procedural key mismatch for id {:?}",
                id
            );
        }
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

    mod doodad_tests {
        use super::*;
        use crate::world::{
            DoodadDefinitionId, DoodadKind, DoodadPlacement, DoodadRecord, DoodadSource,
            LocalPosition, WorldPosition,
        };

        fn chunk_id(x: i32, z: i32) -> ChunkId {
            ChunkId::new(ChunkCoord::new(x, z))
        }

        fn placement_at(chunk: ChunkCoord, local: Vec3) -> DoodadPlacement {
            DoodadPlacement::new(
                WorldPosition::new(chunk, LocalPosition::new(local)),
                Quat::from_rotation_y(0.5),
                Vec3::new(1.0, 1.0, 1.0),
            )
        }

        fn sample_record(id: DoodadId, chunk: ChunkCoord, source: DoodadSource) -> DoodadRecord {
            DoodadRecord::new(
                id,
                DoodadDefinitionId::new("tree_oak"),
                DoodadKind::Tree,
                placement_at(chunk, Vec3::new(64.0, 12.0, 128.0)),
                source,
            )
        }

        #[test]
        fn insert_doodad_into_chunk() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(1, 2);
            let id = world.allocate_doodad_id();
            let record = sample_record(id, chunk.coord(), DoodadSource::Authored);

            world.insert_doodad(chunk, record).unwrap();

            let store = world.doodads_in_chunk(chunk).unwrap();
            assert_eq!(store.len(), 1);
            assert_eq!(store.records()[0].id, id);
        }

        #[test]
        fn retrieve_doodads_by_chunk() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(0, 0);
            assert!(world.doodads_in_chunk(chunk).is_none());

            let id = world.allocate_doodad_id();
            world
                .insert_doodad(
                    chunk,
                    sample_record(id, chunk.coord(), DoodadSource::Authored),
                )
                .unwrap();

            assert_eq!(world.doodads_in_chunk(chunk).unwrap().len(), 1);
            assert!(world.doodads_in_chunk(chunk_id(1, 0)).is_none());
        }

        #[test]
        fn remove_doodad_by_id() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(3, 4);
            let id = world.allocate_doodad_id();
            world
                .insert_doodad(
                    chunk,
                    sample_record(id, chunk.coord(), DoodadSource::Authored),
                )
                .unwrap();

            assert!(world.remove_doodad(chunk, id));
            assert!(world.doodads_in_chunk(chunk).is_none());
            assert!(!world.remove_doodad(chunk, id));
        }

        #[test]
        fn insert_rejects_chunk_placement_mismatch() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(1, 1);
            let id = world.allocate_doodad_id();
            let record = sample_record(id, ChunkCoord::new(2, 2), DoodadSource::Authored);

            assert_eq!(
                world.insert_doodad(chunk, record),
                Err(DoodadInsertError::ChunkPlacementMismatch)
            );
            assert!(world.doodads_in_chunk(chunk).is_none());
        }

        #[test]
        fn authored_and_procedural_source_preserved() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(0, 1);

            let authored_id = world.allocate_doodad_id();
            world
                .insert_doodad(
                    chunk,
                    sample_record(authored_id, chunk.coord(), DoodadSource::Authored),
                )
                .unwrap();

            let proc_id = world.allocate_doodad_id();
            world
                .insert_doodad(
                    chunk,
                    sample_record(
                        proc_id,
                        chunk.coord(),
                        DoodadSource::Procedural { seed: 42 },
                    ),
                )
                .unwrap();

            let store = world.doodads_in_chunk(chunk).unwrap();
            assert_eq!(store.get(authored_id).unwrap().source, DoodadSource::Authored);
            assert_eq!(
                store.get(proc_id).unwrap().source,
                DoodadSource::Procedural { seed: 42 }
            );
        }

        #[test]
        fn exclusion_zone_storage() {
            let mut world = WorldData::new(layout());
            assert!(world.doodad_exclusion_zones().is_empty());

            let zone = crate::world::DoodadExclusionZone::new(
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
                ),
                32.0,
            );
            world.add_doodad_exclusion_zone(zone);

            assert_eq!(world.doodad_exclusion_zones().len(), 1);
            assert_eq!(world.doodad_exclusion_zones()[0].radius_meters, 32.0);
        }

        #[test]
        fn deterministic_ordering_by_doodad_id() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(5, 5);

            let id_c = world.allocate_doodad_id();
            let id_a = world.allocate_doodad_id();
            let id_b = world.allocate_doodad_id();

            world
                .insert_doodad(
                    chunk,
                    sample_record(id_c, chunk.coord(), DoodadSource::Authored),
                )
                .unwrap();
            world
                .insert_doodad(
                    chunk,
                    sample_record(id_a, chunk.coord(), DoodadSource::Authored),
                )
                .unwrap();
            world
                .insert_doodad(
                    chunk,
                    sample_record(id_b, chunk.coord(), DoodadSource::Authored),
                )
                .unwrap();

            let ids: Vec<_> = world
                .doodads_in_chunk(chunk)
                .unwrap()
                .records()
                .iter()
                .map(|r| r.id.raw())
                .collect();
            assert_eq!(ids, vec![id_c.raw(), id_a.raw(), id_b.raw()]);
        }

        #[test]
        fn monotonic_unique_id_allocation() {
            let mut world = WorldData::new(layout());
            let a = world.allocate_doodad_id();
            let b = world.allocate_doodad_id();
            let c = world.allocate_doodad_id();
            assert_eq!(a.raw(), 1);
            assert_eq!(b.raw(), 2);
            assert_eq!(c.raw(), 3);
            assert_ne!(a, b);
        }

        #[test]
        fn doodads_independent_of_terrain_residency() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(0, 0);

            let id = world.allocate_doodad_id();
            world
                .insert_doodad(
                    chunk,
                    sample_record(id, chunk.coord(), DoodadSource::Authored),
                )
                .unwrap();
            assert!(!world.is_chunk_loaded(chunk));
            assert_eq!(world.doodads_in_chunk(chunk).unwrap().len(), 1);

            world.insert(chunk, sample_chunk());
            assert!(world.is_chunk_loaded(chunk));
            assert_eq!(world.doodads_in_chunk(chunk).unwrap().len(), 1);

            world.remove(chunk);
            assert!(!world.is_chunk_loaded(chunk));
            assert_eq!(world.doodads_in_chunk(chunk).unwrap().len(), 1);
            assert!(world.remove_doodad(chunk, id));
        }
    }

    mod doodad_index_tests {
        use super::*;
        use crate::world::{
            create_doodad, move_doodad, remove_doodad, DoodadCatalog, DoodadDefinitionId,
            DoodadPlacementOverrides, DoodadSource, LocalPosition, WorldPosition,
        };

        fn layout() -> ChunkLayout {
            ChunkLayout {
                chunk_size_meters: 256.0,
                units_per_meter: 1.0,
            }
        }

        fn pos(chunk_x: i32, chunk_z: i32, local: Vec3) -> WorldPosition {
            WorldPosition::new(
                ChunkCoord::new(chunk_x, chunk_z),
                LocalPosition::new(local),
            )
        }

        #[test]
        fn index_integrity_after_create_via_authoring() {
            let catalog = DoodadCatalog::default();
            let mut world = WorldData::new(layout());

            let record = create_doodad(
                &catalog,
                &mut world,
                &DoodadDefinitionId::new("tree_oak"),
                pos(2, 3, Vec3::new(64.0, 0.0, 128.0)),
                DoodadSource::Authored,
                DoodadPlacementOverrides::default(),
            )
            .unwrap();

            assert_eq!(
                world.doodad_chunk(record.id),
                Some(ChunkId::new(ChunkCoord::new(2, 3)))
            );
            assert_eq!(world.get_doodad(record.id).unwrap().id, record.id);
            world.assert_doodad_index_consistent();
        }

        #[test]
        fn index_integrity_after_move_same_chunk() {
            let catalog = DoodadCatalog::default();
            let mut world = WorldData::new(layout());
            let chunk = ChunkId::new(ChunkCoord::new(0, 0));
            let record = create_doodad(
                &catalog,
                &mut world,
                &DoodadDefinitionId::new("rock_small"),
                pos(0, 0, Vec3::new(10.0, 0.0, 20.0)),
                DoodadSource::Authored,
                DoodadPlacementOverrides::default(),
            )
            .unwrap();
            world.assert_doodad_index_consistent();

            move_doodad(&mut world, record.id, pos(0, 0, Vec3::new(200.0, 0.0, 50.0)))
                .unwrap();

            assert_eq!(world.doodad_chunk(record.id), Some(chunk));
            world.assert_doodad_index_consistent();
        }

        #[test]
        fn index_integrity_after_move_cross_chunk() {
            let catalog = DoodadCatalog::default();
            let mut world = WorldData::new(layout());
            let record = create_doodad(
                &catalog,
                &mut world,
                &DoodadDefinitionId::new("bush_scrub"),
                pos(0, 0, Vec3::new(1.0, 0.0, 1.0)),
                DoodadSource::Authored,
                DoodadPlacementOverrides::default(),
            )
            .unwrap();
            world.assert_doodad_index_consistent();

            let new_chunk = ChunkId::new(ChunkCoord::new(1, 0));
            move_doodad(
                &mut world,
                record.id,
                pos(1, 0, Vec3::new(128.0, 0.0, 128.0)),
            )
            .unwrap();

            assert_eq!(world.doodad_chunk(record.id), Some(new_chunk));
            assert!(world
                .doodads_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
                .is_none());
            world.assert_doodad_index_consistent();
        }

        #[test]
        fn index_integrity_after_remove_by_id() {
            let catalog = DoodadCatalog::default();
            let mut world = WorldData::new(layout());
            let record = create_doodad(
                &catalog,
                &mut world,
                &DoodadDefinitionId::new("ruin_stone"),
                pos(4, 5, Vec3::new(64.0, 0.0, 64.0)),
                DoodadSource::Authored,
                DoodadPlacementOverrides::default(),
            )
            .unwrap();
            world.assert_doodad_index_consistent();

            remove_doodad(&mut world, record.id).unwrap();

            assert!(world.doodad_chunk(record.id).is_none());
            assert!(world.get_doodad(record.id).is_none());
            world.assert_doodad_index_consistent();
        }

        #[test]
        fn index_integrity_after_remove_by_chunk() {
            let catalog = DoodadCatalog::default();
            let mut world = WorldData::new(layout());
            let chunk = ChunkId::new(ChunkCoord::new(3, 3));
            let record = create_doodad(
                &catalog,
                &mut world,
                &DoodadDefinitionId::new("resource_node_iron"),
                pos(3, 3, Vec3::new(32.0, 0.0, 32.0)),
                DoodadSource::Authored,
                DoodadPlacementOverrides::default(),
            )
            .unwrap();
            world.assert_doodad_index_consistent();

            assert!(world.remove_doodad(chunk, record.id));

            assert!(world.doodad_chunk(record.id).is_none());
            assert!(world.get_doodad(record.id).is_none());
            world.assert_doodad_index_consistent();
        }
    }

    mod procedural_doodad_index_tests {
        use super::*;
        use crate::world::{DoodadSpawnCandidate, materialize_candidates_with_options, move_doodad, DoodadCatalog, DoodadDefinitionId,
            DoodadSource, LocalPosition, MaterializationOptions, ProceduralDoodadKey};
        use bevy::prelude::{Quat, Vec3};

        fn layout() -> ChunkLayout {
            ChunkLayout {
                chunk_size_meters: 256.0,
                units_per_meter: 1.0,
            }
        }

        fn insert_flat(world: &mut WorldData, x: i32, z: i32, height: f32) {
            let samples = vec![height; 9];
            let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
            world.insert(
                ChunkId::new(ChunkCoord::new(x, z)),
                ChunkData::new(heightfield, Vec::new()),
            );
        }

        fn pos(chunk_x: i32, chunk_z: i32, local: Vec3) -> WorldPosition {
            WorldPosition::new(
                ChunkCoord::new(chunk_x, chunk_z),
                LocalPosition::new(local),
            )
        }

        fn materialize_one(world: &mut WorldData, candidate: &DoodadSpawnCandidate) {
            let catalog = DoodadCatalog::default();
            materialize_candidates_with_options(
                &catalog,
                world,
                std::slice::from_ref(candidate),
                &MaterializationOptions::raw(),
            );
        }

        #[test]
        fn procedural_index_consistent_after_same_chunk_move() {
            let mut world = WorldData::new(layout());
            insert_flat(&mut world, 0, 0, 10.0);
            let candidate = DoodadSpawnCandidate {
                definition_id: DoodadDefinitionId::new("tree_oak"),
                source: DoodadSource::Procedural { seed: 42 },
                position: pos(0, 0, Vec3::new(128.0, 0.0, 128.0)),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            };
            let old_key = ProceduralDoodadKey::from_candidate(&candidate).unwrap();
            materialize_one(&mut world, &candidate);
            let id = world.procedural_doodad_id(&old_key).unwrap();

            move_doodad(
                &mut world,
                id,
                pos(0, 0, Vec3::new(64.0, 0.0, 64.0)),
            )
            .unwrap();

            world.assert_procedural_doodad_index_consistent();
            assert_eq!(world.procedural_doodad_id(&old_key), Some(id));
        }

        #[test]
        fn procedural_key_updates_after_cross_chunk_move() {
            let mut world = WorldData::new(layout());
            insert_flat(&mut world, 0, 0, 10.0);
            insert_flat(&mut world, 1, 0, 10.0);
            let candidate = DoodadSpawnCandidate {
                definition_id: DoodadDefinitionId::new("rock_small"),
                source: DoodadSource::Procedural { seed: 7 },
                position: pos(0, 0, Vec3::new(128.0, 0.0, 128.0)),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            };
            let old_key = ProceduralDoodadKey::from_candidate(&candidate).unwrap();
            materialize_one(&mut world, &candidate);
            let id = world.procedural_doodad_id(&old_key).unwrap();

            move_doodad(
                &mut world,
                id,
                pos(1, 0, Vec3::new(128.0, 0.0, 128.0)),
            )
            .unwrap();

            let new_key = ProceduralDoodadKey::new(
                ChunkCoord::new(1, 0),
                candidate.definition_id.clone(),
                7,
            );

            world.assert_procedural_doodad_index_consistent();
            assert!(world.procedural_doodad_id(&old_key).is_none());
            assert_eq!(world.procedural_doodad_id(&new_key), Some(id));
        }

        #[test]
        fn rematerialize_at_new_location_skips_after_cross_chunk_move() {
            let mut world = WorldData::new(layout());
            insert_flat(&mut world, 0, 0, 10.0);
            insert_flat(&mut world, 1, 0, 10.0);
            let candidate = DoodadSpawnCandidate {
                definition_id: DoodadDefinitionId::new("bush_scrub"),
                source: DoodadSource::Procedural { seed: 99 },
                position: pos(0, 0, Vec3::new(64.0, 0.0, 64.0)),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            };
            let old_key = ProceduralDoodadKey::from_candidate(&candidate).unwrap();
            materialize_one(&mut world, &candidate);
            let id = world.procedural_doodad_id(&old_key).unwrap();

            move_doodad(
                &mut world,
                id,
                pos(1, 0, Vec3::new(64.0, 0.0, 64.0)),
            )
            .unwrap();

            let candidate_at_new = DoodadSpawnCandidate {
                position: pos(1, 0, Vec3::new(64.0, 0.0, 64.0)),
                ..candidate
            };
            let new_key = ProceduralDoodadKey::from_candidate(&candidate_at_new).unwrap();

            let catalog = DoodadCatalog::default();
            let report = materialize_candidates_with_options(
                &catalog,
                &mut world,
                &[candidate_at_new],
                &MaterializationOptions::raw(),
            );

            assert_eq!(report.skipped_duplicate, 1);
            assert_eq!(report.inserted, 0);
            assert_eq!(world.procedural_doodad_id(&new_key), Some(id));
            assert!(world.procedural_doodad_id(&old_key).is_none());
        }
    }
}
