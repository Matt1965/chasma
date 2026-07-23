//! Authoritative spawn helpers for dev mode (ADR-043).

use bevy::prelude::*;

use crate::world::{
    BuildingCatalog, BuildingDefinitionId, BuildingOwnership, BuildingSource, DoodadCatalog,
    DoodadDefinitionId, DoodadPlacementOverrides, DoodadSource, FootprintCatalog,
    InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, UnitCatalog,
    UnitDefinitionId, UnitOwnership, UnitSource, WorldData, WorldPosition,
    create_dev_complete_building, create_dev_complete_building_with_inventory, create_doodad,
    create_unit_with_inventory, starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions,
};

use super::dev_mode::{DefinitionId, SpawnMode};

/// Outcome of a dev spawn attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevSpawnOutcome {
    SpawnedUnit { definition_id: UnitDefinitionId },
    SpawnedDoodad { definition_id: DoodadDefinitionId },
    SpawnedBuilding { definition_id: BuildingDefinitionId },
    NoDefinitionSelected,
    TerrainMiss,
    AuthoringFailed(String),
}

/// Resolve a terrain raycast hit into a grounded spawn position.
pub fn dev_spawn_position_from_terrain_click(
    world: &WorldData,
    world_position: WorldPosition,
) -> Option<WorldPosition> {
    crate::world::ground_world_position(world, world_position)
}

/// Spawn the selected definition at an authoritative world position via WorldData APIs.
pub fn spawn_selected_at_position(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    selected: Option<&DefinitionId>,
    position: WorldPosition,
    spawn_affiliation: crate::world::Affiliation,
) -> DevSpawnOutcome {
    let Some(definition) = selected else {
        return DevSpawnOutcome::NoDefinitionSelected;
    };

    match definition {
        DefinitionId::Unit(definition_id) => {
            let ownership = UnitOwnership::with_affiliation(spawn_affiliation);
            match create_unit_with_inventory(
                unit_catalog,
                world,
                definition_id,
                position,
                UnitSource::Dev,
                ownership,
                inventory_ctx,
            ) {
                Ok(_record) => DevSpawnOutcome::SpawnedUnit {
                    definition_id: definition_id.clone(),
                },
                Err(error) => DevSpawnOutcome::AuthoringFailed(format!("{error:?}")),
            }
        }
        DefinitionId::Doodad(definition_id) => match create_doodad(
            doodad_catalog,
            world,
            definition_id,
            position,
            DoodadSource::Dev,
            DoodadPlacementOverrides::default(),
            None,
        ) {
            Ok(_record) => DevSpawnOutcome::SpawnedDoodad {
                definition_id: definition_id.clone(),
            },
            Err(error) => DevSpawnOutcome::AuthoringFailed(format!("{error:?}")),
        },
        DefinitionId::Building(definition_id) => {
            let ownership = BuildingOwnership::with_affiliation(spawn_affiliation);
            let position = crate::world::ground_and_quantize_building_anchor(world, position)
                .unwrap_or(position);
            let occupancy = crate::world::OccupancyCatalogs {
                doodad: doodad_catalog,
                building: building_catalog,
                footprint: footprint_catalog,
            };
            let result = if building_catalog
                .get(definition_id)
                .is_some_and(|def| def.inventory_profile_id.is_some())
            {
                create_dev_complete_building_with_inventory(
                    building_catalog,
                    world,
                    definition_id,
                    position,
                    Quat::IDENTITY,
                    ownership,
                    Some(occupancy),
                    inventory_ctx,
                )
            } else {
                create_dev_complete_building(
                    building_catalog,
                    world,
                    definition_id,
                    position,
                    Quat::IDENTITY,
                    ownership,
                    Some(occupancy),
                )
            };
            match result {
                Ok(_record) => DevSpawnOutcome::SpawnedBuilding {
                    definition_id: definition_id.clone(),
                },
                Err(error) => DevSpawnOutcome::AuthoringFailed(format!("{error:?}")),
            }
        }
        DefinitionId::Item(_) | DefinitionId::InventoryProfile(_) => {
            DevSpawnOutcome::NoDefinitionSelected
        }
    }
}

/// Convenience for tests — spawn by explicit mode + id string.
pub fn spawn_by_mode_at_position(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    mode: SpawnMode,
    definition_key: &str,
    position: WorldPosition,
) -> DevSpawnOutcome {
    let selected = match mode {
        SpawnMode::Unit => DefinitionId::Unit(UnitDefinitionId::new(definition_key)),
        SpawnMode::Doodad => DefinitionId::Doodad(DoodadDefinitionId::new(definition_key)),
        SpawnMode::Building => DefinitionId::Building(BuildingDefinitionId::new(definition_key)),
    };
    spawn_selected_at_position(
        world,
        unit_catalog,
        doodad_catalog,
        building_catalog,
        footprint_catalog,
        inventory_ctx,
        Some(&selected),
        position,
        crate::world::Affiliation::Player,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCatalog, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition,
    };
    use bevy::prelude::Vec3;

    fn flat_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn inventory_ctx() -> InventoryCatalogCtx<'static> {
        let categories = Box::leak(Box::new(
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap(),
        ));
        let items = Box::leak(Box::new(
            ItemCatalog::from_definitions(starter_item_definitions(), categories).unwrap(),
        ));
        let profiles = Box::leak(Box::new(
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions()).unwrap(),
        ));
        InventoryCatalogCtx::new(items, categories, profiles)
    }

    #[test]
    fn spawn_uses_world_data_unit_api() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let position = pos(40.0, 40.0);
        let building_catalog = BuildingCatalog::default();
        let footprint_catalog = FootprintCatalog::default();
        let ctx = inventory_ctx();
        let outcome = spawn_by_mode_at_position(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &building_catalog,
            &footprint_catalog,
            &ctx,
            SpawnMode::Unit,
            "wolf",
            position,
        );
        assert!(matches!(outcome, DevSpawnOutcome::SpawnedUnit { .. }));
        let store = world
            .units_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
            .unwrap();
        assert_eq!(store.len(), 1);
        let record = store.records()[0].clone();
        assert_eq!(record.source, UnitSource::Dev);
    }

    #[test]
    fn spawn_uses_world_data_doodad_api() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let position = pos(50.0, 50.0);
        let building_catalog = BuildingCatalog::default();
        let footprint_catalog = FootprintCatalog::default();
        let ctx = inventory_ctx();
        let outcome = spawn_by_mode_at_position(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &building_catalog,
            &footprint_catalog,
            &ctx,
            SpawnMode::Doodad,
            "tree_oak",
            position,
        );
        assert!(matches!(outcome, DevSpawnOutcome::SpawnedDoodad { .. }));
        let store = world
            .doodads_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
            .unwrap();
        assert_eq!(store.len(), 1);
        assert_eq!(store.records()[0].source, DoodadSource::Dev);
    }

    #[test]
    fn terrain_spawn_position_grounds_click() {
        let world = flat_world();
        let candidate = pos(12.0, 18.0);
        let grounded = dev_spawn_position_from_terrain_click(&world, candidate).unwrap();
        assert_eq!(grounded.chunk, candidate.chunk);
    }
}
