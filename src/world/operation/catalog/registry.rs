//! Read-only operation definition registry (EP3).

use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::OperationDefinition;
use super::definition_id::OperationDefinitionId;
use super::starter::starter_definitions;
use super::validation::OperationCatalogError;
use super::{OperationCategory, OperationInputDefinition, OperationOutputDefinition};

/// Read-only registry of operation definitions (EP3).
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct OperationCatalog {
    definitions: Vec<OperationDefinition>,
    by_id: HashMap<OperationDefinitionId, usize>,
}

impl Default for OperationCatalog {
    fn default() -> Self {
        Self::from_definitions(starter_definitions()).expect("operation catalog is valid")
    }
}

impl OperationCatalog {
    pub fn from_definitions(
        definitions: Vec<OperationDefinition>,
    ) -> Result<Self, OperationCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        for (index, definition) in definitions.iter().enumerate() {
            validate_definition(definition)?;
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(OperationCatalogError::DuplicateId(definition.id.clone()));
            }
        }
        Ok(Self { definitions, by_id })
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn definitions(&self) -> &[OperationDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &OperationDefinitionId) -> Option<&OperationDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &OperationDefinition> {
        self.definitions
            .iter()
            .filter(|definition| definition.enabled)
    }

    pub fn definitions_for_category(
        &self,
        category: OperationCategory,
    ) -> impl Iterator<Item = &OperationDefinition> {
        self.definitions
            .iter()
            .filter(move |definition| definition.category == category)
    }
}

fn validate_definition(definition: &OperationDefinition) -> Result<(), OperationCatalogError> {
    if definition.display_name.trim().is_empty() {
        return Err(OperationCatalogError::InvalidDefinition {
            operation_id: definition.id.clone(),
            reason: "display name is empty".into(),
        });
    }
    if definition.max_workers == 0 {
        return Err(OperationCatalogError::InvalidDefinition {
            operation_id: definition.id.clone(),
            reason: "max_workers must be greater than zero".into(),
        });
    }
    if definition.base_labor == 0 {
        return Err(OperationCatalogError::InvalidDefinition {
            operation_id: definition.id.clone(),
            reason: "base_labor must be greater than zero".into(),
        });
    }
    for input in &definition.inputs {
        input.validate().map_err(|err| OperationCatalogError::InvalidDefinition {
            operation_id: definition.id.clone(),
            reason: format!("invalid input: {err:?}"),
        })?;
    }
    for output in &definition.outputs {
        output.validate().map_err(|err| OperationCatalogError::InvalidDefinition {
            operation_id: definition.id.clone(),
            reason: format!("invalid output: {err:?}"),
        })?;
    }
    let mut seen_terrain_fields = std::collections::HashSet::new();
    for terrain in &definition.terrain_requirements {
        terrain.validate().map_err(|err| OperationCatalogError::InvalidDefinition {
            operation_id: definition.id.clone(),
            reason: format!("invalid terrain requirement: {err:?}"),
        })?;
        if !seen_terrain_fields.insert(terrain.field_id.clone()) {
            return Err(OperationCatalogError::InvalidDefinition {
                operation_id: definition.id.clone(),
                reason: format!(
                    "duplicate terrain field requirement `{}`",
                    terrain.field_id.as_str()
                ),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::operation::catalog::starter::test_workbench_operation;

    #[test]
    fn starter_operations_resolve() {
        let catalog = OperationCatalog::default();
        assert!(catalog.get(&OperationDefinitionId::new("mine_iron")).is_some());
        assert!(catalog.get(&OperationDefinitionId::new("research")).is_some());
    }

    #[test]
    fn duplicate_operation_id_rejected() {
        let mut defs = starter_definitions();
        defs.push(defs[0].clone());
        assert!(matches!(
            OperationCatalog::from_definitions(defs),
            Err(OperationCatalogError::DuplicateId(_))
        ));
    }

    #[test]
    fn test_workbench_operation_resolves() {
        let catalog =
            OperationCatalog::from_definitions(vec![test_workbench_operation()]).unwrap();
        assert!(catalog
            .get(&OperationDefinitionId::new("test_workbench_op"))
            .is_some());
    }

    #[test]
    fn future_io_structures_serialize() {
        let input = OperationInputDefinition {
            item_id: crate::world::ItemDefinitionId::new("iron_ore"),
            quantity: 2,
            source_binding: None,
        };
        let output = OperationOutputDefinition::Item {
            item_id: crate::world::ItemDefinitionId::new("iron_ore"),
            quantity: 1,
            destination_binding: None,
        };
        let input_ron = ron::ser::to_string(&input).unwrap();
        let output_ron = ron::ser::to_string(&output).unwrap();
        assert!(input_ron.contains("iron_ore"));
        assert!(output_ron.contains("iron_ore"));
    }
}
