//! Read-only work skill definition registry.

use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::WorkSkillDefinition;
use super::id::WorkSkillId;
use super::starter::starter_work_skill_definitions;

/// Immutable catalog of authored work skill definitions.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct WorkSkillCatalog {
    definitions: Vec<WorkSkillDefinition>,
    by_id: HashMap<WorkSkillId, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkSkillCatalogError {
    EmptyWorkSkillId,
    DuplicateWorkSkillId(WorkSkillId),
}

impl Default for WorkSkillCatalog {
    fn default() -> Self {
        Self::from_definitions(starter_work_skill_definitions())
            .expect("starter work skill catalog is valid")
    }
}

impl WorkSkillCatalog {
    pub fn from_definitions(
        definitions: Vec<WorkSkillDefinition>,
    ) -> Result<Self, WorkSkillCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        for (index, definition) in definitions.iter().enumerate() {
            if definition.id.as_str().is_empty() {
                return Err(WorkSkillCatalogError::EmptyWorkSkillId);
            }
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(WorkSkillCatalogError::DuplicateWorkSkillId(
                    definition.id.clone(),
                ));
            }
        }
        Ok(Self { definitions, by_id })
    }

    pub fn definitions(&self) -> &[WorkSkillDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &WorkSkillId) -> Option<&WorkSkillDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn get_str(&self, id: &str) -> Option<&WorkSkillDefinition> {
        self.get(&WorkSkillId::new(id))
    }

    /// Enabled definitions in stable presentation order.
    pub fn enabled_definitions_ordered(&self) -> Vec<&WorkSkillDefinition> {
        let mut defs: Vec<&WorkSkillDefinition> = self
            .definitions
            .iter()
            .filter(|definition| definition.enabled)
            .collect();
        defs.sort_by(|left, right| {
            left.sort_order
                .cmp(&right.sort_order)
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });
        defs
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}
