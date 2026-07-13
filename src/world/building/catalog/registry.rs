use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::BuildingDefinition;
use super::definition_id::BuildingDefinitionId;
use super::starter::starter_definitions;
use crate::world::building::category::{BuildingCategoryCatalog, BuildingCategoryId};

/// Read-only registry of building type definitions (ADR-078 B1).
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct BuildingCatalog {
    definitions: Vec<BuildingDefinition>,
    by_id: HashMap<BuildingDefinitionId, usize>,
    by_category: HashMap<BuildingCategoryId, Vec<usize>>,
}

impl Default for BuildingCatalog {
    fn default() -> Self {
        let categories = BuildingCategoryCatalog::default();
        Self::from_definitions(starter_definitions(), &categories)
            .expect("building catalog is valid")
    }
}

/// Why [`BuildingCatalog::from_definitions`] or validation rejected input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingCatalogError {
    DuplicateId(BuildingDefinitionId),
    UnknownCategory {
        building_id: BuildingDefinitionId,
        category_id: BuildingCategoryId,
    },
}

impl std::fmt::Display for BuildingCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate building id `{id}`"),
            Self::UnknownCategory {
                building_id,
                category_id,
            } => write!(
                f,
                "building `{}` references unknown category `{}`",
                building_id.as_str(),
                category_id.as_str()
            ),
        }
    }
}

impl BuildingCatalog {
    pub fn from_definitions(
        definitions: Vec<BuildingDefinition>,
        categories: &BuildingCategoryCatalog,
    ) -> Result<Self, BuildingCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        let mut by_category = HashMap::new();

        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(BuildingCatalogError::DuplicateId(definition.id.clone()));
            }
            if !categories.contains(&definition.category_id) {
                return Err(BuildingCatalogError::UnknownCategory {
                    building_id: definition.id.clone(),
                    category_id: definition.category_id.clone(),
                });
            }
            by_category
                .entry(definition.category_id.clone())
                .or_insert_with(Vec::new)
                .push(index);
        }

        Ok(Self {
            definitions,
            by_id,
            by_category,
        })
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn definitions(&self) -> &[BuildingDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &BuildingDefinitionId) -> Option<&BuildingDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn definitions_for_category(
        &self,
        category_id: &BuildingCategoryId,
    ) -> impl Iterator<Item = &BuildingDefinition> {
        self.by_category
            .get(category_id)
            .into_iter()
            .flat_map(|indices| indices.iter().map(|&index| &self.definitions[index]))
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &BuildingDefinition> {
        self.definitions
            .iter()
            .filter(|definition| definition.enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::catalog::render_key::BuildingRenderKey;
    use crate::world::building::footprint::FootprintSpec;

    #[test]
    fn starter_buildings_load() {
        let catalog = BuildingCatalog::default();
        assert!(catalog.get(&BuildingDefinitionId::new("hut")).is_some());
    }

    #[test]
    fn lookup_by_building_id() {
        let catalog = BuildingCatalog::default();
        let hut = catalog.get(&BuildingDefinitionId::new("hut")).unwrap();
        assert_eq!(hut.display_name, "Survival Hut");
        assert!(catalog.get(&BuildingDefinitionId::new("missing")).is_none());
    }

    #[test]
    fn duplicate_building_id_rejected() {
        let categories = BuildingCategoryCatalog::default();
        let mut defs = starter_definitions();
        defs.push(defs[0].clone());
        assert!(matches!(
            BuildingCatalog::from_definitions(defs, &categories),
            Err(BuildingCatalogError::DuplicateId(_))
        ));
    }

    #[test]
    fn unknown_category_rejected() {
        let categories = BuildingCategoryCatalog::default();
        let definition = BuildingDefinition::new(
            BuildingDefinitionId::new("orphan"),
            "Orphan",
            BuildingCategoryId::new("missing"),
            BuildingRenderKey::reserved("hut"),
            BuildingRenderKey::reserved("hut_collision"),
            100,
            30.0,
            FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            40.0,
            true,
        );
        assert!(matches!(
            BuildingCatalog::from_definitions(vec![definition], &categories),
            Err(BuildingCatalogError::UnknownCategory { .. })
        ));
    }
}
