//! Client-local terrain field overlay state (ADR-103 TF3).

use bevy::prelude::*;

use crate::world::TerrainFieldId;

/// Maximum player-selected overlay opacity (90%).
pub const MAX_PLAYER_OVERLAY_OPACITY_BP: u16 = 9_000;

/// Default overlay opacity when no field is selected (55%).
pub const DEFAULT_OVERLAY_OPACITY_BP: u16 = 5_500;

/// Layered overlay selection seam for TF4 Build Mode overrides.
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct TerrainOverlaySelection {
    pub manual: Option<TerrainFieldId>,
    /// Reserved for TF4 temporary Build Mode overlay override.
    pub temporary_override: Option<TerrainFieldId>,
}

impl TerrainOverlaySelection {
    pub fn effective_field(&self) -> Option<&TerrainFieldId> {
        self.temporary_override.as_ref().or(self.manual.as_ref())
    }
}

/// Player-facing Terrain Analysis overlay state (not authoritative).
#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
pub struct TerrainOverlayState {
    pub selection: TerrainOverlaySelection,
    /// Player opacity in basis points (0–10000).
    pub opacity_basis_points: u16,
    /// When true, field switches preserve the current opacity instead of defaults.
    pub opacity_user_override: bool,
    pub show_cursor_value: bool,
    pub panel_open: bool,
    /// Incremented on field switch to ignore stale GPU upload completions.
    pub request_revision: u64,
    pub stale_completions_ignored: u64,
}

impl Default for TerrainOverlayState {
    fn default() -> Self {
        Self {
            selection: TerrainOverlaySelection::default(),
            opacity_basis_points: DEFAULT_OVERLAY_OPACITY_BP,
            opacity_user_override: false,
            show_cursor_value: true,
            panel_open: false,
            request_revision: 0,
            stale_completions_ignored: 0,
        }
    }
}

impl TerrainOverlayState {
    pub fn effective_field(&self) -> Option<&TerrainFieldId> {
        self.selection.effective_field()
    }

    pub fn set_manual_field(&mut self, field: Option<TerrainFieldId>) {
        if self.selection.manual != field {
            self.selection.manual = field;
            self.request_revision = self.request_revision.saturating_add(1);
        }
    }

    /// TF4 Build Mode temporary overlay — does not overwrite manual selection.
    pub fn set_temporary_override(&mut self, field: Option<TerrainFieldId>) {
        if self.selection.temporary_override != field {
            self.selection.temporary_override = field;
            self.request_revision = self.request_revision.saturating_add(1);
        }
    }

    pub fn clear_temporary_override(&mut self) {
        self.set_temporary_override(None);
    }

    pub fn set_opacity_basis_points(&mut self, opacity_bp: u16) {
        self.opacity_basis_points = opacity_bp.min(MAX_PLAYER_OVERLAY_OPACITY_BP);
        self.opacity_user_override = true;
    }

    pub fn clamp_opacity(opacity_bp: u16) -> u16 {
        opacity_bp.min(MAX_PLAYER_OVERLAY_OPACITY_BP)
    }
}
