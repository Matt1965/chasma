//! Navigation Editor client-local UI state (Slice 7).

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::dev::inspector::BlueprintInspectionState;
use crate::dev::window::{DevWindowId, DevWindowRegistry};

/// Default building presentation opacity while the Navigation Editor is active.
pub const DEFAULT_NAV_EDITOR_BUILDING_OPACITY: f32 = 0.42;

/// Pending user intent blocked by unsaved blueprint edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationEditorBlockedAction {
    CloseWindow,
    DisableDevMode,
    DisableAdvancedMode,
    ChangeSelection,
}

/// Client-local diagnostics from the last Regenerate (not blueprint-persisted).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NavigationGenerationDiagnostics {
    pub entrances_generated: usize,
    pub explicit_markers: usize,
    pub synthesized_entrances: usize,
    pub deduplicated_candidates: usize,
    pub regeneration_source: String,
    /// Compact candidate lines for tooltip/detail (`name @ [x,z] (source)`).
    pub candidate_details: Vec<String>,
}

impl NavigationGenerationDiagnostics {
    pub fn summary_line(&self) -> String {
        if self.candidate_details.is_empty() {
            return "No entrance candidates".into();
        }
        self.candidate_details.join(" | ")
    }
}

/// Client-local Navigation Editor presentation state (not scene-persisted).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct NavigationEditorUiState {
    pub pending_blocked_action: Option<NavigationEditorBlockedAction>,
    pub variant_display_name: String,
    pub variant_asset_id: String,
    pub variant_description: String,
    pub validation_expanded: bool,
    /// Editor-only building mesh opacity (0 = invisible, 1 = opaque).
    pub building_opacity: f32,
    /// Last mesh-slicing source label shown after Regenerate (session-local).
    pub regeneration_source_label: Option<String>,
    /// Last regenerate entrance diagnostics (session-local; not saved).
    pub generation_diagnostics: Option<NavigationGenerationDiagnostics>,
}

impl Default for NavigationEditorUiState {
    fn default() -> Self {
        Self {
            pending_blocked_action: None,
            variant_display_name: String::new(),
            variant_asset_id: String::new(),
            variant_description: String::new(),
            validation_expanded: false,
            building_opacity: DEFAULT_NAV_EDITOR_BUILDING_OPACITY,
            regeneration_source_label: None,
            generation_diagnostics: None,
        }
    }
}

impl NavigationEditorUiState {
    pub fn clear_blocked(&mut self) {
        self.pending_blocked_action = None;
    }

    pub fn reset_session_presentation(&mut self) {
        self.building_opacity = DEFAULT_NAV_EDITOR_BUILDING_OPACITY;
        self.regeneration_source_label = None;
        self.generation_diagnostics = None;
    }
}

/// Navigation Editor window is visible and owns an active inspection session.
pub fn navigation_editor_owns_session(
    dev_enabled: bool,
    registry: &DevWindowRegistry,
    inspection: &BlueprintInspectionState,
) -> bool {
    dev_enabled && registry.is_visible(DevWindowId::NavigationEditor) && inspection.active
}

/// Snapshot of selection used to revert blocked selection changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardedSelectionSnapshot {
    pub category: WorldSelectionCategory,
    pub building_id: Option<crate::world::BuildingId>,
    pub doodad_id: Option<crate::world::DoodadId>,
    pub pile_id: Option<crate::world::ItemPileId>,
}

impl GuardedSelectionSnapshot {
    pub fn capture(selection: &WorldSelectionState) -> Self {
        Self {
            category: selection.category,
            building_id: selection.building_id,
            doodad_id: selection.doodad_id,
            pile_id: selection.pile_id,
        }
    }
}
