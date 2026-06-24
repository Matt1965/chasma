use bevy::prelude::*;
use std::collections::HashMap;

use super::biome::{BiomeMask, BiomeSample};
use super::chunk::{ChunkData, ChunkId};
use super::config::WorldConfig;
use super::coordinates::{ChunkCoord, ChunkLayout, WorldPosition};
use super::doodad::{
    ChunkDoodadStore, DoodadExclusionZone, DoodadId, DoodadInsertError, DoodadRecord,
    ProceduralDoodadKey,
};
use super::unit::{
    ChunkUnitStore, UnitId, UnitInsertError, UnitRecord,
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
/// with the chunk stores. Unit records follow the same pattern (ADR-027 U2):
/// chunk-keyed [`ChunkUnitStore`] plus a required [`UnitId`] → [`ChunkId`]
/// index, independent of terrain residency. An optional [`BiomeMask`] holds world-scale biome
/// authority (ADR-024), independent of terrain residency. The layout is a
/// snapshot derived from [`WorldConfig`] at initialization so position-based
/// lookups do not require threading layout through every call.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct WorldData {
    layout: ChunkLayout,
    chunks: HashMap<ChunkId, ChunkData>,
    doodads: HashMap<ChunkId, ChunkDoodadStore>,
    /// World-scale biome classification raster (ADR-024).
    biome_mask: Option<BiomeMask>,
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
    units: HashMap<ChunkId, ChunkUnitStore>,
    /// Required O(1) index: unit id → owning chunk (ADR-027 U2).
    unit_locations: HashMap<UnitId, ChunkId>,
    next_unit_id: u64,
    authored_extent: Option<ChunkExtent>,
    /// Deferred MoveTo orders resolved before movement (ADR-037 U12).
    command_buffer: super::movement::feel::UnitCommandBuffer,
    /// Per-unit direction smoothing cache (ADR-037 U12).
    #[reflect(ignore)]
    movement_smoothing: super::movement::feel::MovementSmoothingState,
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
            biome_mask: None,
            doodad_locations: HashMap::new(),
            procedural_doodads: HashMap::new(),
            exclusion_zones: Vec::new(),
            next_doodad_id: 1,
            units: HashMap::new(),
            unit_locations: HashMap::new(),
            next_unit_id: 1,
            authored_extent: None,
            command_buffer: super::movement::feel::UnitCommandBuffer::default(),
            movement_smoothing: super::movement::feel::MovementSmoothingState::default(),
        }
    }

    /// Borrow the deferred unit command buffer (ADR-037 U12).
    pub fn command_buffer(&self) -> &super::movement::feel::UnitCommandBuffer {
        &self.command_buffer
    }

    /// Mutably borrow the deferred unit command buffer (ADR-037 U12).
    pub fn command_buffer_mut(&mut self) -> &mut super::movement::feel::UnitCommandBuffer {
        &mut self.command_buffer
    }

    /// Mutably borrow per-unit movement smoothing state (ADR-037 U12).
    pub fn movement_smoothing_mut(
        &mut self,
    ) -> &mut super::movement::feel::MovementSmoothingState {
        &mut self.movement_smoothing
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
    /// Does not change authored extent, doodad records, unit records, or on-disk assets
    /// (ADR-012, ADR-015, ADR-027).
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

    /// Allocate the next monotonic [`UnitId`].
    pub fn allocate_unit_id(&mut self) -> UnitId {
        let id = UnitId::new(self.next_unit_id);
        self.next_unit_id += 1;
        id
    }

    /// Insert a unit into the chunk-local store and update the id index.
    ///
    /// The record's [`super::unit::UnitPlacement::position`] chunk must match
    /// `chunk`. Units may exist when terrain [`ChunkData`] is not resident.
    pub fn insert_unit(
        &mut self,
        chunk: ChunkId,
        record: UnitRecord,
    ) -> Result<(), UnitInsertError> {
        if record.placement.position.chunk != chunk.coord() {
            return Err(UnitInsertError::ChunkPlacementMismatch);
        }
        self.units
            .entry(chunk)
            .or_default()
            .insert(record.clone());
        self.unit_locations.insert(record.id, chunk);
        Ok(())
    }

    /// Remove a unit from a chunk store and the id index. Returns `true` when removed.
    pub fn remove_unit(&mut self, chunk: ChunkId, id: UnitId) -> bool {
        if self.units.get_mut(&chunk).and_then(|store| store.take(id)).is_none() {
            return false;
        }
        self.unit_locations.remove(&id);
        if self
            .units
            .get(&chunk)
            .is_some_and(|store| store.is_empty())
        {
            self.units.remove(&chunk);
        }
        true
    }

    /// Remove a unit by id alone, returning the removed record (ADR-027 U2).
    pub fn remove_unit_by_id(&mut self, id: UnitId) -> Option<UnitRecord> {
        let chunk = self.unit_locations.remove(&id)?;
        let store = self.units.get_mut(&chunk)?;
        let record = store.take(id)?;
        if store.is_empty() {
            self.units.remove(&chunk);
        }
        Some(record)
    }

    /// Borrow a unit record by id via the O(1) id index (ADR-027 U2).
    pub fn get_unit(&self, id: UnitId) -> Option<&UnitRecord> {
        let chunk = self.unit_locations.get(&id)?;
        self.units.get(chunk)?.get(id)
    }

    /// The chunk that currently stores a unit instance.
    pub fn unit_chunk(&self, id: UnitId) -> Option<ChunkId> {
        self.unit_locations.get(&id).copied()
    }

    /// All unit ids sorted for deterministic simulation iteration (ADR-030).
    pub fn sorted_unit_ids(&self) -> Vec<UnitId> {
        let mut ids: Vec<_> = self.unit_locations.keys().copied().collect();
        ids.sort();
        ids
    }

    /// All doodad ids sorted for deterministic iteration (ADR-045 dev scenes).
    pub fn sorted_doodad_ids(&self) -> Vec<DoodadId> {
        let mut ids: Vec<_> = self.doodad_locations.keys().copied().collect();
        ids.sort();
        ids
    }

    #[cfg(feature = "dev")]
    /// Remove all unit and doodad instances without touching terrain (ADR-045).
    pub fn dev_clear_units_and_doodads(&mut self) {
        for id in self.sorted_unit_ids() {
            let _ = self.remove_unit_by_id(id);
        }
        for id in self.sorted_doodad_ids() {
            let _ = self.remove_doodad_by_id(id);
        }
        self.procedural_doodads.clear();
        let _ = self.command_buffer_mut().take_pending_sorted();
        self.movement_smoothing_mut().clear_all();
    }

    #[cfg(feature = "dev")]
    /// Ensure monotonic id allocators stay above restored instance ids (ADR-045).
    pub fn dev_restore_id_counters(&mut self, next_unit_id: u64, next_doodad_id: u64) {
        self.next_unit_id = self.next_unit_id.max(next_unit_id);
        self.next_doodad_id = self.next_doodad_id.max(next_doodad_id);
    }

    #[cfg(feature = "dev")]
    pub fn dev_next_unit_id(&self) -> u64 {
        self.next_unit_id
    }

    #[cfg(feature = "dev")]
    pub fn dev_next_doodad_id(&self) -> u64 {
        self.next_doodad_id
    }

    #[cfg(feature = "dev")]
    pub fn dev_reregister_procedural_doodad(&mut self, record: &DoodadRecord) {
        self.reregister_procedural_doodad(record);
    }

    /// Update simulation state without changing placement (ADR-030).
    pub fn set_unit_state(&mut self, id: UnitId, state: super::unit::UnitState) -> Result<(), UnitInsertError> {
        let chunk = self
            .unit_locations
            .get(&id)
            .copied()
            .ok_or(UnitInsertError::UnitNotFound)?;
        let store = self.units.get_mut(&chunk).ok_or(UnitInsertError::UnitNotFound)?;
        let record = store.get_mut(id).ok_or(UnitInsertError::UnitNotFound)?;
        record.state = state;
        Ok(())
    }

    /// Move a unit to a new [`WorldPosition`], including cross-chunk moves (ADR-027 U2).
    ///
    /// Preserves id, definition, source, metadata, rotation, and state. Updates
    /// the id index when the owning chunk changes.
    pub fn relocate_unit(
        &mut self,
        id: UnitId,
        new_position: WorldPosition,
    ) -> Result<UnitRecord, UnitInsertError> {
        let old_chunk = self
            .unit_locations
            .get(&id)
            .copied()
            .ok_or(UnitInsertError::UnitNotFound)?;

        let new_chunk = ChunkId::new(new_position.chunk);
        let mut record = self
            .units
            .get_mut(&old_chunk)
            .and_then(|store| store.take(id))
            .ok_or(UnitInsertError::UnitNotFound)?;

        if self
            .units
            .get(&old_chunk)
            .is_some_and(|store| store.is_empty())
        {
            self.units.remove(&old_chunk);
        }

        record.placement.position = new_position;
        let moved = record.clone();
        self.insert_unit(new_chunk, record)?;
        Ok(moved)
    }

    /// Borrow the unit store for a chunk, if any records exist.
    pub fn units_in_chunk(&self, chunk: ChunkId) -> Option<&ChunkUnitStore> {
        self.units.get(&chunk)
    }

    /// Units within `radius_meters` of `position` (XZ distance), sorted by [`UnitId`].
    ///
    /// Scans chunk-local stores in a bounded neighborhood only — not the full world.
    pub fn query_units_in_radius(
        &self,
        position: WorldPosition,
        radius_meters: f32,
        exclude: Option<UnitId>,
    ) -> Vec<UnitId> {
        let layout = self.layout();
        let center = position.to_global(layout);
        let center_xz = Vec2::new(center.x, center.z);
        let chunk_span = (radius_meters / layout.chunk_size_units())
            .ceil()
            .max(0.0) as i32
            + 1;

        let mut matches = Vec::new();
        for dz in -chunk_span..=chunk_span {
            for dx in -chunk_span..=chunk_span {
                let chunk_coord = ChunkCoord::new(position.chunk.x + dx, position.chunk.z + dz);
                let Some(store) = self.units_in_chunk(ChunkId::new(chunk_coord)) else {
                    continue;
                };
                for record in store.records() {
                    if exclude == Some(record.id) {
                        continue;
                    }
                    let global = record.placement.position.to_global(layout);
                    let xz = Vec2::new(global.x, global.z);
                    if center_xz.distance(xz) <= radius_meters {
                        matches.push(record.id);
                    }
                }
            }
        }
        matches.sort_unstable();
        matches
    }

    /// Set the authoritative world-scale biome mask (ADR-024).
    pub fn set_biome_mask(&mut self, mask: BiomeMask) {
        self.biome_mask = Some(mask);
    }

    /// Borrow the biome mask, if imported.
    pub fn biome_mask(&self) -> Option<&BiomeMask> {
        self.biome_mask.as_ref()
    }

    /// Sample biome classification at an authoritative [`WorldPosition`] (ADR-024).
    ///
    /// Returns `None` when no mask is loaded. Does not require terrain residency.
    pub fn biome_at(&self, position: WorldPosition) -> Option<BiomeSample> {
        let mask = self.biome_mask.as_ref()?;
        Some(mask.sample_at_global(position.to_global(self.layout)))
    }

    /// Sample biome classification at a global render-space position (ADR-024).
    pub fn sample_biome_at_global(&self, global: Vec3) -> Option<BiomeSample> {
        self.biome_mask.as_ref().map(|mask| mask.sample_at_global(global))
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

    /// Verify the unit id index matches all chunk stores (both directions + counts).
    pub(crate) fn assert_unit_index_consistent(&self) {
        let indexed = self.unit_locations.len();
        let stored: usize = self.units.values().map(|store| store.len()).sum();
        assert_eq!(
            indexed, stored,
            "unit index len {indexed} != stored record count {stored}"
        );

        for (chunk, store) in &self.units {
            for record in store.records() {
                assert_eq!(
                    self.unit_chunk(record.id),
                    Some(*chunk),
                    "index missing or wrong for unit {:?}",
                    record.id
                );
                assert!(
                    self.get_unit(record.id).is_some(),
                    "get_unit failed for indexed record {:?}",
                    record.id
                );
            }
        }

        for (id, chunk) in &self.unit_locations {
            assert!(
                self.units
                    .get(chunk)
                    .and_then(|store| store.get(*id))
                    .is_some(),
                "unit index entry {:?} -> {:?} has no matching store record",
                id,
                chunk
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

    mod biome_tests {
        use super::*;
        use crate::world::{
            BiomeColorMapping, BiomeId, BiomeMask, BiomeMaskBounds, LocalPosition, WorldPosition,
        };

        fn sample_mask() -> BiomeMask {
            BiomeMask::from_rgba_rows(
                2,
                2,
                BiomeMaskBounds::new(0.0, 0.0, 512.0, 512.0),
                &[
                    255, 0, 0, 255, 0, 255, 0, 255, //
                    0, 0, 255, 255, 255, 255, 0, 255, //
                ],
                4,
                &BiomeColorMapping::starter(),
            )
            .unwrap()
        }

        #[test]
        fn biome_at_none_without_mask() {
            let world = WorldData::new(layout());
            let position = WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(10.0, 0.0, 10.0)),
            );
            assert!(world.biome_at(position).is_none());
        }

        #[test]
        fn biome_at_works_without_terrain_residency() {
            let mut world = WorldData::new(layout());
            world.set_biome_mask(sample_mask());
            let position = WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(10.0, 0.0, 10.0)),
            );
            let sample = world.biome_at(position).unwrap();
            assert_eq!(sample.biome, BiomeId::Desert);
        }

        #[test]
        fn biome_at_edge_and_center_are_consistent() {
            let mut world = WorldData::new(layout());
            world.set_biome_mask(sample_mask());

            let southwest = world
                .biome_at(WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::ZERO),
                ))
                .unwrap();
            assert_eq!(southwest.biome, BiomeId::Desert);

            let northeast = world
                .sample_biome_at_global(Vec3::new(511.0, 0.0, 511.0))
                .unwrap();
            assert_eq!(northeast.biome, BiomeId::Plains);

            let center = world
                .sample_biome_at_global(Vec3::new(256.0, 0.0, 256.0))
                .unwrap();
            assert_eq!(center.biome, BiomeId::Plains);
        }

        #[test]
        fn biome_sampling_is_deterministic() {
            let mut world = WorldData::new(layout());
            world.set_biome_mask(sample_mask());
            let global = Vec3::new(100.0, 0.0, 200.0);
            assert_eq!(
                world.sample_biome_at_global(global),
                world.sample_biome_at_global(global)
            );
        }
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

    mod unit_tests {
        use super::*;
        use crate::world::{
            LocalPosition, UnitDefinitionId, UnitMetadata, UnitPlacement, UnitRecord,
            UnitSource, UnitState,
        };

        fn chunk_id(x: i32, z: i32) -> ChunkId {
            ChunkId::new(ChunkCoord::new(x, z))
        }

        fn placement_at(chunk: ChunkCoord, local: Vec3) -> UnitPlacement {
            UnitPlacement::new(
                WorldPosition::new(chunk, LocalPosition::new(local)),
                Quat::from_rotation_y(0.25),
            )
        }

        fn sample_record(id: UnitId, chunk: ChunkCoord, source: UnitSource) -> UnitRecord {
            let mut record = UnitRecord::new(
                id,
                UnitDefinitionId::new("wolf"),
                placement_at(chunk, Vec3::new(64.0, 0.0, 128.0)),
                source,
                crate::world::default_ownership_for_source(source),
            );
            record.state = UnitState::Idle;
            record.metadata = UnitMetadata;
            record
        }

        #[test]
        fn allocate_unit_id_is_monotonic() {
            let mut world = WorldData::new(layout());
            let a = world.allocate_unit_id();
            let b = world.allocate_unit_id();
            let c = world.allocate_unit_id();
            assert_eq!(a.raw(), 1);
            assert_eq!(b.raw(), 2);
            assert_eq!(c.raw(), 3);
            assert_ne!(a, b);
        }

        #[test]
        fn insert_unit_into_chunk() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(1, 2);
            let id = world.allocate_unit_id();
            let record = sample_record(id, chunk.coord(), UnitSource::Authored);

            world.insert_unit(chunk, record).unwrap();

            let store = world.units_in_chunk(chunk).unwrap();
            assert_eq!(store.len(), 1);
            assert_eq!(store.records()[0].id, id);
            world.assert_unit_index_consistent();
        }

        #[test]
        fn retrieve_units_by_chunk() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(0, 0);
            assert!(world.units_in_chunk(chunk).is_none());

            let id = world.allocate_unit_id();
            world
                .insert_unit(
                    chunk,
                    sample_record(id, chunk.coord(), UnitSource::Authored),
                )
                .unwrap();

            assert_eq!(world.units_in_chunk(chunk).unwrap().len(), 1);
            assert!(world.units_in_chunk(chunk_id(1, 0)).is_none());
        }

        #[test]
        fn get_unit_by_id() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(2, 3);
            let id = world.allocate_unit_id();
            world
                .insert_unit(
                    chunk,
                    sample_record(id, chunk.coord(), UnitSource::Authored),
                )
                .unwrap();

            assert_eq!(world.get_unit(id).unwrap().id, id);
            assert!(world.get_unit(UnitId::new(999)).is_none());
        }

        #[test]
        fn remove_unit_by_chunk_and_id() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(3, 4);
            let id = world.allocate_unit_id();
            world
                .insert_unit(
                    chunk,
                    sample_record(id, chunk.coord(), UnitSource::Authored),
                )
                .unwrap();

            assert!(world.remove_unit(chunk, id));
            assert!(world.units_in_chunk(chunk).is_none());
            assert!(!world.remove_unit(chunk, id));
            world.assert_unit_index_consistent();
        }

        #[test]
        fn remove_unit_by_id() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(5, 6);
            let id = world.allocate_unit_id();
            world
                .insert_unit(
                    chunk,
                    sample_record(id, chunk.coord(), UnitSource::Procedural { seed: 7 }),
                )
                .unwrap();

            let removed = world.remove_unit_by_id(id).unwrap();
            assert_eq!(removed.id, id);
            assert!(world.unit_chunk(id).is_none());
            world.assert_unit_index_consistent();
        }

        #[test]
        fn insert_rejects_chunk_placement_mismatch() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(1, 1);
            let id = world.allocate_unit_id();
            let record = sample_record(id, ChunkCoord::new(2, 2), UnitSource::Authored);

            assert_eq!(
                world.insert_unit(chunk, record),
                Err(crate::world::UnitInsertError::ChunkPlacementMismatch)
            );
            assert!(world.units_in_chunk(chunk).is_none());
        }

        #[test]
        fn relocate_within_same_chunk() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(0, 0);
            let id = world.allocate_unit_id();
            let record = sample_record(id, chunk.coord(), UnitSource::Authored);
            world.insert_unit(chunk, record).unwrap();
            world.assert_unit_index_consistent();

            let new_pos = WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(200.0, 5.0, 50.0)),
            );
            let moved = world.relocate_unit(id, new_pos).unwrap();

            assert_eq!(moved.id, id);
            assert_eq!(moved.placement.position, new_pos);
            assert_eq!(world.unit_chunk(id), Some(chunk));
            world.assert_unit_index_consistent();
        }

        #[test]
        fn relocate_across_chunk_boundary() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(0, 0);
            let id = world.allocate_unit_id();
            world
                .insert_unit(
                    chunk,
                    sample_record(id, chunk.coord(), UnitSource::Authored),
                )
                .unwrap();
            world.assert_unit_index_consistent();

            let new_chunk = chunk_id(1, 0);
            let new_pos = WorldPosition::new(
                ChunkCoord::new(1, 0),
                LocalPosition::new(Vec3::new(128.0, 0.0, 128.0)),
            );
            let moved = world.relocate_unit(id, new_pos).unwrap();

            assert_eq!(moved.placement.position, new_pos);
            assert_eq!(world.unit_chunk(id), Some(new_chunk));
            assert!(world.units_in_chunk(chunk).is_none());
            world.assert_unit_index_consistent();
        }

        #[test]
        fn unit_id_preserved_after_relocate() {
            let mut world = WorldData::new(layout());
            let id = world.allocate_unit_id();
            world
                .insert_unit(
                    chunk_id(0, 0),
                    sample_record(id, ChunkCoord::new(0, 0), UnitSource::Authored),
                )
                .unwrap();

            world
                .relocate_unit(
                    id,
                    WorldPosition::new(
                        ChunkCoord::new(3, 4),
                        LocalPosition::new(Vec3::new(50.0, 0.0, 50.0)),
                    ),
                )
                .unwrap();

            assert_eq!(world.get_unit(id).unwrap().id, id);
            world.assert_unit_index_consistent();
        }

        #[test]
        fn state_preserved_after_relocate() {
            let mut world = WorldData::new(layout());
            let id = world.allocate_unit_id();
            let mut record =
                sample_record(id, ChunkCoord::new(0, 0), UnitSource::Authored);
            record.state = UnitState::Idle;
            world.insert_unit(chunk_id(0, 0), record).unwrap();

            world
                .relocate_unit(
                    id,
                    WorldPosition::new(
                        ChunkCoord::new(1, 0),
                        LocalPosition::new(Vec3::new(1.0, 0.0, 1.0)),
                    ),
                )
                .unwrap();

            assert_eq!(world.get_unit(id).unwrap().state, UnitState::Idle);
        }

        #[test]
        fn source_preserved_after_relocate() {
            let mut world = WorldData::new(layout());
            let source = UnitSource::Procedural { seed: 42 };
            let id = world.allocate_unit_id();
            world
                .insert_unit(
                    chunk_id(0, 0),
                    sample_record(id, ChunkCoord::new(0, 0), source),
                )
                .unwrap();

            world
                .relocate_unit(
                    id,
                    WorldPosition::new(
                        ChunkCoord::new(2, 0),
                        LocalPosition::new(Vec3::new(1.0, 0.0, 1.0)),
                    ),
                )
                .unwrap();

            assert_eq!(world.get_unit(id).unwrap().source, source);
        }

        #[test]
        fn metadata_preserved_after_relocate() {
            let mut world = WorldData::new(layout());
            let id = world.allocate_unit_id();
            world
                .insert_unit(
                    chunk_id(0, 0),
                    sample_record(id, ChunkCoord::new(0, 0), UnitSource::Authored),
                )
                .unwrap();
            assert_eq!(world.get_unit(id).unwrap().metadata, UnitMetadata);

            world
                .relocate_unit(
                    id,
                    WorldPosition::new(
                        ChunkCoord::new(1, 0),
                        LocalPosition::new(Vec3::new(1.0, 0.0, 1.0)),
                    ),
                )
                .unwrap();

            assert_eq!(world.get_unit(id).unwrap().metadata, UnitMetadata);
        }

        #[test]
        fn deterministic_ordering_by_unit_id() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(5, 5);

            let id_c = world.allocate_unit_id();
            let id_a = world.allocate_unit_id();
            let id_b = world.allocate_unit_id();

            world
                .insert_unit(
                    chunk,
                    sample_record(id_c, chunk.coord(), UnitSource::Authored),
                )
                .unwrap();
            world
                .insert_unit(
                    chunk,
                    sample_record(id_a, chunk.coord(), UnitSource::Authored),
                )
                .unwrap();
            world
                .insert_unit(
                    chunk,
                    sample_record(id_b, chunk.coord(), UnitSource::Authored),
                )
                .unwrap();

            let ids: Vec<_> = world
                .units_in_chunk(chunk)
                .unwrap()
                .records()
                .iter()
                .map(|r| r.id.raw())
                .collect();
            assert_eq!(ids, vec![id_c.raw(), id_a.raw(), id_b.raw()]);
        }

        #[test]
        fn query_units_in_radius_returns_sorted_neighbors() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(0, 0);
            let center = WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(50.0, 0.0, 50.0)),
            );

            let id_near_b = world.allocate_unit_id();
            let id_near_a = world.allocate_unit_id();
            world
                .insert_unit(
                    chunk,
                    UnitRecord::new(
                        id_near_b,
                        UnitDefinitionId::new("wolf"),
                        UnitPlacement::new(
                            WorldPosition::new(
                                ChunkCoord::new(0, 0),
                                LocalPosition::new(Vec3::new(52.0, 0.0, 50.0)),
                            ),
                            Quat::IDENTITY,
                        ),
                        UnitSource::Authored,
                        crate::world::UnitOwnership::neutral(),
                    ),
                )
                .unwrap();
            world
                .insert_unit(
                    chunk,
                    UnitRecord::new(
                        id_near_a,
                        UnitDefinitionId::new("wolf"),
                        UnitPlacement::new(
                            WorldPosition::new(
                                ChunkCoord::new(0, 0),
                                LocalPosition::new(Vec3::new(48.0, 0.0, 50.0)),
                            ),
                            Quat::IDENTITY,
                        ),
                        UnitSource::Authored,
                        crate::world::UnitOwnership::neutral(),
                    ),
                )
                .unwrap();
            let id_far = world.allocate_unit_id();
            world
                .insert_unit(
                    chunk,
                    UnitRecord::new(
                        id_far,
                        UnitDefinitionId::new("wolf"),
                        UnitPlacement::new(
                            WorldPosition::new(
                                ChunkCoord::new(0, 0),
                                LocalPosition::new(Vec3::new(80.0, 0.0, 80.0)),
                            ),
                            Quat::IDENTITY,
                        ),
                        UnitSource::Authored,
                        crate::world::UnitOwnership::neutral(),
                    ),
                )
                .unwrap();

            let nearby = world.query_units_in_radius(center, 5.0, None);
            assert_eq!(nearby.len(), 2);
            assert!(nearby[0] < nearby[1]);
            assert_eq!(nearby, vec![id_near_b, id_near_a]);

            let excluding = world.query_units_in_radius(center, 5.0, Some(id_near_b));
            assert_eq!(excluding, vec![id_near_a]);
        }

        #[test]
        fn units_independent_of_terrain_residency() {
            let mut world = WorldData::new(layout());
            let chunk = chunk_id(0, 0);

            let id = world.allocate_unit_id();
            world
                .insert_unit(
                    chunk,
                    sample_record(id, chunk.coord(), UnitSource::Authored),
                )
                .unwrap();
            assert!(!world.is_chunk_loaded(chunk));
            assert_eq!(world.units_in_chunk(chunk).unwrap().len(), 1);

            world.insert(chunk, sample_chunk());
            assert!(world.is_chunk_loaded(chunk));
            assert_eq!(world.units_in_chunk(chunk).unwrap().len(), 1);

            world.remove(chunk);
            assert!(!world.is_chunk_loaded(chunk));
            assert_eq!(world.units_in_chunk(chunk).unwrap().len(), 1);
            assert!(world.remove_unit(chunk, id));
            world.assert_unit_index_consistent();
        }
    }

    mod unit_index_tests {
        use super::*;
        use crate::world::{
            create_unit, move_unit, remove_unit, LocalPosition, UnitCatalog, UnitDefinitionId,
            UnitSource,
        };

        fn pos(chunk_x: i32, chunk_z: i32, local: Vec3) -> WorldPosition {
            WorldPosition::new(
                ChunkCoord::new(chunk_x, chunk_z),
                LocalPosition::new(local),
            )
        }

        #[test]
        fn index_integrity_after_create_via_authoring() {
            let catalog = UnitCatalog::default();
            let mut world = WorldData::new(layout());

            let record = create_unit(
                &catalog,
                &mut world,
                &UnitDefinitionId::new("wolf"),
                pos(2, 3, Vec3::new(64.0, 0.0, 128.0)),
                UnitSource::Authored,
            )
            .unwrap();

            assert_eq!(
                world.unit_chunk(record.id),
                Some(ChunkId::new(ChunkCoord::new(2, 3)))
            );
            world.assert_unit_index_consistent();
        }

        #[test]
        fn index_integrity_after_move_same_chunk() {
            let catalog = UnitCatalog::default();
            let mut world = WorldData::new(layout());
            let chunk = ChunkId::new(ChunkCoord::new(0, 0));
            let record = create_unit(
                &catalog,
                &mut world,
                &UnitDefinitionId::new("deer"),
                pos(0, 0, Vec3::new(10.0, 0.0, 20.0)),
                UnitSource::Authored,
            )
            .unwrap();
            world.assert_unit_index_consistent();

            move_unit(&mut world, record.id, pos(0, 0, Vec3::new(200.0, 0.0, 50.0)))
                .unwrap();

            assert_eq!(world.unit_chunk(record.id), Some(chunk));
            world.assert_unit_index_consistent();
        }

        #[test]
        fn index_integrity_after_move_cross_chunk() {
            let catalog = UnitCatalog::default();
            let mut world = WorldData::new(layout());
            let record = create_unit(
                &catalog,
                &mut world,
                &UnitDefinitionId::new("bandit"),
                pos(0, 0, Vec3::new(1.0, 0.0, 1.0)),
                UnitSource::Authored,
            )
            .unwrap();
            world.assert_unit_index_consistent();

            let new_chunk = ChunkId::new(ChunkCoord::new(1, 0));
            move_unit(
                &mut world,
                record.id,
                pos(1, 0, Vec3::new(128.0, 0.0, 128.0)),
            )
            .unwrap();

            assert_eq!(world.unit_chunk(record.id), Some(new_chunk));
            world.assert_unit_index_consistent();
        }

        #[test]
        fn index_integrity_after_remove_by_id() {
            let catalog = UnitCatalog::default();
            let mut world = WorldData::new(layout());
            let record = create_unit(
                &catalog,
                &mut world,
                &UnitDefinitionId::new("wolf"),
                pos(4, 5, Vec3::new(64.0, 0.0, 64.0)),
                UnitSource::Authored,
            )
            .unwrap();
            world.assert_unit_index_consistent();

            remove_unit(&mut world, record.id).unwrap();

            assert!(world.unit_chunk(record.id).is_none());
            world.assert_unit_index_consistent();
        }

        #[test]
        fn index_integrity_after_remove_by_chunk() {
            let catalog = UnitCatalog::default();
            let mut world = WorldData::new(layout());
            let chunk = ChunkId::new(ChunkCoord::new(3, 3));
            let record = create_unit(
                &catalog,
                &mut world,
                &UnitDefinitionId::new("deer"),
                pos(3, 3, Vec3::new(32.0, 0.0, 32.0)),
                UnitSource::Authored,
            )
            .unwrap();
            world.assert_unit_index_consistent();

            assert!(world.remove_unit(chunk, record.id));

            assert!(world.unit_chunk(record.id).is_none());
            world.assert_unit_index_consistent();
        }
    }
}
