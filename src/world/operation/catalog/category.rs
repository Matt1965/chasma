//! Operation category taxonomy (EP3).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Strongly typed operation category for UI, filtering, validation, and future AI (EP3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect, Serialize, Deserialize)]
pub enum OperationCategory {
    #[default]
    Extraction,
    Processing,
    Crafting,
    Agriculture,
    Research,
    Medical,
    Ritual,
}

impl OperationCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Extraction => "Extraction",
            Self::Processing => "Processing",
            Self::Crafting => "Crafting",
            Self::Agriculture => "Agriculture",
            Self::Research => "Research",
            Self::Medical => "Medical",
            Self::Ritual => "Ritual",
        }
    }
}
