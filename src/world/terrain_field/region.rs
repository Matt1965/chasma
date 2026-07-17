//! Deterministic area sampling region (ADR-101).

use crate::world::occupancy::OccupancyCellCoord;

/// Ordered occupancy cells used for terrain field area sampling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSampleRegion {
    cells: Vec<OccupancyCellCoord>,
}

impl FieldSampleRegion {
    pub fn from_cells(mut cells: Vec<OccupancyCellCoord>) -> Self {
        cells.sort_by_key(|cell| (cell.z, cell.x));
        Self { cells }
    }

    pub fn cells(&self) -> &[OccupancyCellCoord] {
        &self.cells
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }
}
