//! Authored Strategic Task Templates (SA6).
//!
//! Maps ResponseType / ResponseId → task kinds. Never hardcodes Need → Building.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::settlement::response::{ResponseId, ResponseType};
use crate::world::task::TaskType;

/// Stable template identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct StrategicTaskTemplateId(pub String);

impl StrategicTaskTemplateId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Authored mapping from response → strategic task emission.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct StrategicTaskTemplate {
    pub id: StrategicTaskTemplateId,
    pub display_name: String,
    /// When set, only this ResponseId matches. Otherwise match `response_type`.
    pub response_id: Option<ResponseId>,
    pub response_type: ResponseType,
    pub task_type: TaskType,
    /// Prefer attaching to incomplete constructible buildings when true.
    pub prefer_construction_sites: bool,
    pub enabled: bool,
}

impl StrategicTaskTemplate {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        response_type: ResponseType,
        task_type: TaskType,
    ) -> Self {
        Self {
            id: StrategicTaskTemplateId::new(id),
            display_name: display_name.into(),
            response_id: None,
            response_type,
            task_type,
            prefer_construction_sites: matches!(response_type, ResponseType::ConstructBuilding),
            enabled: true,
        }
    }

    pub fn with_response_id(mut self, response_id: impl Into<String>) -> Self {
        self.response_id = Some(ResponseId::new(response_id));
        self
    }

    pub fn matches_response(&self, response_id: &ResponseId, response_type: ResponseType) -> bool {
        if !self.enabled {
            return false;
        }
        if let Some(required) = &self.response_id {
            return required == response_id;
        }
        self.response_type == response_type
    }
}

pub fn starter_strategic_task_templates() -> Vec<StrategicTaskTemplate> {
    vec![
        StrategicTaskTemplate::new(
            "construct_from_intent",
            "Construct From Intent",
            ResponseType::ConstructBuilding,
            TaskType::StrategicConstruct,
        ),
        // Food construction path — catalog maps response id, not NeedId.
        StrategicTaskTemplate::new(
            "construct_food_building",
            "Construct Food Building",
            ResponseType::ConstructBuilding,
            TaskType::StrategicConstruct,
        )
        .with_response_id("construct_food_building"),
        StrategicTaskTemplate::new(
            "repair_from_intent",
            "Repair From Intent",
            ResponseType::RepairBuilding,
            TaskType::RepairBuilding,
        ),
        StrategicTaskTemplate::new(
            "recruit_from_intent",
            "Recruit From Intent",
            ResponseType::Recruit,
            TaskType::RecruitWorker,
        ),
        StrategicTaskTemplate::new(
            "expand_storage_from_intent",
            "Expand Storage From Intent",
            ResponseType::Expand,
            TaskType::ExpandStorage,
        ),
        // Clear rubble — future Defend-adjacent seam (authored, not hardcoded Need).
        StrategicTaskTemplate::new(
            "clear_rubble_defend",
            "Clear Rubble",
            ResponseType::Defend,
            TaskType::ClearRubble,
        ),
    ]
}
