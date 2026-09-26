//! Client-only dialogue session state.

use bevy::prelude::*;

use crate::world::{DialogueAction, DialogueActionKind, DialogueContent, UnitId};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DialogueView {
    #[default]
    Options,
    TalkResponse(DialogueContent),
    TradePlaceholder,
    RecruitPlaceholder,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct DialogueSessionState {
    pub open: bool,
    pub actor_unit_id: Option<UnitId>,
    pub target_unit_id: Option<UnitId>,
    pub view: DialogueView,
    pub feedback: String,
    pub last_action: Option<DialogueAction>,
}

impl DialogueSessionState {
    pub fn open_session(&mut self, actor_unit_id: UnitId, target_unit_id: UnitId) {
        self.open = true;
        self.actor_unit_id = Some(actor_unit_id);
        self.target_unit_id = Some(target_unit_id);
        self.view = DialogueView::Options;
        self.feedback.clear();
        self.last_action = None;
    }

    pub fn close(&mut self) {
        *self = Self::default();
    }

    pub fn show_talk_response(&mut self, content: DialogueContent) {
        self.view = DialogueView::TalkResponse(content);
        self.feedback.clear();
    }

    pub fn show_trade_placeholder(&mut self) {
        self.view = DialogueView::TradePlaceholder;
        self.feedback = "Trading is not implemented yet.".to_string();
    }

    pub fn show_recruit_placeholder(&mut self) {
        self.view = DialogueView::RecruitPlaceholder;
        self.feedback = "Recruitment is not implemented yet.".to_string();
    }

    pub fn back_to_options(&mut self) {
        self.view = DialogueView::Options;
        self.feedback.clear();
    }

    pub fn select_option(&mut self, kind: DialogueActionKind, actor: UnitId, target: UnitId) {
        match kind {
            DialogueActionKind::Talk => {
                self.last_action = Some(DialogueAction::Talk {
                    actor_unit_id: actor,
                    target_unit_id: target,
                });
                self.show_talk_response(DialogueContent::default_greeting());
            }
            DialogueActionKind::Trade => {
                self.last_action = Some(DialogueAction::Trade {
                    actor_unit_id: actor,
                    target_unit_id: target,
                });
                self.show_trade_placeholder();
            }
            DialogueActionKind::Recruit => {
                self.last_action = Some(DialogueAction::Recruit {
                    actor_unit_id: actor,
                    target_unit_id: target,
                });
                self.show_recruit_placeholder();
            }
        }
    }
}
