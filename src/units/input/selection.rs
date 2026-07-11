//! Client-local multi-unit selection state (ADR-034 U9).

use std::collections::HashSet;

use bevy::prelude::*;

use crate::world::{UnitId, WorldData};

/// Runtime-only set of locally selected units (SC2-style, U9).
///
/// Not written to [`WorldData`]. Ordering is undefined; duplicates are forbidden.
#[derive(Debug, Resource, Default, Clone, PartialEq, Eq)]
pub struct SelectedUnits(pub HashSet<UnitId>);

impl SelectedUnits {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains(&self, unit_id: UnitId) -> bool {
        self.0.contains(&unit_id)
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Replace the entire selection with a single unit.
    pub fn set_single(&mut self, unit_id: UnitId) {
        self.0.clear();
        self.0.insert(unit_id);
    }

    /// SC2 shift-click: add if absent, remove if present.
    pub fn toggle(&mut self, unit_id: UnitId) {
        if !self.0.insert(unit_id) {
            self.0.remove(&unit_id);
        }
    }

    /// Replace selection with the given set (deduplicated).
    pub fn replace_with(&mut self, unit_ids: impl IntoIterator<Item = UnitId>) {
        self.0.clear();
        self.0.extend(unit_ids);
    }

    /// Union units into the current selection.
    pub fn add_all(&mut self, unit_ids: impl IntoIterator<Item = UnitId>) {
        self.0.extend(unit_ids);
    }

    /// Drop unit ids that no longer exist in authoritative world data.
    pub fn prune_missing(&mut self, world: &WorldData) {
        self.0.retain(|id| world.get_unit(*id).is_some());
    }

    /// Drop dead or removed units from the selection set (ADR-059 C6).
    pub fn prune_dead(&mut self, world: &WorldData) {
        use crate::world::is_unit_alive;
        self.0
            .retain(|id| world.get_unit(*id).map(is_unit_alive).unwrap_or(false));
    }

    pub fn iter(&self) -> impl Iterator<Item = UnitId> + '_ {
        self.0.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, UnitCatalog,
        UnitDefinitionId, UnitSource, WorldData, WorldPosition, create_unit,
    };

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn spawn_unit(world: &mut WorldData, catalog: &UnitCatalog, x: f32, z: f32) -> UnitId {
        create_unit(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            pos(x, z),
            UnitSource::Authored,
        )
        .unwrap()
        .id
    }

    #[test]
    fn selected_units_starts_empty() {
        let selection = SelectedUnits::default();
        assert!(selection.is_empty());
    }

    #[test]
    fn click_selects_single_unit() {
        let mut selection = SelectedUnits::default();
        let a = UnitId::new(1);
        let b = UnitId::new(2);
        selection.set_single(a);
        assert!(selection.contains(a));
        selection.set_single(b);
        assert!(!selection.contains(a));
        assert!(selection.contains(b));
    }

    #[test]
    fn shift_click_toggles_selection() {
        let mut selection = SelectedUnits::default();
        let a = UnitId::new(1);
        selection.toggle(a);
        assert!(selection.contains(a));
        selection.toggle(a);
        assert!(!selection.contains(a));
    }

    #[test]
    fn clearing_selection_works() {
        let mut selection = SelectedUnits::default();
        selection.set_single(UnitId::new(1));
        selection.clear();
        assert!(selection.is_empty());
    }

    #[test]
    fn invalid_unit_ids_removed_from_selection() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let unit_id = spawn_unit(&mut world, &catalog, 4.0, 4.0);
        let mut selection = SelectedUnits::default();
        selection.set_single(unit_id);
        selection.0.insert(UnitId::new(9999));
        selection.prune_missing(&world);
        assert_eq!(selection.0.len(), 1);
        assert!(selection.contains(unit_id));
    }

    #[test]
    fn box_select_replaces_selection_set() {
        let mut selection = SelectedUnits::default();
        let ids = [UnitId::new(1), UnitId::new(2), UnitId::new(3)];
        selection.replace_with(ids);
        assert_eq!(selection.0.len(), 3);
        for id in ids {
            assert!(selection.contains(id));
        }
    }
}
