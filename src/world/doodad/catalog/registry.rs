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
    /// Legacy scene/procgen ids that resolve to an existing definition (same index, no duplicate row).
    legacy_aliases: HashMap<DoodadDefinitionId, usize>,
}

impl Default for DoodadCatalog {
    /// Empty outside unit tests; fixtures come from Excel import at runtime (ADR-049).
    fn default() -> Self {
        Self::from_definitions(starter_definitions()).expect("doodad catalog is valid")
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
            legacy_aliases: HashMap::new(),
        })
    }

    /// Map legacy definition ids (e.g. scene `tree_oak`) onto canonical Excel-imported rows by id.
    ///
    /// Skips aliases when the canonical id is missing or the alias id already owns a definition.
    pub fn with_legacy_aliases(
        mut self,
        aliases: impl IntoIterator<Item = (DoodadDefinitionId, DoodadDefinitionId)>,
    ) -> Result<Self, DoodadCatalogError> {
        for (alias, canonical) in aliases {
            if self.by_id.contains_key(&alias) {
                continue;
            }
            let Some(&index) = self.by_id.get(&canonical) else {
                continue;
            };
            if self.legacy_aliases.insert(alias, index).is_some() {
                return Err(DoodadCatalogError::DuplicateId(canonical));
            }
        }
        Ok(self)
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
        self.by_id
            .get(id)
            .or_else(|| self.legacy_aliases.get(id))
            .map(|&index| &self.definitions[index])
    }

    pub fn definitions_for_kind(
        &self,
        kind: DoodadKind,
    ) -> impl Iterator<Item = &DoodadDefinition> {
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
    use crate::world::doodad::catalog::definition::DoodadDefinition;
    use crate::world::DoodadRenderKey;

    fn starter_catalog() -> DoodadCatalog {
        DoodadCatalog::default()
    }

    #[test]
    fn catalog_contains_starter_definitions() {
        let catalog = starter_catalog();
        assert_eq!(catalog.len(), starter_definitions().len());
        assert!(catalog.get(&DoodadDefinitionId::new("tree_oak")).is_some());
        assert!(
            catalog
                .get(&DoodadDefinitionId::new("resource_node_iron"))
                .is_some()
        );
    }

    #[test]
    fn lookup_by_kind() {
        let catalog = starter_catalog();
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
        let catalog = starter_catalog();
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
        let catalog = starter_catalog();
        let ruin = catalog.get(&DoodadDefinitionId::new("ruin_stone")).unwrap();
        assert_eq!(ruin.placement_radius_meters, 8.0);
        assert_eq!(ruin.min_scale, 1.0);
        assert_eq!(ruin.max_scale, 1.0);
        assert_eq!(ruin.max_slope_degrees, Some(15.0));
        assert!(ruin.enabled);
    }

    #[test]
    fn render_key_preserved() {
        let catalog = starter_catalog();
        let rock = catalog.get(&DoodadDefinitionId::new("rock_large")).unwrap();
        assert_eq!(rock.render_key, DoodadRenderKey::reserved("rock/large"));
    }

    #[test]
    fn catalog_iteration_stable() {
        let catalog = starter_catalog();
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
                "interior_chair",
            ]
        );
    }

    #[test]
    fn default_catalog_is_empty_without_test_fixtures() {
        let catalog = DoodadCatalog::from_definitions(Vec::new()).unwrap();
        assert!(catalog.is_empty());
    }

    #[test]
    fn legacy_alias_resolves_to_canonical_definition() {
        let defs = vec![DoodadDefinition::new(
            DoodadDefinitionId::new("d_0001"),
            DoodadKind::Tree,
            "Oak",
            4.0,
            0.5,
            1.5,
            None,
            None,
            Some(25.0),
            true,
            DoodadRenderKey::reserved("tree/oak"),
        )];
        let catalog = DoodadCatalog::from_definitions(defs)
            .unwrap()
            .with_legacy_aliases([(
                DoodadDefinitionId::new("tree_oak"),
                DoodadDefinitionId::new("d_0001"),
            )])
            .unwrap();
        assert!(catalog.get(&DoodadDefinitionId::new("d_0001")).is_some());
        let oak = catalog.get(&DoodadDefinitionId::new("tree_oak")).unwrap();
        assert_eq!(oak.id.as_str(), "d_0001");
        assert_eq!(oak.render_key.as_str(), Some("tree/oak"));
    }
}
