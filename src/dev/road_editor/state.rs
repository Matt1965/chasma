use bevy::prelude::*;



use crate::world::{Road, RoadControlPoint, RoadId, RoadNetwork, RoadStyleId, SnapCandidate};



use super::transaction::{

    RoadToolTransaction, RoadToolTransactionKind, is_modal_road_tool, rollback_transaction,

};



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

    pub snap_preview: Option<SnapCandidate>,

    pub transaction: Option<RoadToolTransaction>,

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

            snap_preview: None,

            transaction: None,

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



    pub fn has_modal_tool_active(&self) -> bool {

        is_modal_road_tool(self.mode)

    }



    pub fn clear_tool_state(&mut self) {

        self.mode = RoadEditMode::Inactive;

        self.create_points.clear();

        self.dragging_point = None;

        self.pending_delete_confirmation = false;

        self.snap_preview = None;

        self.transaction = None;

    }



    pub fn cancel_active(&mut self, network: &mut RoadNetwork) {

        if let Some(transaction) = self.transaction.take() {

            rollback_transaction(network, &transaction);

            self.dirty = transaction.dirty_before;

        }

        self.create_points.clear();

        self.clear_tool_state();

        self.status_message = "Road editing cancelled".into();

    }



    pub fn begin_create(&mut self, network: &mut RoadNetwork) {

        self.cancel_active(network);

        self.transaction = Some(RoadToolTransaction {

            dirty_before: self.dirty,

            kind: RoadToolTransactionKind::Create,

        });

        self.mode = RoadEditMode::Create;

        self.create_display_name = "New Road".into();

        self.create_style = RoadStyleId::default_for_v1();

        self.selected_road_id = None;

        self.selected_point_index = None;

        self.status_message = "Create road — click terrain for control points".into();

    }



    pub fn begin_extend(

        &mut self,

        network: &mut RoadNetwork,

        from_start: bool,

    ) -> Result<(), String> {

        let road_id = self

            .selected_road_id

            .clone()

            .ok_or_else(|| "Select a road before extending".to_string())?;

        let original_road = network

            .roads

            .get(&road_id)

            .cloned()

            .ok_or_else(|| format!("road {road_id} not found"))?;

        self.cancel_active(network);

        self.transaction = Some(RoadToolTransaction {

            dirty_before: self.dirty,

            kind: RoadToolTransactionKind::Extend {

                road_id: road_id.clone(),

                original_road,

            },

        });

        self.mode = if from_start {

            RoadEditMode::ExtendStart

        } else {

            RoadEditMode::ExtendEnd

        };

        self.status_message = "Click terrain to add extension points, then Finish".into();

        Ok(())

    }



    pub fn begin_insert_point(&mut self, network: &mut RoadNetwork) -> Result<(), String> {

        if self.selected_road_id.is_none() {

            return Err("Select a road before inserting a point".into());

        }

        self.cancel_active(network);

        self.transaction = Some(RoadToolTransaction {

            dirty_before: self.dirty,

            kind: RoadToolTransactionKind::InsertPoint,

        });

        self.mode = RoadEditMode::InsertPoint;

        self.status_message = "Click a road segment to insert a point".into();

        Ok(())

    }



    pub fn take_extend_transaction(&mut self) -> Option<(RoadId, Road, bool)> {

        let transaction = self.transaction.take()?;

        match transaction.kind {

            RoadToolTransactionKind::Extend {

                road_id,

                original_road,

            } => Some((road_id, original_road, transaction.dirty_before)),

            _ => {

                self.transaction = Some(transaction);

                None

            }

        }

    }



    pub fn take_create_transaction(&mut self) -> Option<bool> {

        let transaction = self.transaction.take()?;

        match transaction.kind {

            RoadToolTransactionKind::Create => Some(transaction.dirty_before),

            _ => {

                self.transaction = Some(transaction);

                None

            }

        }

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



    pub fn road_button_active(&self, button: super::actions::RoadEditorButton) -> bool {

        match button {

            super::actions::RoadEditorButton::Create => self.mode == RoadEditMode::Create,

            super::actions::RoadEditorButton::ExtendStart => {

                self.mode == RoadEditMode::ExtendStart

            }

            super::actions::RoadEditorButton::ExtendEnd => self.mode == RoadEditMode::ExtendEnd,

            super::actions::RoadEditorButton::InsertPoint => self.mode == RoadEditMode::InsertPoint,

            _ => false,

        }

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


