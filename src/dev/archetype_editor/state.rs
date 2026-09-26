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
    pub dialogue_talk_enabled: bool,
    pub dialogue_talk_min: String,
    pub dialogue_trade_enabled: bool,
    pub dialogue_trade_min: String,
    pub dialogue_recruit_enabled: bool,
    pub dialogue_recruit_min: String,
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
        self.dialogue_talk_enabled = false;
        self.dialogue_talk_min = "0".to_string();
        self.dialogue_trade_enabled = false;
        self.dialogue_trade_min = "0".to_string();
        self.dialogue_recruit_enabled = false;
        self.dialogue_recruit_min = "0".to_string();
        self.pending_template_update = false;
        self.status_message.clear();
    }

    pub fn load_dialogue_from_definition(
        &mut self,
        dialogue: Option<&crate::world::UnitDialogueConfig>,
    ) {
        if let Some(config) = dialogue {
            self.dialogue_talk_enabled = config.talk.enabled;
            self.dialogue_talk_min = config.talk.min_relationship.to_string();
            self.dialogue_trade_enabled = config.trade.enabled;
            self.dialogue_trade_min = config.trade.min_relationship.to_string();
            self.dialogue_recruit_enabled = config.recruit.enabled;
            self.dialogue_recruit_min = config.recruit.min_relationship.to_string();
        } else {
            self.dialogue_talk_enabled = false;
            self.dialogue_talk_min = "0".to_string();
            self.dialogue_trade_enabled = false;
            self.dialogue_trade_min = "0".to_string();
            self.dialogue_recruit_enabled = false;
            self.dialogue_recruit_min = "0".to_string();
        }
    }

    pub fn build_dialogue_config(&self) -> Option<crate::world::UnitDialogueConfig> {
        if !self.dialogue_talk_enabled
            && !self.dialogue_trade_enabled
            && !self.dialogue_recruit_enabled
        {
            return None;
        }
        Some(crate::world::UnitDialogueConfig {
            talk: crate::world::DialogueOptionRule {
                enabled: self.dialogue_talk_enabled,
                min_relationship: parse_i32_or_zero(&self.dialogue_talk_min),
            },
            trade: crate::world::DialogueOptionRule {
                enabled: self.dialogue_trade_enabled,
                min_relationship: parse_i32_or_zero(&self.dialogue_trade_min),
            },
            recruit: crate::world::DialogueOptionRule {
                enabled: self.dialogue_recruit_enabled,
                min_relationship: parse_i32_or_zero(&self.dialogue_recruit_min),
            },
        })
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

fn parse_i32_or_zero(input: &str) -> i32 {
    input.trim().parse().unwrap_or(0)
}
