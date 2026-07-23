//! Read-only catalog of building navigation blueprints (NV1.1).

use std::collections::BTreeMap;
use std::path::Path;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::definition::BuildingNavigationBlueprint;
use super::error::BuildingNavigationBlueprintError;
use super::id::BuildingNavigationBlueprintId;
use super::starter;

/// Monotonic revision bumped when catalog content changes (hot reload seam).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Reflect, Resource)]
pub struct BuildingNavigationBlueprintCatalogRevision(pub u64);

/// Read-only navigation blueprint catalog.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct BuildingNavigationBlueprintCatalog {
    definitions: Vec<BuildingNavigationBlueprint>,
    #[reflect(ignore)]
    by_id: BTreeMap<BuildingNavigationBlueprintId, usize>,
}

impl Default for BuildingNavigationBlueprintCatalog {
    fn default() -> Self {
        Self::from_definitions(starter::starter_navigation_blueprints())
            .expect("starter navigation blueprints are valid")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildingNavigationBlueprintCatalogRon {
    pub definitions: Vec<BuildingNavigationBlueprint>,
}

impl BuildingNavigationBlueprintCatalog {
    pub fn from_definitions(
        definitions: Vec<BuildingNavigationBlueprint>,
    ) -> Result<Self, BuildingNavigationBlueprintError> {
        let mut by_id = BTreeMap::new();
        for (index, definition) in definitions.iter().enumerate() {
            definition.validate()?;
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(BuildingNavigationBlueprintError::DuplicateId(
                    definition.id.clone(),
                ));
            }
        }
        Ok(Self { definitions, by_id })
    }

    pub fn get(&self, id: &BuildingNavigationBlueprintId) -> Option<&BuildingNavigationBlueprint> {
        self.by_id.get(id).map(|index| &self.definitions[*index])
    }

    pub fn definitions(&self) -> &[BuildingNavigationBlueprint] {
        &self.definitions
    }

    pub fn enabled_definitions(&self) -> impl Iterator<Item = &BuildingNavigationBlueprint> {
        self.definitions.iter().filter(|def| def.enabled)
    }

    pub fn upsert(
        &mut self,
        blueprint: BuildingNavigationBlueprint,
    ) -> Result<(), BuildingNavigationBlueprintError> {
        blueprint.validate()?;
        if let Some(index) = self.by_id.get(&blueprint.id).copied() {
            self.definitions[index] = blueprint;
        } else {
            let index = self.definitions.len();
            self.by_id.insert(blueprint.id.clone(), index);
            self.definitions.push(blueprint);
        }
        Ok(())
    }

    pub fn load_from_ron_path(path: &Path) -> Result<Self, BuildingNavigationBlueprintError> {
        let text = std::fs::read_to_string(path)
            .map_err(|err| BuildingNavigationBlueprintError::RonIo(err.to_string()))?;
        Self::load_from_ron(&text)
    }

    pub fn load_from_ron(text: &str) -> Result<Self, BuildingNavigationBlueprintError> {
        let file: BuildingNavigationBlueprintCatalogRon = ron::from_str(text)
            .map_err(|err| BuildingNavigationBlueprintError::RonParse(err.to_string()))?;
        Self::from_definitions(file.definitions)
    }
}

pub const BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH: &str =
    "assets/buildings/navigation_blueprints/catalog.ron";

pub fn load_building_navigation_blueprint_catalog() -> BuildingNavigationBlueprintCatalog {
    BuildingNavigationBlueprintCatalog::load_from_ron_path(Path::new(
        BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH,
    ))
    .unwrap_or_else(|err| {
        bevy::log::warn!(
            "building navigation blueprint catalog missing or invalid at {BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH} ({err}); using starter blueprints"
        );
        BuildingNavigationBlueprintCatalog::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_catalog_loads() {
        let catalog = BuildingNavigationBlueprintCatalog::default();
        assert!(catalog.get(&BuildingNavigationBlueprintId::new("two_story_hut")).is_some());
        assert!(catalog.get(&BuildingNavigationBlueprintId::new("barn_interior")).is_some());
    }
}
