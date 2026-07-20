//! Authoritative building placement API (ADR-079 B2, ADR-080 B3).
//!
//! Operates on [`crate::world::WorldData`] and [`super::catalog::BuildingCatalog`].
//! No ECS entities, rendering, or construction simulation.

use bevy::prelude::*;

use super::catalog::BuildingCatalog;
use super::id::BuildingId;
use super::insert::BuildingInsertError;
use super::interaction_profile::BuildingInteractionProfileCatalog;
use super::inventory::{
    BuildingInventoryCleanup, BuildingInventoryRemovalPolicy, attach_inventory_on_building_create,
    finalize_building_inventory_removal,
};
use super::inventory_binding::effective_inventory_binding_definitions;
use super::inventory_error::BuildingInventoryError;
use super::ownership::BuildingOwnership;
use super::placement::BuildingPlacement;
use super::record::BuildingRecord;
use super::source::BuildingSource;
use super::state::BuildingLifecycleState;
use super::state::ConstructionState;
use super::vitals::BuildingVitals;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::{
    BuildingDefinitionId, DoodadCatalog, OccupancyCatalogs, OccupancySource, WorldData,
    WorldPosition, deactivate_building_interior,
};
use crate::world::{
    register_building_occupancy, unregister_source_occupancy, update_building_occupancy,
};

/// Why an authoring operation failed (ADR-079 B2).
#[derive(Debug, Clone, PartialEq)]
pub enum BuildingAuthoringError {
    DefinitionNotFound(BuildingDefinitionId),
    DefinitionDisabled(BuildingDefinitionId),
    BuildingNotFound(BuildingId),
    ChunkPlacementMismatch,
    Occupancy(crate::world::OccupancyError),
    InventoryAllocationFailed(BuildingId),
    Inventory(BuildingInventoryError),
}

/// Create a building instance from a catalog definition and insert it into world data.
pub fn create_building(
    catalog: &BuildingCatalog,
    world: &mut WorldData,
    definition_id: &BuildingDefinitionId,
    position: WorldPosition,
    rotation: Quat,
    source: BuildingSource,
    ownership: BuildingOwnership,
    occupancy: Option<OccupancyCatalogs<'_>>,
) -> Result<BuildingRecord, BuildingAuthoringError> {
    create_building_impl(
        catalog,
        world,
        definition_id,
        position,
        rotation,
        source,
        ownership,
        occupancy,
        None,
    )
}

/// Create a building and allocate its container inventory when the definition has a profile.
pub fn create_building_with_inventory(
    catalog: &BuildingCatalog,
    world: &mut WorldData,
    definition_id: &BuildingDefinitionId,
    position: WorldPosition,
    rotation: Quat,
    source: BuildingSource,
    ownership: BuildingOwnership,
    occupancy: Option<OccupancyCatalogs<'_>>,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> Result<BuildingRecord, BuildingAuthoringError> {
    create_building_impl(
        catalog,
        world,
        definition_id,
        position,
        rotation,
        source,
        ownership,
        occupancy,
        Some(inventory_ctx),
    )
}

fn create_building_impl(
    catalog: &BuildingCatalog,
    world: &mut WorldData,
    definition_id: &BuildingDefinitionId,
    position: WorldPosition,
    rotation: Quat,
    source: BuildingSource,
    ownership: BuildingOwnership,
    occupancy: Option<OccupancyCatalogs<'_>>,
    inventory_ctx: Option<&InventoryCatalogCtx<'_>>,
) -> Result<BuildingRecord, BuildingAuthoringError> {
    let definition = catalog
        .get(definition_id)
        .ok_or_else(|| BuildingAuthoringError::DefinitionNotFound(definition_id.clone()))?;

    if !definition.enabled {
        return Err(BuildingAuthoringError::DefinitionDisabled(
            definition_id.clone(),
        ));
    }

    let id = world.allocate_building_id();
    let mut record = BuildingRecord::new(
        id,
        definition.id.clone(),
        BuildingPlacement::new(position, rotation),
        ownership,
        definition.max_hp,
        source,
    );

    if definition.inventory_profile_id.is_some() && inventory_ctx.is_none() {
        return Err(BuildingAuthoringError::InventoryAllocationFailed(id));
    }

    if let Some(ctx) = inventory_ctx {
        if !effective_inventory_binding_definitions(definition).is_empty() {
            attach_inventory_on_building_create(world, ctx, &mut record, definition).map_err(
                |error| match error {
                    BuildingInventoryError::Inventory(inventory_error) => {
                        BuildingAuthoringError::Inventory(BuildingInventoryError::Inventory(
                            inventory_error,
                        ))
                    }
                    other => BuildingAuthoringError::Inventory(other),
                },
            )?;
        }
    }

    let chunk = crate::world::ChunkId::new(position.chunk);
    if let Err(error) = world.insert_building(chunk, record.clone()) {
        if record.inventory_id.is_some() {
            if let Some(ctx) = inventory_ctx {
                let _ = super::inventory::cleanup_building_inventory_on_delete(world, ctx, &record);
            }
        }
        return Err(match error {
            BuildingInsertError::ChunkPlacementMismatch => {
                BuildingAuthoringError::ChunkPlacementMismatch
            }
            BuildingInsertError::BuildingNotFound => BuildingAuthoringError::BuildingNotFound(id),
        });
    }

    if let Some(catalogs) = occupancy {
        if let Err(error) = register_building_occupancy(world, catalogs, &record) {
            let _ = world.remove_building_by_id(id);
            if record.inventory_id.is_some() {
                if let Some(ctx) = inventory_ctx {
                    let _ =
                        super::inventory::cleanup_building_inventory_on_delete(world, ctx, &record);
                }
            }
            return Err(BuildingAuthoringError::Occupancy(error));
        }
    }

    Ok(record)
}

/// Apply dev Complete spawn policy to an existing building record (ADR-096).
pub fn apply_dev_complete_building_state(
    world: &mut WorldData,
    building_id: BuildingId,
) -> Result<(), BuildingAuthoringError> {
    let Some(record) = world.get_building(building_id) else {
        return Err(BuildingAuthoringError::BuildingNotFound(building_id));
    };
    let max_hp = record.vitals.max_hp.max(1);
    world
        .mutate_building(building_id, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
            record.construction.progress_0_1 = 1.0;
            record.vitals = BuildingVitals::full(max_hp);
            record.source = BuildingSource::Dev;
        })
        .ok_or(BuildingAuthoringError::BuildingNotFound(building_id))?;
    Ok(())
}

/// Create a dev-spawned building in Complete state with full HP and finished construction.
pub fn create_dev_complete_building(
    catalog: &BuildingCatalog,
    world: &mut WorldData,
    definition_id: &BuildingDefinitionId,
    position: WorldPosition,
    rotation: Quat,
    ownership: BuildingOwnership,
    occupancy: Option<OccupancyCatalogs<'_>>,
) -> Result<BuildingRecord, BuildingAuthoringError> {
    let record = create_building(
        catalog,
        world,
        definition_id,
        position,
        rotation,
        BuildingSource::Dev,
        ownership,
        occupancy,
    )?;
    apply_dev_complete_building_state(world, record.id)?;
    world
        .get_building(record.id)
        .cloned()
        .ok_or(BuildingAuthoringError::BuildingNotFound(record.id))
}

/// Dev Complete building creation with optional container inventory.
pub fn create_dev_complete_building_with_inventory(
    catalog: &BuildingCatalog,
    world: &mut WorldData,
    definition_id: &BuildingDefinitionId,
    position: WorldPosition,
    rotation: Quat,
    ownership: BuildingOwnership,
    occupancy: Option<OccupancyCatalogs<'_>>,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> Result<BuildingRecord, BuildingAuthoringError> {
    let record = create_building_with_inventory(
        catalog,
        world,
        definition_id,
        position,
        rotation,
        BuildingSource::Dev,
        ownership,
        occupancy,
        inventory_ctx,
    )?;
    apply_dev_complete_building_state(world, record.id)?;
    world
        .get_building(record.id)
        .cloned()
        .ok_or(BuildingAuthoringError::BuildingNotFound(record.id))
}

/// Place a player-owned building in [`BuildingLifecycleState::Planned`] with atomic occupancy.
///
/// Validation must be performed by the caller before commit. Rolls back the record if occupancy
/// registration fails.
pub fn place_player_building(
    catalog: &BuildingCatalog,
    world: &mut WorldData,
    definition_id: &BuildingDefinitionId,
    position: WorldPosition,
    rotation: Quat,
    ownership: BuildingOwnership,
    occupancy: OccupancyCatalogs<'_>,
) -> Result<BuildingRecord, BuildingAuthoringError> {
    place_player_building_impl(
        catalog,
        world,
        definition_id,
        position,
        rotation,
        ownership,
        occupancy,
        None,
    )
}

/// Planned player placement with container inventory allocation at create time.
pub fn place_player_building_with_inventory(
    catalog: &BuildingCatalog,
    world: &mut WorldData,
    definition_id: &BuildingDefinitionId,
    position: WorldPosition,
    rotation: Quat,
    ownership: BuildingOwnership,
    occupancy: OccupancyCatalogs<'_>,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> Result<BuildingRecord, BuildingAuthoringError> {
    place_player_building_impl(
        catalog,
        world,
        definition_id,
        position,
        rotation,
        ownership,
        occupancy,
        Some(inventory_ctx),
    )
}

fn place_player_building_impl(
    catalog: &BuildingCatalog,
    world: &mut WorldData,
    definition_id: &BuildingDefinitionId,
    position: WorldPosition,
    rotation: Quat,
    ownership: BuildingOwnership,
    occupancy: OccupancyCatalogs<'_>,
    inventory_ctx: Option<&InventoryCatalogCtx<'_>>,
) -> Result<BuildingRecord, BuildingAuthoringError> {
    let definition = catalog
        .get(definition_id)
        .ok_or_else(|| BuildingAuthoringError::DefinitionNotFound(definition_id.clone()))?;

    if !definition.enabled {
        return Err(BuildingAuthoringError::DefinitionDisabled(
            definition_id.clone(),
        ));
    }

    let id = world.allocate_building_id();
    let mut record = BuildingRecord::new(
        id,
        definition.id.clone(),
        BuildingPlacement::new(position, rotation),
        ownership,
        definition.max_hp,
        BuildingSource::Authored,
    );
    record.lifecycle_state = BuildingLifecycleState::Planned;
    record.construction = ConstructionState::default();
    record.vitals = BuildingVitals::construction_vulnerable(definition.max_hp);

    if !effective_inventory_binding_definitions(definition).is_empty() {
        let Some(ctx) = inventory_ctx else {
            return Err(BuildingAuthoringError::InventoryAllocationFailed(id));
        };
        attach_inventory_on_building_create(world, ctx, &mut record, definition)
            .map_err(BuildingAuthoringError::Inventory)?;
    }

    let chunk = crate::world::ChunkId::new(position.chunk);
    world
        .insert_building(chunk, record.clone())
        .map_err(|error| match error {
            BuildingInsertError::ChunkPlacementMismatch => {
                BuildingAuthoringError::ChunkPlacementMismatch
            }
            BuildingInsertError::BuildingNotFound => BuildingAuthoringError::BuildingNotFound(id),
        })?;

    if let Err(error) = register_building_occupancy(world, occupancy, &record) {
        let _ = world.remove_building_by_id(id);
        if record.inventory_id.is_some() {
            if let Some(ctx) = inventory_ctx {
                let _ = super::inventory::cleanup_building_inventory_on_delete(world, ctx, &record);
            }
        }
        return Err(BuildingAuthoringError::Occupancy(error));
    }

    let _ = crate::world::sync_construction_tasks(world, catalog, 0);

    Ok(record)
}

/// Move an existing building to a new world position, including cross-chunk moves.
pub fn move_building(
    world: &mut WorldData,
    id: BuildingId,
    new_position: WorldPosition,
    occupancy: Option<OccupancyCatalogs<'_>>,
) -> Result<BuildingRecord, BuildingAuthoringError> {
    let moved = world
        .relocate_building(id, new_position)
        .map_err(|error| match error {
            BuildingInsertError::ChunkPlacementMismatch => {
                BuildingAuthoringError::ChunkPlacementMismatch
            }
            BuildingInsertError::BuildingNotFound => BuildingAuthoringError::BuildingNotFound(id),
        })?;

    if let Some(catalogs) = occupancy {
        update_building_occupancy(world, catalogs, &moved)
            .map_err(BuildingAuthoringError::Occupancy)?;
    }

    Ok(moved)
}

/// Remove a building by id, returning the removed record.
pub fn remove_building(
    world: &mut WorldData,
    id: BuildingId,
    occupancy: Option<OccupancyCatalogs<'_>>,
    building_catalog: Option<&BuildingCatalog>,
    doodad_catalog: Option<&DoodadCatalog>,
    interaction_catalog: Option<&BuildingInteractionProfileCatalog>,
    inventory_cleanup: Option<(
        &BuildingInventoryCleanup<'_>,
        BuildingInventoryRemovalPolicy,
    )>,
) -> Result<BuildingRecord, BuildingAuthoringError> {
    let record = world
        .get_building(id)
        .cloned()
        .ok_or(BuildingAuthoringError::BuildingNotFound(id))?;

    if let (Some(catalog), Some(interaction), Some((cleanup, policy))) =
        (building_catalog, interaction_catalog, inventory_cleanup)
    {
        if record.inventory_id.is_some()
            || world
                .building_inventory_binding_store()
                .get(id)
                .is_some_and(|set| !set.is_empty())
        {
            finalize_building_inventory_removal(
                world,
                catalog,
                interaction,
                Some(cleanup),
                &record,
                policy,
            )
            .map_err(BuildingAuthoringError::Inventory)?;
        }
    }

    if let (Some(building_catalog), Some(doodad_catalog)) = (building_catalog, doodad_catalog) {
        let _ =
            deactivate_building_interior(world, doodad_catalog, building_catalog, occupancy, id);
    }
    if occupancy.is_some() {
        unregister_source_occupancy(world, OccupancySource::Building(id));
    }
    world.building_production_store_mut().remove(id);
    world.building_inventory_binding_store_mut().remove(id);
    world
        .remove_building_by_id(id)
        .ok_or(BuildingAuthoringError::BuildingNotFound(id))
}

/// Borrow a building record by id.
pub fn lookup_building(world: &WorldData, id: BuildingId) -> Option<&BuildingRecord> {
    world.get_building(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCatalog, BuildingLifecycleState, BuildingOwnership, ChunkCoord, ChunkLayout,
        DoodadCatalog, FootprintCatalog, InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog,
        ItemCategoryCatalog, LocalPosition, OccupancyCatalogs, is_building_operational,
    };

    fn layout_world() -> WorldData {
        WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        })
    }

    fn catalog() -> BuildingCatalog {
        BuildingCatalog::default()
    }

    fn inventory_ctx() -> InventoryCatalogCtx<'static> {
        let categories = ItemCategoryCatalog::from_definitions(
            crate::world::starter_item_category_definitions(),
        )
        .unwrap();
        let items =
            ItemCatalog::from_definitions(crate::world::starter_item_definitions(), &categories)
                .unwrap();
        let profiles = InventoryProfileCatalog::from_definitions(
            crate::world::starter_inventory_profile_definitions(),
        )
        .unwrap();
        let items = Box::leak(Box::new(items));
        let categories = Box::leak(Box::new(categories));
        let profiles = Box::leak(Box::new(profiles));
        InventoryCatalogCtx::new(items, categories, profiles)
    }

    fn position(chunk_x: i32, chunk_z: i32, local: Vec3) -> WorldPosition {
        WorldPosition::new(ChunkCoord::new(chunk_x, chunk_z), LocalPosition::new(local))
    }

    #[test]
    fn create_building_from_definition() {
        let cat = catalog();
        let mut world = layout_world();
        let def = BuildingDefinitionId::new("hut");
        let pos = position(1, 2, Vec3::new(64.0, 0.0, 128.0));

        let record = create_building(
            &cat,
            &mut world,
            &def,
            pos,
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::neutral(),
            None,
        )
        .unwrap();

        assert_eq!(record.definition_id, def);
        assert_eq!(record.placement.position, pos);
        assert_eq!(record.vitals.current_hp, 250);
        assert_eq!(record.vitals.max_hp, 250);
        assert_eq!(lookup_building(&world, record.id).unwrap().id, record.id);
        world.assert_building_index_consistent();
    }

    #[test]
    fn definition_lookup_failure() {
        let cat = catalog();
        let mut world = layout_world();
        let missing = BuildingDefinitionId::new("missing");

        let err = create_building(
            &cat,
            &mut world,
            &missing,
            position(0, 0, Vec3::ZERO),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::neutral(),
            None,
        )
        .unwrap_err();

        assert_eq!(err, BuildingAuthoringError::DefinitionNotFound(missing));
    }

    #[test]
    fn move_across_chunk_boundary() {
        let cat = catalog();
        let mut world = layout_world();
        let record = create_building(
            &cat,
            &mut world,
            &BuildingDefinitionId::new("hut"),
            position(0, 0, Vec3::new(200.0, 0.0, 200.0)),
            Quat::IDENTITY,
            BuildingSource::Dev,
            BuildingOwnership::with_affiliation(crate::world::Affiliation::Dev),
            None,
        )
        .unwrap();

        let new_pos = position(1, 0, Vec3::new(64.0, 0.0, 64.0));
        let moved = move_building(&mut world, record.id, new_pos, None).unwrap();

        assert_eq!(moved.placement.position, new_pos);
        assert_eq!(
            world.building_chunk(record.id),
            Some(crate::world::ChunkId::new(ChunkCoord::new(1, 0)))
        );
        assert_eq!(moved.id, record.id);
        world.assert_building_index_consistent();
    }

    #[test]
    fn place_player_building_is_planned_with_occupancy() {
        let cat = catalog();
        let footprint = FootprintCatalog::default();
        let doodad = DoodadCatalog::default();
        let mut world = layout_world();
        let occ = OccupancyCatalogs {
            doodad: &doodad,
            building: &cat,
            footprint: &footprint,
        };
        let record = place_player_building(
            &cat,
            &mut world,
            &BuildingDefinitionId::new("hut"),
            position(0, 0, Vec3::new(64.0, 0.0, 64.0)),
            Quat::IDENTITY,
            BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
            occ,
        )
        .unwrap();
        assert_eq!(record.lifecycle_state, BuildingLifecycleState::Planned);
        assert!(world.occupancy_cell_count() > 0);
    }

    #[test]
    fn remove_building_by_authoring_id() {
        let cat = catalog();
        let mut world = layout_world();
        let record = create_building(
            &cat,
            &mut world,
            &BuildingDefinitionId::new("smelter"),
            position(2, 3, Vec3::new(128.0, 0.0, 128.0)),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::neutral(),
            None,
        )
        .unwrap();

        let removed = remove_building(&mut world, record.id, None, None, None, None, None).unwrap();
        assert_eq!(removed.id, record.id);
        assert!(lookup_building(&world, record.id).is_none());
        world.assert_building_index_consistent();
    }

    #[test]
    fn chest_requires_inventory_ctx() {
        let cat = catalog();
        let mut world = layout_world();
        let err = create_building(
            &cat,
            &mut world,
            &BuildingDefinitionId::new("storage_chest"),
            position(0, 0, Vec3::new(64.0, 0.0, 64.0)),
            Quat::IDENTITY,
            BuildingSource::Dev,
            BuildingOwnership::neutral(),
            None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            BuildingAuthoringError::InventoryAllocationFailed(_)
        ));
    }

    #[test]
    fn chest_allocates_inventory_with_ctx() {
        let cat = catalog();
        let mut world = layout_world();
        let ctx = inventory_ctx();
        let record = create_building_with_inventory(
            &cat,
            &mut world,
            &BuildingDefinitionId::new("storage_chest"),
            position(0, 0, Vec3::new(64.0, 0.0, 64.0)),
            Quat::IDENTITY,
            BuildingSource::Dev,
            BuildingOwnership::neutral(),
            None,
            &ctx,
        )
        .unwrap();
        assert!(record.inventory_id.is_some());
    }

    #[test]
    fn dev_complete_building_has_full_hp_and_progress() {
        let cat = catalog();
        let mut world = layout_world();
        let record = create_dev_complete_building(
            &cat,
            &mut world,
            &BuildingDefinitionId::new("hut"),
            position(0, 0, Vec3::new(64.0, 0.0, 64.0)),
            Quat::IDENTITY,
            BuildingOwnership::with_affiliation(crate::world::Affiliation::Dev),
            None,
        )
        .unwrap();
        assert_eq!(record.lifecycle_state, BuildingLifecycleState::Complete);
        assert_eq!(record.construction.progress_0_1, 1.0);
        assert_eq!(record.vitals.current_hp, record.vitals.max_hp);
        assert!(is_building_operational(&record));
    }
}
