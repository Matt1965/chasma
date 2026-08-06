//! Navigation Editor client-local UI state (Slice 7).

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::dev::inspector::BlueprintInspectionState;
use crate::dev::widgets::DevStatusSeverity;
use crate::dev::window::{DevWindowId, DevWindowRegistry};

/// Default building presentation opacity while the Navigation Editor is active.
pub const DEFAULT_NAV_EDITOR_BUILDING_OPACITY: f32 = 0.42;

/// Auto-dismiss duration for success toasts (seconds).
pub const NAV_EDITOR_SUCCESS_TOAST_SECS: f32 = 3.0;

/// Transient action feedback shown near the top of the Navigation Editor body.
#[derive(Debug, Clone, PartialEq)]
pub struct NavEditorToast {
    pub message: String,
    pub severity: DevStatusSeverity,
    pub shown_at_secs: f32,
    pub auto_dismiss: bool,
}

impl NavEditorToast {
    pub fn is_expired(&self, now_secs: f32) -> bool {
        self.auto_dismiss && now_secs - self.shown_at_secs >= NAV_EDITOR_SUCCESS_TOAST_SECS
    }
}

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

    /// Compact counts for the always-visible generation summary row.
    pub fn concise_counts_line(&self) -> String {
        format!(
            "Entrances: {}  Explicit: {}  Synthesized: {}  Deduplicated: {}",
            self.entrances_generated,
            self.explicit_markers,
            self.synthesized_entrances,
            self.deduplicated_candidates,
        )
    }
}

/// Wrap long single-line dev panel text to a maximum line width (character-based).
pub fn wrap_panel_text(text: &str, max_line_chars: usize) -> String {
    if max_line_chars == 0 || text.len() <= max_line_chars {
        return text.to_string();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
            continue;
        }
        if current.len() + 1 + word.len() > max_line_chars {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines.join("\n")
}

/// Always-visible generation summary (source + topology + validation error count).
pub fn format_concise_generation_summary(
    source_label: Option<&str>,
    region_count: usize,
    connection_count: usize,
    validation_error_count: usize,
) -> String {
    format!(
        "Source: {}  Regions: {}  Connections: {}  Validation errors: {}",
        source_label.unwrap_or("-"),
        region_count,
        connection_count,
        validation_error_count,
    )
}

/// Verbose marker/diagnostic lines for the collapsible generation-details section.
pub fn format_generation_details(
    source_label: Option<&str>,
    diagnostics: Option<&NavigationGenerationDiagnostics>,
) -> String {
    let mut lines = format!("Regeneration source: {}", source_label.unwrap_or("-"));
    if let Some(diag) = diagnostics {
        lines.push('\n');
        lines.push_str(&diag.concise_counts_line());
        if !diag.candidate_details.is_empty() {
            lines.push_str("\nMarkers:");
            for detail in &diag.candidate_details {
                lines.push('\n');
                lines.push_str("- ");
                lines.push_str(&wrap_panel_text(detail, 52));
            }
        }
    }
    lines
}

/// Client-local Navigation Editor presentation state (not scene-persisted).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct NavigationEditorUiState {
    pub pending_blocked_action: Option<NavigationEditorBlockedAction>,
    pub variant_display_name: String,
    pub variant_asset_id: String,
    pub variant_description: String,
    pub validation_expanded: bool,
    /// User-controlled expansion for generation details (persists across repaint).
    pub generation_details_expanded: bool,
    /// Active toast/banner feedback (replaces bottom status line).
    pub toast: Option<NavEditorToast>,
    /// Last inspector message mirrored into toast to detect changes.
    pub last_toast_source_message: String,
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
            generation_details_expanded: false,
            toast: None,
            last_toast_source_message: String::new(),
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
        self.generation_details_expanded = false;
        self.toast = None;
        self.last_toast_source_message.clear();
    }

    pub fn queue_toast(
        &mut self,
        message: impl Into<String>,
        severity: DevStatusSeverity,
        now_secs: f32,
    ) {
        let message = message.into();
        let auto_dismiss = severity == DevStatusSeverity::Success;
        self.toast = Some(NavEditorToast {
            message: message.clone(),
            severity,
            shown_at_secs: now_secs,
            auto_dismiss,
        });
        self.last_toast_source_message = message;
    }

    pub fn sync_toast_from_message(
        &mut self,
        message: &str,
        severity: DevStatusSeverity,
        now_secs: f32,
    ) {
        let trimmed = message.trim();
        if trimmed.is_empty() {
            return;
        }
        if trimmed == self.last_toast_source_message {
            return;
        }
        self.queue_toast(trimmed, severity, now_secs);
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
