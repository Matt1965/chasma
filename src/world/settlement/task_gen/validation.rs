//! Strategic task generation validation (SA6).

use std::collections::BTreeSet;

use super::catalog::StrategicTaskTemplateCatalog;
use super::report::StrategicTaskGenerationReport;
use crate::world::task::{TaskPriority, TaskType};
use crate::world::WorldData;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrategicTaskValidationError {
    DuplicateEmission {
        template_id: String,
        building_id: u64,
    },
    UnknownTemplate(String),
    InvalidPriority,
    BrokenMapping {
        response_id: String,
        detail: String,
    },
}

impl std::fmt::Display for StrategicTaskValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateEmission {
                template_id,
                building_id,
            } => write!(
                f,
                "duplicate emission template=`{template_id}` building=#{building_id}"
            ),
            Self::UnknownTemplate(id) => write!(f, "unknown template `{id}`"),
            Self::InvalidPriority => write!(f, "invalid task priority"),
            Self::BrokenMapping { response_id, detail } => {
                write!(f, "broken mapping for `{response_id}`: {detail}")
            }
        }
    }
}

pub fn validate_strategic_task_report(
    world: &WorldData,
    catalog: &StrategicTaskTemplateCatalog,
    report: &StrategicTaskGenerationReport,
) -> Vec<StrategicTaskValidationError> {
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    for emission in &report.emissions {
        let key = (
            emission.template_id.clone(),
            emission.building_id.raw(),
            emission.task_type.label().to_string(),
        );
        if !seen.insert(key.clone()) {
            errors.push(StrategicTaskValidationError::DuplicateEmission {
                template_id: key.0,
                building_id: key.1,
            });
        }
        if catalog
            .get(&super::template::StrategicTaskTemplateId::new(
                emission.template_id.clone(),
            ))
            .is_none()
        {
            errors.push(StrategicTaskValidationError::UnknownTemplate(
                emission.template_id.clone(),
            ));
        }
        if world.task_store().get(emission.task_id).is_none() {
            errors.push(StrategicTaskValidationError::BrokenMapping {
                response_id: emission.response_id.clone(),
                detail: format!("missing task #{}", emission.task_id.raw()),
            });
        }
        if emission.priority == TaskPriority::PlayerAssigned {
            errors.push(StrategicTaskValidationError::InvalidPriority);
        }
        // Production/haul must never be emitted by strategic gen.
        if matches!(
            emission.task_type,
            TaskType::OperateWorkstation | TaskType::Haul
        ) {
            errors.push(StrategicTaskValidationError::BrokenMapping {
                response_id: emission.response_id.clone(),
                detail: "strategic gen must not emit production/haul tasks".into(),
            });
        }
    }
    errors
}
