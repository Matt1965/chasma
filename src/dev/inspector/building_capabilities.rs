//! Authoritative capability probes for Selected Object building sections.

use crate::world::{
    BuildingCatalog, BuildingDefinition, BuildingFieldRequirementCatalog, BuildingId, WorldData,
    definition_requires_inventory_allocation,
};

/// Which building dev UI sections are meaningful for the current selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildingDevCapabilities {
    pub construction: bool,
    pub lifecycle: bool,
    pub production: bool,
    pub production_operation_selector: bool,
    pub inventory: bool,
    pub doors: bool,
    pub terrain: bool,
}

impl BuildingDevCapabilities {
    pub fn for_building(
        world: &WorldData,
        building_catalog: &BuildingCatalog,
        requirement_catalog: &BuildingFieldRequirementCatalog,
        building_id: BuildingId,
    ) -> Option<Self> {
        let record = world.get_building(building_id)?;
        let definition = building_catalog.get(&record.definition_id)?;
        Some(Self::from_definition(
            world,
            requirement_catalog,
            building_id,
            definition,
        ))
    }

    pub fn from_definition(
        world: &WorldData,
        requirement_catalog: &BuildingFieldRequirementCatalog,
        building_id: BuildingId,
        definition: &BuildingDefinition,
    ) -> Self {
        let operation_count = definition.supported_operations.len();
        let production = operation_count > 0;
        let inventory = definition_requires_inventory_allocation(definition)
            || world
                .building_inventory_binding_store()
                .get(building_id)
                .is_some_and(|set| !set.bindings().is_empty());
        let doors = !world.door_store().building_door_ids(building_id).is_empty();
        let terrain = !requirement_catalog
            .requirements_for_building(&definition.id)
            .is_empty();
        Self {
            construction: true,
            lifecycle: true,
            production,
            production_operation_selector: operation_count > 1,
            inventory,
            doors,
            terrain,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingDefinition, BuildingFieldRequirementCatalog, BuildingId, WorldConfig, WorldData,
        starter_building_definitions,
    };

    fn definition(id: &str) -> BuildingDefinition {
        starter_building_definitions()
            .into_iter()
            .find(|def| def.id.as_str() == id)
            .unwrap_or_else(|| panic!("missing building definition {id}"))
    }

    #[test]
    fn farm_has_production_inventory_and_terrain_without_doors() {
        let world = WorldData::new(WorldConfig::default().chunk_layout());
        let caps = BuildingDevCapabilities::from_definition(
            &world,
            &BuildingFieldRequirementCatalog::default(),
            BuildingId::new(1),
            &definition("prispod_farm"),
        );
        assert!(caps.production);
        assert!(!caps.production_operation_selector);
        assert!(caps.inventory);
        assert!(!caps.doors);
        assert!(caps.terrain);
    }

    #[test]
    fn storage_chest_has_inventory_without_production_or_terrain() {
        let world = WorldData::new(WorldConfig::default().chunk_layout());
        let caps = BuildingDevCapabilities::from_definition(
            &world,
            &BuildingFieldRequirementCatalog::default(),
            BuildingId::new(2),
            &definition("storage_chest"),
        );
        assert!(!caps.production);
        assert!(caps.inventory);
        assert!(!caps.terrain);
        assert!(!caps.doors);
    }

    #[test]
    fn non_production_building_has_no_production_section() {
        let world = WorldData::new(WorldConfig::default().chunk_layout());
        let caps = BuildingDevCapabilities::from_definition(
            &world,
            &BuildingFieldRequirementCatalog::default(),
            BuildingId::new(4),
            &definition("hut"),
        );
        assert!(!caps.production);
        assert!(!caps.production_operation_selector);
    }

    #[test]
    fn workbench_exposes_operation_selector_for_multiple_operations() {
        let world = WorldData::new(WorldConfig::default().chunk_layout());
        let caps = BuildingDevCapabilities::from_definition(
            &world,
            &BuildingFieldRequirementCatalog::default(),
            BuildingId::new(3),
            &definition("workbench"),
        );
        assert!(caps.production);
        assert!(caps.production_operation_selector);
    }

    #[test]
    fn farm_definition_has_no_navigation_blueprint() {
        let farm = definition("prispod_farm");
        assert!(farm.navigation_blueprint_id.is_none());
        assert!(farm.interior_profile_id.is_none());
    }
}
