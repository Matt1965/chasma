use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::DoodadDefinition;
use super::definition_id::DoodadDefinitionId;
use super::starter::starter_definitions;
use crate::world::DoodadKind;

/// Read-only registry of doodad type definitions (ADR-016).
///
/// Owned as a Bevy [`Resource`] alongside [`crate::world::WorldConfig`], not as
/// part of [`crate::world::WorldData`]. Type definitions are world-independent;
/// instance records remain on `WorldData`.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct DoodadCatalog {
    definitions: Vec<DoodadDefinition>,
    by_id: HashMap<DoodadDefinitionId, usize>,
    by_kind: HashMap<DoodadKind, Vec<usize>>,
}

impl Default for DoodadCatalog {
    fn default() -> Self {
        Self::from_definitions(starter_definitions()).expect("starter catalog is valid")
    }
}

impl DoodadCatalog {
    /// Build a catalog from definitions. Rejects duplicate ids.
    pub fn from_definitions(
        definitions: Vec<DoodadDefinition>,
    ) -> Result<Self, DoodadCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        let mut by_kind: HashMap<DoodadKind, Vec<usize>> = HashMap::new();

        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(DoodadCatalogError::DuplicateId(definition.id.clone()));
            }
            by_kind.entry(definition.kind).or_default().push(index);
        }

        Ok(Self {
            definitions,
            by_id,
            by_kind,
        })
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    /// Stable iteration order matches construction order.
    pub fn definitions(&self) -> &[DoodadDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &DoodadDefinitionId) -> Option<&DoodadDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn definitions_for_kind(&self, kind: DoodadKind) -> impl Iterator<Item = &DoodadDefinition> {
        self.by_kind
            .get(&kind)
            .into_iter()
            .flat_map(|indices| indices.iter().map(|&index| &self.definitions[index]))
    }
}

/// Why [`DoodadCatalog::from_definitions`] rejected input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoodadCatalogError {
    DuplicateId(DoodadDefinitionId),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::DoodadRenderKey;

    #[test]
    fn catalog_contains_starter_definitions() {
        let catalog = DoodadCatalog::default();
        assert_eq!(catalog.len(), starter_definitions().len());
        assert!(catalog.get(&DoodadDefinitionId::new("tree_oak")).is_some());
        assert!(catalog.get(&DoodadDefinitionId::new("resource_node_iron")).is_some());
    }

    #[test]
    fn lookup_by_kind() {
        let catalog = DoodadCatalog::default();
        let trees: Vec<_> = catalog
            .definitions_for_kind(DoodadKind::Tree)
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(trees, vec!["tree_oak", "tree_dead"]);

        let rocks: Vec<_> = catalog
            .definitions_for_kind(DoodadKind::Rock)
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(rocks, vec!["rock_small", "rock_large"]);
    }

    #[test]
    fn lookup_by_definition_id() {
        let catalog = DoodadCatalog::default();
        let oak = catalog.get(&DoodadDefinitionId::new("tree_oak")).unwrap();
        assert_eq!(oak.display_name, "Oak Tree");
        assert_eq!(oak.kind, DoodadKind::Tree);
        assert!(oak.blocks_movement);
        assert!(catalog.get(&DoodadDefinitionId::new("missing")).is_none());
    }

    #[test]
    fn definition_ids_unique() {
        let mut defs = starter_definitions();
        defs.push(defs[0].clone());
        assert!(matches!(
            DoodadCatalog::from_definitions(defs),
            Err(DoodadCatalogError::DuplicateId(id)) if id.as_str() == "tree_oak"
        ));
    }

    #[test]
    fn placement_constraints_preserved() {
        let catalog = DoodadCatalog::default();
        let ruin = catalog.get(&DoodadDefinitionId::new("ruin_stone")).unwrap();
        assert_eq!(ruin.placement_radius_meters, 8.0);
        assert_eq!(ruin.min_scale, 1.0);
        assert_eq!(ruin.max_scale, 1.0);
        assert_eq!(ruin.max_slope_degrees, Some(15.0));
        assert!(ruin.enabled);
    }

    #[test]
    fn render_key_preserved() {
        let catalog = DoodadCatalog::default();
        let rock = catalog.get(&DoodadDefinitionId::new("rock_large")).unwrap();
        assert_eq!(
            rock.render_key,
            DoodadRenderKey::reserved("rock/large")
        );
    }

    #[test]
    fn catalog_iteration_stable() {
        let catalog = DoodadCatalog::default();
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
        assert_eq!(
            ids_a,
            vec![
                "tree_oak",
                "tree_dead",
                "rock_small",
                "rock_large",
                "bush_scrub",
                "ruin_stone",
                "resource_node_iron",
            ]
        );
    }
}
