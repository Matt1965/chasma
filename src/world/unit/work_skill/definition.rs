//! Authored work skill catalog entries.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::id::WorkSkillId;

/// One work skill type (Farming, Construction, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct WorkSkillDefinition {
    pub id: WorkSkillId,
    pub display_name: String,
    /// Deterministic presentation order in UI and future workforce matrix.
    pub sort_order: u32,
    pub enabled: bool,
}

impl WorkSkillDefinition {
    pub fn new(id: impl Into<String>, display_name: impl Into<String>, sort_order: u32) -> Self {
        Self {
            id: WorkSkillId::new(id),
            display_name: display_name.into(),
            sort_order,
            enabled: true,
        }
    }
}
