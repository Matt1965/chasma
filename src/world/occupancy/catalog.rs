//! Footprint catalog registry (ADR-080 B3).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::OccupancyError;
use super::footprint::FootprintDefinition;

/// Stable footprint identifier referenced by building definitions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect, Serialize, Deserialize)]
pub struct FootprintId(pub String);

impl FootprintId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Catalog of footprint definitions and baked masks.
#[derive(Debug, Clone, Resource, Reflect)]
pub struct FootprintCatalog {
    definitions: BTreeMap<FootprintId, FootprintDefinition>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FootprintCatalogError {
    DuplicateId(FootprintId),
    Validation(OccupancyError),
}

impl FootprintCatalog {
    pub fn from_definitions(
        definitions: Vec<FootprintDefinition>,
    ) -> Result<Self, FootprintCatalogError> {
        let mut catalog = Self {
            definitions: BTreeMap::new(),
        };
        for definition in definitions {
            definition
                .validate()
                .map_err(FootprintCatalogError::Validation)?;
            if catalog.definitions.contains_key(&definition.id) {
                return Err(FootprintCatalogError::DuplicateId(definition.id.clone()));
            }
            catalog
                .definitions
                .insert(definition.id.clone(), definition);
        }
        Ok(catalog)
    }

    pub fn get(&self, id: &FootprintId) -> Option<&FootprintDefinition> {
        self.definitions.get(id)
    }

    pub fn definitions(&self) -> impl Iterator<Item = &FootprintDefinition> {
        self.definitions.values()
    }

    pub fn insert(&mut self, definition: FootprintDefinition) -> Result<(), FootprintCatalogError> {
        definition
            .validate()
            .map_err(FootprintCatalogError::Validation)?;
        if self.definitions.contains_key(&definition.id) {
            return Err(FootprintCatalogError::DuplicateId(definition.id.clone()));
        }
        self.definitions.insert(definition.id.clone(), definition);
        Ok(())
    }
}

#[cfg(any(test, feature = "dev"))]
mod fixtures {
    use super::*;
    use crate::world::building::starter_definitions;
    use crate::world::occupancy::footprint::inline_footprint_from_building;

    pub fn starter_footprint_definitions() -> Vec<FootprintDefinition> {
        let mut definitions: Vec<FootprintDefinition> = starter_definitions()
            .iter()
            .filter_map(inline_footprint_from_building)
            .collect();
        definitions.extend(field_sampling_footprints());
        definitions
    }

    fn field_sampling_footprints() -> Vec<FootprintDefinition> {
        use crate::world::FootprintShape;
        vec![
            FootprintDefinition::new(
                FootprintId::new("quarry_excavation"),
                FootprintShape::Rectangle {
                    width_meters: 12.0,
                    depth_meters: 12.0,
                },
            ),
            FootprintDefinition::new(
                FootprintId::new("farm_cultivation"),
                FootprintShape::Rectangle {
                    width_meters: 16.0,
                    depth_meters: 12.0,
                },
            ),
            FootprintDefinition::new(
                FootprintId::new("well_extraction"),
                FootprintShape::Circle { radius_meters: 1.5 },
            ),
        ]
    }
}

#[cfg(any(test, feature = "dev"))]
pub use fixtures::starter_footprint_definitions;

#[cfg(not(any(test, feature = "dev")))]
pub fn starter_footprint_definitions() -> Vec<FootprintDefinition> {
    Vec::new()
}

impl Default for FootprintCatalog {
    fn default() -> Self {
        Self::from_definitions(starter_footprint_definitions()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BakedCellMask, FootprintShape};
    use std::collections::BTreeSet;

    #[test]
    fn duplicate_footprint_id_rejected() {
        let id = FootprintId::new("test");
        let a = FootprintDefinition::new(id.clone(), FootprintShape::Circle { radius_meters: 1.0 });
        let b = FootprintDefinition::new(id, FootprintShape::Circle { radius_meters: 2.0 });
        assert!(FootprintCatalog::from_definitions(vec![a, b]).is_err());
    }

    #[test]
    fn starter_footprints_load() {
        let catalog = FootprintCatalog::default();
        assert!(catalog.get(&FootprintId::new("hut")).is_some());
    }
}
