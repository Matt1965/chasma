//! Authoritative dialogue / social option configuration.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// One authored social option gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct DialogueOptionRule {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub min_relationship: i32,
}

impl Default for DialogueOptionRule {
    fn default() -> Self {
        Self {
            enabled: false,
            min_relationship: 0,
        }
    }
}

/// Per-unit social interaction capabilities baked from archetype spawn.
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct UnitDialogueConfig {
    #[serde(default)]
    pub talk: DialogueOptionRule,
    #[serde(default)]
    pub trade: DialogueOptionRule,
    #[serde(default)]
    pub recruit: DialogueOptionRule,
}

impl UnitDialogueConfig {
    pub fn rule(&self, kind: DialogueActionKind) -> &DialogueOptionRule {
        match kind {
            DialogueActionKind::Talk => &self.talk,
            DialogueActionKind::Trade => &self.trade,
            DialogueActionKind::Recruit => &self.recruit,
        }
    }

    pub fn has_any_enabled(&self) -> bool {
        self.talk.enabled || self.trade.enabled || self.recruit.enabled
    }
}

/// Social action kinds exposed by the dialogue UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum DialogueActionKind {
    Talk,
    Trade,
    Recruit,
}

impl DialogueActionKind {
    pub const ALL: [Self; 3] = [Self::Talk, Self::Trade, Self::Recruit];

    pub fn label(self) -> &'static str {
        match self {
            Self::Talk => "Talk",
            Self::Trade => "Trade",
            Self::Recruit => "Recruit",
        }
    }
}
