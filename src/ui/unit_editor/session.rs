//! Client-local Unit Editor session state (CG3).

use bevy::prelude::*;

use crate::world::UnitId;

use super::draft::UnitAppearanceDraft;

/// How the editor was opened. CG8 will add `NewGameDraft`.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub enum UnitEditorMode {
    LiveUnit(UnitId),
}

/// Active editor session — presentation/UI only; not serialized into [`WorldData`].
#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
pub struct UnitEditorSession {
    pub mode: UnitEditorMode,
    pub draft: UnitAppearanceDraft,
    pub initial_draft: UnitAppearanceDraft,
    pub dirty: bool,
    pub preview_yaw_radians: f32,
    pub preview_zoom: f32,
    pub error_message: Option<String>,
}

impl UnitEditorSession {
    pub fn new(mode: UnitEditorMode, draft: UnitAppearanceDraft) -> Self {
        let initial_draft = draft.clone();
        Self {
            mode,
            draft,
            initial_draft,
            dirty: false,
            preview_yaw_radians: 0.0,
            preview_zoom: 1.0,
            error_message: None,
        }
    }

    pub fn recompute_dirty(&mut self) {
        self.dirty = self.draft != self.initial_draft;
    }

    pub fn live_unit_id(&self) -> Option<UnitId> {
        match self.mode {
            UnitEditorMode::LiveUnit(id) => Some(id),
        }
    }
}
