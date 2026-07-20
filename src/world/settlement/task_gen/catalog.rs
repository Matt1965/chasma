//! Strategic Task Template catalog (SA6).

use std::collections::HashMap;

use bevy::prelude::*;

use super::template::{
    starter_strategic_task_templates, StrategicTaskTemplate, StrategicTaskTemplateId,
};
use crate::world::settlement::response::{ResponseId, ResponseType};

#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct StrategicTaskTemplateCatalog {
    templates: Vec<StrategicTaskTemplate>,
    by_id: HashMap<StrategicTaskTemplateId, usize>,
}

impl Default for StrategicTaskTemplateCatalog {
    fn default() -> Self {
        Self::from_templates(starter_strategic_task_templates())
            .expect("strategic task templates are valid")
    }
}

impl StrategicTaskTemplateCatalog {
    pub fn from_templates(
        templates: Vec<StrategicTaskTemplate>,
    ) -> Result<Self, StrategicTaskCatalogError> {
        let mut by_id = HashMap::new();
        for (index, template) in templates.iter().enumerate() {
            if template.id.as_str().is_empty() {
                return Err(StrategicTaskCatalogError::EmptyTemplateId);
            }
            if by_id.insert(template.id.clone(), index).is_some() {
                return Err(StrategicTaskCatalogError::DuplicateTemplateId(
                    template.id.clone(),
                ));
            }
        }
        Ok(Self { templates, by_id })
    }

    pub fn templates(&self) -> &[StrategicTaskTemplate] {
        &self.templates
    }

    pub fn get(&self, id: &StrategicTaskTemplateId) -> Option<&StrategicTaskTemplate> {
        self.by_id.get(id).map(|&i| &self.templates[i])
    }

    /// Templates matching a chosen response (response-id templates win over type-only).
    pub fn templates_for_response(
        &self,
        response_id: &ResponseId,
        response_type: ResponseType,
    ) -> Vec<&StrategicTaskTemplate> {
        let mut specific = Vec::new();
        let mut generic = Vec::new();
        for template in self.templates.iter().filter(|t| t.enabled) {
            if !template.matches_response(response_id, response_type) {
                continue;
            }
            if template.response_id.is_some() {
                specific.push(template);
            } else {
                generic.push(template);
            }
        }
        if !specific.is_empty() {
            specific
        } else {
            generic
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrategicTaskCatalogError {
    EmptyTemplateId,
    DuplicateTemplateId(StrategicTaskTemplateId),
}

impl std::fmt::Display for StrategicTaskCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTemplateId => write!(f, "strategic task template has empty id"),
            Self::DuplicateTemplateId(id) => {
                write!(f, "duplicate strategic task template `{}`", id.as_str())
            }
        }
    }
}
