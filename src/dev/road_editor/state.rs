use bevy::prelude::*;

use crate::world::{RoadControlPoint, RoadId, RoadNetwork, RoadStyleId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoadEditMode {
    Inactive,
    Create,
    ExtendStart,
    ExtendEnd,
    InsertPoint,
}

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct RoadEditorUiState {
    pub mode: RoadEditMode,
    pub dirty: bool,
    pub status_message: String,
    pub create_points: Vec<RoadControlPoint>,
    pub create_display_name: String,
    pub create_style: RoadStyleId,
    pub selected_road_id: Option<RoadId>,
    pub selected_point_index: Option<usize>,
    pub dragging_point: Option<(RoadId, usize)>,
    pub name_input: String,
    pub saved_baseline: RoadNetwork,
    pub pending_delete_confirmation: bool,
}

impl Default for RoadEditorUiState {
    fn default() -> Self {
        Self {
            mode: RoadEditMode::Inactive,
            dirty: false,
            status_message: String::new(),
            create_points: Vec::new(),
            create_display_name: "New Road".into(),
            create_style: RoadStyleId::default_for_v1(),
            selected_road_id: None,
            selected_point_index: None,
            dragging_point: None,
            name_input: String::new(),
            saved_baseline: RoadNetwork::empty(),
            pending_delete_confirmation: false,
        }
    }
}

impl RoadEditorUiState {
    pub fn sync_baseline_from(&mut self, network: &RoadNetwork) {
        self.saved_baseline = network.clone();
        self.dirty = false;
    }

    pub fn mark_dirty(&mut self, message: impl Into<String>) {
        self.dirty = true;
        self.status_message = message.into();
    }

    pub fn clear_draft_modes(&mut self) {
        self.mode = RoadEditMode::Inactive;
        self.create_points.clear();
        self.dragging_point = None;
        self.pending_delete_confirmation = false;
    }

    pub fn begin_create(&mut self) {
        self.clear_draft_modes();
        self.mode = RoadEditMode::Create;
        self.create_display_name = "New Road".into();
        self.create_style = RoadStyleId::default_for_v1();
        self.selected_road_id = None;
        self.selected_point_index = None;
        self.status_message = "Create road — click terrain for control points".into();
    }

    pub fn cancel_active(&mut self) {
        self.clear_draft_modes();
        self.status_message = "Road editing cancelled".into();
    }

    pub fn owns_world_pointer(&self, window_visible: bool, panel_hovered: bool) -> bool {
        window_visible
            && !panel_hovered
            && (matches!(
                self.mode,
                RoadEditMode::Create | RoadEditMode::ExtendStart | RoadEditMode::ExtendEnd
            ) || self.dragging_point.is_some()
                || self.mode == RoadEditMode::InsertPoint
                || self.selected_road_id.is_some())
    }

    pub fn window_title_suffix(&self) -> &'static str {
        if self.dirty { " *" } else { "" }
    }
}

pub fn road_editor_owns_world_pointer(
    dev_enabled: bool,
    window_visible: bool,
    panel_hovered: bool,
    editor: &RoadEditorUiState,
) -> bool {
    dev_enabled && editor.owns_world_pointer(window_visible, panel_hovered)
}
