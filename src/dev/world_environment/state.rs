//! World environment UI state (Slice 11).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::dev::widgets::NumericDraft;
use crate::environment::EnvironmentValidationError;

use super::fields::EnvFieldId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldEnvironmentConfirm {
    Revert,
    ResetBuiltIn,
    SaveProjectDefaults,
}

/// Dev-only authoring UI state for the World window.
#[derive(Resource, Debug, Default)]
pub struct WorldEnvironmentUiState {
    pub numeric_drafts: HashMap<EnvFieldId, NumericDraft>,
    pub focused_field: Option<EnvFieldId>,
    pub pending_confirmation: Option<WorldEnvironmentConfirm>,
    pub status_message: String,
    pub status_ttl_frames: u32,
    pub error_message: String,
    pub validation_error: Option<EnvironmentValidationError>,
    pub selected_skybox_index: usize,
}

impl WorldEnvironmentUiState {
    pub fn draft_mut(&mut self, field: EnvFieldId) -> &mut NumericDraft {
        self.numeric_drafts.entry(field).or_default()
    }

    pub fn clear_focus(&mut self) {
        self.focused_field = None;
    }

    pub fn set_success(&mut self, message: impl Into<String>) {
        self.status_message = message.into();
        self.status_ttl_frames = 180;
        self.error_message.clear();
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error_message = message.into();
        self.status_message.clear();
        self.status_ttl_frames = 0;
    }

    pub fn tick_status(&mut self) {
        if self.status_ttl_frames > 0 {
            self.status_ttl_frames -= 1;
            if self.status_ttl_frames == 0 {
                self.status_message.clear();
            }
        }
    }

    pub fn clear_numeric_drafts(&mut self) {
        self.numeric_drafts.clear();
        self.focused_field = None;
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DevWorldEnvironmentStatusText;

#[derive(Component, Debug, Clone, Copy)]
pub struct DevWorldEnvironmentDirtyBadge;

#[derive(Component, Debug, Clone, Copy)]
pub struct DevWorldEnvironmentLoadStatusText;

#[derive(Component, Debug, Clone, Copy)]
pub struct DevWorldEnvironmentValidationText;

#[derive(Component, Debug, Clone, Copy)]
pub enum DevWorldEnvironmentAction {
    SaveProjectDefaults,
    Revert,
    ResetBuiltIn,
    Confirm,
    CancelConfirm,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DevWorldCycleToggle;

#[derive(Component, Debug, Clone, Copy)]
pub struct DevWorldPauseToggle;

#[derive(Component, Debug, Clone, Copy)]
pub struct DevWorldTimePresetButton {
    pub preset: WorldTimePreset,
}

#[derive(Debug, Clone, Copy)]
pub enum WorldTimePreset {
    Dawn,
    Noon,
    Midnight,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DevWorldSkyboxOption {
    pub index: usize,
}

#[derive(Component, Debug)]
pub struct DevWorldEnvironmentSection;

#[derive(Component, Debug, Clone, Copy)]
pub struct DevWorldEnvironmentConfirmationBar;
