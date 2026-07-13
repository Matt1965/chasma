//! Chunk-keyed derived occupancy storage (ADR-080 B3).

use bevy::prelude::*;
use std::collections::BTreeMap;

use super::OccupancySource;
use super::cell::{OccupancyCellCoord, SURFACE_SPACE_ID};

/// Occupancy state for a registered cell (B3/B4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum OccupancyState {
    /// Blocks unit movement and placement.
    Blocked,
    /// Reserves footprint for planned construction — blocks placement, not movement (B4).
    Reserved,
}

/// One registered occupancy cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct OccupancyCellEntry {
    pub state: OccupancyState,
    pub source: OccupancySource,
    pub space_id: u32,
}

/// Per-chunk occupancy grid (derived, rebuildable).
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct ChunkOccupancyGrid {
    cells: BTreeMap<(OccupancyCellCoord, u32), OccupancyCellEntry>,
}

impl ChunkOccupancyGrid {
    pub fn get(&self, cell: OccupancyCellCoord, space_id: u32) -> Option<&OccupancyCellEntry> {
        self.cells.get(&(cell, space_id))
    }

    pub fn insert(
        &mut self,
        cell: OccupancyCellCoord,
        space_id: u32,
        entry: OccupancyCellEntry,
    ) -> Option<OccupancyCellEntry> {
        self.cells.insert((cell, space_id), entry)
    }

    pub fn remove(
        &mut self,
        cell: OccupancyCellCoord,
        space_id: u32,
    ) -> Option<OccupancyCellEntry> {
        self.cells.remove(&(cell, space_id))
    }

    pub fn remove_source(&mut self, source: OccupancySource) -> usize {
        let before = self.cells.len();
        self.cells.retain(|_, entry| entry.source != source);
        before - self.cells.len()
    }

    pub fn cells(&self) -> impl Iterator<Item = (&OccupancyCellCoord, &OccupancyCellEntry)> {
        self.cells.iter().map(|((cell, _), entry)| (cell, entry))
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

/// Default space id for B3 surface occupancy.
pub fn default_space_id() -> u32 {
    SURFACE_SPACE_ID
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::DoodadId;

    #[test]
    fn remove_source_clears_cells() {
        let mut grid = ChunkOccupancyGrid::default();
        let source = OccupancySource::Doodad(DoodadId::new(1));
        grid.insert(
            OccupancyCellCoord::new(0, 0),
            SURFACE_SPACE_ID,
            OccupancyCellEntry {
                state: OccupancyState::Blocked,
                source,
                space_id: SURFACE_SPACE_ID,
            },
        );
        assert_eq!(grid.remove_source(source), 1);
        assert!(grid.is_empty());
    }
}
