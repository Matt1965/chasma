use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::UnitDefinition;
use super::definition_id::UnitDefinitionId;
use super::starter::starter_definitions;

/// Read-only registry of unit type definitions (ADR-027).
///
/// Owned as a Bevy [`Resource`] alongside [`crate::world::WorldConfig`], not on
/// [`crate::world::WorldData`]. Type definitions are world-independent.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct UnitCatalog {
    definitions: Vec<UnitDefinition>,
    by_id: HashMap<UnitDefinitionId, usize>,
}

impl Default for UnitCatalog {
    /// Empty outside unit tests; fixtures come from Excel import at runtime (ADR-049).
    fn default() -> Self {
        Self::from_definitions(starter_definitions()).expect("unit catalog is valid")
    }
}

impl UnitCatalog {
    /// Build a catalog from definitions. Rejects duplicate ids.
    pub fn from_definitions(definitions: Vec<UnitDefinition>) -> Result<Self, UnitCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());

        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(UnitCatalogError::DuplicateId(definition.id.clone()));
            }
        }

        Ok(Self {
            definitions,
            by_id,
        })
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    /// Stable iteration order matches construction order.
    pub fn definitions(&self) -> &[UnitDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &UnitDefinitionId) -> Option<&UnitDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    /// Enabled definitions only (catalog rows with `enabled == true` at import).
    pub fn enabled_definitions(&self) -> impl Iterator<Item = &UnitDefinition> {
        self.definitions.iter().filter(|d| d.enabled)
    }
}

/// Why [`UnitCatalog::from_definitions`] rejected input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitCatalogError {
    DuplicateId(UnitDefinitionId),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::UnitRenderKey;

    #[test]
    fn catalog_contains_starter_definitions() {
        let catalog = UnitCatalog::default();
        assert_eq!(catalog.len(), starter_definitions().len());
        assert!(catalog.get(&UnitDefinitionId::new("wolf")).is_some());
        assert!(catalog.get(&UnitDefinitionId::new("bandit")).is_some());
    }

    #[test]
    fn lookup_by_definition_id() {
        let catalog = UnitCatalog::default();
        let wolf = catalog.get(&UnitDefinitionId::new("wolf")).unwrap();
        assert_eq!(wolf.display_name, "Wolf");
        assert!(catalog.get(&UnitDefinitionId::new("missing")).is_none());
    }

    #[test]
    fn definition_ids_unique() {
        let mut defs = starter_definitions();
        defs.push(defs[0].clone());
        assert!(matches!(
            UnitCatalog::from_definitions(defs),
            Err(UnitCatalogError::DuplicateId(id)) if id.as_str() == "wolf"
        ));
    }

    #[test]
    fn catalog_iteration_stable() {
        let catalog = UnitCatalog::default();
        let ids_a: Vec<_> = catalog
            .definitions()
            .iter()
            .map(|d| d.id.as_str())
            .collect();
        let ids_b: Vec<_> = catalog
            .definitions()
            .iter()
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(ids_a, ids_b);
        assert_eq!(ids_a, vec!["wolf", "bandit", "deer"]);
    }

    #[test]
    fn enabled_definitions_filter() {
        let catalog = UnitCatalog::default();
        assert_eq!(catalog.enabled_definitions().count(), catalog.len());
    }

    #[test]
    fn render_key_preserved() {
        let catalog = UnitCatalog::default();
        let wolf = catalog.get(&UnitDefinitionId::new("wolf")).unwrap();
        assert_eq!(wolf.render_key, UnitRenderKey::reserved("wolf"));
    }

    #[test]
    fn runtime_default_catalog_is_empty_without_test_fixtures() {
        let catalog = UnitCatalog::from_definitions(Vec::new()).unwrap();
        assert!(catalog.is_empty());
    }
}
