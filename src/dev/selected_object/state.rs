//! Selected Object window UI state (Slice 5) — client-local, non-authoritative.

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::dev::gizmo::SelectedWorldObject;
use crate::units::input::SelectedUnits;
use crate::world::{UnitId, WorldData};

/// Pending destructive action awaiting confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingDeleteTarget {
    Doodad(crate::world::DoodadId),
    Building(crate::world::BuildingId),
    ItemPile(crate::world::ItemPileId),
    Unit(UnitId),
}

/// Session-only Selected Object presentation state.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct SelectedObjectUiState {
    pub diagnostics_expanded: bool,
    pub navigation_section_expanded: bool,
    pub pending_delete: Option<PendingDeleteTarget>,
}

impl SelectedObjectUiState {
    pub fn clear_pending_delete(&mut self) {
        self.pending_delete = None;
    }

    pub fn toggle_diagnostics(&mut self) {
        self.diagnostics_expanded = !self.diagnostics_expanded;
    }

    pub fn toggle_navigation_section(&mut self) {
        self.navigation_section_expanded = !self.navigation_section_expanded;
    }

    pub fn request_delete(&mut self, target: SelectedWorldObject) {
        self.pending_delete = match target {
            SelectedWorldObject::Doodad(id) => Some(PendingDeleteTarget::Doodad(id)),
            SelectedWorldObject::Building(id) => Some(PendingDeleteTarget::Building(id)),
            SelectedWorldObject::ItemPile(id) => Some(PendingDeleteTarget::ItemPile(id)),
        };
    }

    pub fn request_delete_unit(&mut self, unit_id: UnitId) {
        self.pending_delete = Some(PendingDeleteTarget::Unit(unit_id));
    }

    /// Returns false when selection no longer matches the armed delete target.
    pub fn pending_delete_still_valid(
        pending: PendingDeleteTarget,
        world_selection: &WorldSelectionState,
        selected_units: &SelectedUnits,
        world: &WorldData,
    ) -> bool {
        match pending {
            PendingDeleteTarget::Doodad(id) => world_selection.transform_doodad() == Some(id),
            PendingDeleteTarget::Building(id) => world_selection.transform_building() == Some(id),
            PendingDeleteTarget::ItemPile(id) => world_selection.transform_item_pile() == Some(id),
            PendingDeleteTarget::Unit(unit_id) => {
                world_selection.category == WorldSelectionCategory::Units
                    && selected_units.0.len() == 1
                    && selected_units.0.contains(&unit_id)
                    && world.get_unit(unit_id).is_some()
            }
        }
    }
}
