//! Building production policy — player/AI intent separate from runtime state (EP2).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::operation_id::OperationDefinitionId;
use crate::world::building::catalog::BuildingDefinition;
use crate::world::operation::OperationCatalog;

/// Who controls production policy for a building (EP2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect, Serialize, Deserialize)]
pub enum ControlSource {
    #[default]
    #[serde(rename = "Player")]
    PlayerControlled,
    #[serde(rename = "Ai")]
    AIControlled,
}

impl ControlSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::PlayerControlled => "Player",
            Self::AIControlled => "AI",
        }
    }
}

/// How many production cycles to run before stopping (EP2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect, Serialize, Deserialize)]
pub enum RepeatMode {
    #[default]
    #[serde(rename = "Forever")]
    Continuous,
    Count(u32),
}

impl RepeatMode {
    pub fn display_label(self) -> String {
        match self {
            Self::Continuous => "Continuous".to_string(),
            Self::Count(n) => format!("RepeatCount({n})"),
        }
    }

    pub fn is_exhausted(self, completions: u32) -> bool {
        match self {
            Self::Continuous => false,
            Self::Count(target) => target == 0 || completions >= target,
        }
    }

    pub fn remaining_repeats(self, completions: u32) -> Option<u32> {
        match self {
            Self::Continuous => None,
            Self::Count(target) => Some(target.saturating_sub(completions)),
        }
    }

    pub fn is_valid(self) -> bool {
        match self {
            Self::Continuous => true,
            Self::Count(count) => count > 0,
        }
    }
}

/// Player/AI intent for one building's production (EP2).
///
/// Separate from [`super::store::BuildingOperationState`] simulation truth.
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct BuildingOperationPolicy {
    pub enabled: bool,
    pub paused: bool,
    pub selected_operation: Option<OperationDefinitionId>,
    pub repeat_mode: RepeatMode,
    pub priority: u8,
    pub control_source: ControlSource,
    /// When true, settlement planner may adjust this policy (EP9).
    #[serde(default)]
    pub planner_managed: bool,
}

impl Default for BuildingOperationPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            paused: false,
            selected_operation: None,
            repeat_mode: RepeatMode::Continuous,
            priority: 128,
            control_source: ControlSource::PlayerControlled,
            planner_managed: false,
        }
    }
}

impl BuildingOperationPolicy {
    pub fn default_for_building(
        definition: &BuildingDefinition,
        _operation_catalog: &OperationCatalog,
    ) -> Self {
        let selected_operation = definition.resolved_default_operation();
        Self {
            enabled: true,
            paused: false,
            selected_operation,
            repeat_mode: RepeatMode::Continuous,
            priority: 128,
            control_source: ControlSource::PlayerControlled,
            planner_managed: false,
        }
    }
}
