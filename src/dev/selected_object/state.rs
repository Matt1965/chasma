//! Selected Object window UI state (Slice 5) — client-local, non-authoritative.

use bevy::prelude::*;

use crate::dev::gizmo::SelectedWorldObject;

/// Pending destructive action awaiting confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingDeleteTarget {
    Doodad(crate::world::DoodadId),
    Building(crate::world::BuildingId),
    ItemPile(crate::world::ItemPileId),
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
}
