use std::collections::HashSet;

use bevy::prelude::*;

use crate::world::relationship::SpeciesId;
use crate::world::{
    BuildingArchetypeId, CapturedUnitArchetypeTemplate, UnitArchetypeId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchetypeEditorMode {
    UnitCreate,
    UnitEdit,
    BuildingCreate,
    BuildingEdit,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct DevArchetypeEditorState {
    pub modal_open: bool,
    pub mode: Option<ArchetypeEditorMode>,
    pub editing_unit_id: Option<UnitArchetypeId>,
    pub editing_building_id: Option<BuildingArchetypeId>,
    pub name_input: String,
    pub gold_min_input: String,
    pub gold_max_input: String,
    pub selected_species: HashSet<SpeciesId>,
    pub captured_unit_template: Option<CapturedUnitArchetypeTemplate>,
    pub status_message: String,
    pub pending_template_update: bool,
}

impl DevArchetypeEditorState {
    pub fn close(&mut self) {
        self.modal_open = false;
        self.mode = None;
        self.editing_unit_id = None;
        self.editing_building_id = None;
        self.name_input.clear();
        self.gold_min_input = "0".to_string();
        self.gold_max_input = "0".to_string();
        self.selected_species.clear();
        self.captured_unit_template = None;
        self.pending_template_update = false;
        self.status_message.clear();
    }

    pub fn is_unit_modal(&self) -> bool {
        matches!(
            self.mode,
            Some(ArchetypeEditorMode::UnitCreate | ArchetypeEditorMode::UnitEdit)
        )
    }

    pub fn is_building_modal(&self) -> bool {
        matches!(
            self.mode,
            Some(ArchetypeEditorMode::BuildingCreate | ArchetypeEditorMode::BuildingEdit)
        )
    }
}
