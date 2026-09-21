//! Reconstruct a placed building archetype snapshot into ordinary world records.

use bevy::prelude::*;

use crate::world::building::BuildingLifecycleState;
use crate::world::doodad::{create_doodad, remove_doodad, DoodadAuthoringError};
use crate::world::inventory::{
    InventoryCatalogCtx, InventoryOwnerRef, RestoreInventorySubgraphOptions,
    restore_inventory_subgraph,
};
use crate::world::item_pile::{ItemPileSource, WorldItemPileRecord};
use crate::world::{
    BuildingAuthoringError, BuildingCatalog, BuildingDefinitionId, BuildingId, BuildingOwnership,
    BuildingRecord, DoodadCatalog, DoodadDefinitionId, DoodadPlacementOverrides, DoodadSource,
    FixedScale, InventoryProfileId, ItemDefinitionId, ItemInstanceMetadata, OccupancyCatalogs,
    OperationCatalog, WorldData, WorldPosition, create_dev_complete_building,
    create_dev_complete_building_with_inventory, create_inventory,
    definition_requires_inventory_allocation, place_player_building,
    place_player_building_with_inventory, remove_building, set_building_container_locked,
};

use super::building::{
    BuildingArchetypeDefinition, BuildingArchetypeDurableExtensions,
    BuildingArchetypeMember, BuildingArchetypeMemberBuildingState, BuildingArchetypeMemberKind,
    BuildingArchetypeMemberWorldItemState,
};
use super::capture_volume::{
    building_uniform_scale_from_local_pose, doodad_scale_from_local_pose,
    world_pose_from_local_pose,
};
use super::durable_capture::validate_building_archetype_definition;

#[derive(Debug, Clone, PartialEq)]
pub enum BuildingArchetypeReconstructError {
    Validation(super::durable_capture::BuildingArchetypeValidationError),
    Building(BuildingAuthoringError),
    Doodad(DoodadAuthoringError),
    Inventory(String),
    WorldItem(String),
    RootNotFound,
}

pub struct BuildingArchetypeReconstructCtx<'a> {
    pub building_catalog: &'a BuildingCatalog,
    pub doodad_catalog: &'a DoodadCatalog,
    pub item_catalog: &'a crate::world::ItemCatalog,
    pub operation_catalog: &'a OperationCatalog,
    pub interior_catalog: &'a crate::world::InteriorProfileCatalog,
    pub inventory_ctx: &'a InventoryCatalogCtx<'a>,
    pub occupancy: OccupancyCatalogs<'a>,
    pub nav_catalog: Option<&'a crate::world::BuildingNavigationBlueprintCatalog>,
    pub created_tick: u64,
}

struct ReconstructionRollback {
    member_buildings: Vec<BuildingId>,
    doodads: Vec<crate::world::DoodadId>,
    piles: Vec<crate::world::ItemPileId>,
}

impl ReconstructionRollback {
    fn new() -> Self {
        Self {
            member_buildings: Vec::new(),
            doodads: Vec::new(),
            piles: Vec::new(),
        }
    }

    fn undo(
        &self,
        world: &mut WorldData,
        ctx: &BuildingArchetypeReconstructCtx<'_>,
    ) {
        for pile_id in &self.piles {
            world.item_pile_store_mut().remove(*pile_id);
        }
        for doodad_id in &self.doodads {
            let _ = remove_doodad(world, *doodad_id, Some(ctx.occupancy));
        }
        for building_id in &self.member_buildings {
            let _ = remove_building(
                world,
                *building_id,
                Some(ctx.occupancy),
                Some(ctx.building_catalog),
                Some(ctx.doodad_catalog),
                None,
                None,
            );
        }
    }
}

/// After the root building is placed, restore durable state and spawn all captured members.
pub fn apply_building_archetype_placement(
    world: &mut WorldData,
    root_id: BuildingId,
    archetype: &BuildingArchetypeDefinition,
    ctx: &BuildingArchetypeReconstructCtx<'_>,
) -> Result<(), BuildingArchetypeReconstructError> {
    validate_building_archetype_definition(
        archetype,
        ctx.item_catalog,
        ctx.building_catalog,
        ctx.doodad_catalog,
        ctx.operation_catalog,
    )
    .map_err(BuildingArchetypeReconstructError::Validation)?;

    let root = world
        .get_building(root_id)
        .cloned()
        .ok_or(BuildingArchetypeReconstructError::RootNotFound)?;

    apply_root_durable_state(world, &root, &archetype.snapshot, ctx)?;

    let mut rollback = ReconstructionRollback::new();
    for member in &archetype.members {
        if let Err(error) = spawn_member(world, &root, member, ctx, &mut rollback) {
            rollback.undo(world, ctx);
            return Err(error);
        }
    }

    for building_id in rollback.member_buildings.iter().copied().chain([root_id]) {
        let _ = crate::world::try_activate_interior_if_complete(
            world,
            ctx.building_catalog,
            ctx.interior_catalog,
            ctx.doodad_catalog,
            ctx.occupancy,
            ctx.nav_catalog,
            building_id,
        );
    }

    Ok(())
}

fn apply_root_durable_state(
    world: &mut WorldData,
    root: &BuildingRecord,
    snapshot: &super::building::BuildingArchetypeSnapshot,
    ctx: &BuildingArchetypeReconstructCtx<'_>,
) -> Result<(), BuildingArchetypeReconstructError> {
    if snapshot.container_locked {
        let _ = set_building_container_locked(world, root.id, true);
    }
    if (snapshot.uniform_scale - 1.0).abs() > 0.001 {
        if let Ok(scale) = FixedScale::from_f32(snapshot.uniform_scale) {
            world.mutate_building(root.id, |building| {
                building.placement.uniform_scale = scale;
            });
        }
    }
    apply_durable_extensions(world, root, &snapshot.extensions, ctx)?;
    Ok(())
}

fn apply_durable_extensions(
    world: &mut WorldData,
    building: &BuildingRecord,
    extensions: &BuildingArchetypeDurableExtensions,
    ctx: &BuildingArchetypeReconstructCtx<'_>,
) -> Result<(), BuildingArchetypeReconstructError> {
    if let Some(policy) = &extensions.operation_policy {
        world
            .building_production_store_mut()
            .set_policy(building.id, policy.clone());
    }
    if let Some(policy) = &extensions.storage_policy {
        *world
            .building_storage_policy_store_mut()
            .policy_mut(building.id) = policy.clone();
    }
    if let Some(inventory) = &extensions.inventory {
        restore_building_inventory(world, building, inventory, ctx)?;
    }
    Ok(())
}

fn restore_building_inventory(
    world: &mut WorldData,
    building: &BuildingRecord,
    snapshot: &crate::world::inventory::InventorySubgraphSnapshot,
    ctx: &BuildingArchetypeReconstructCtx<'_>,
) -> Result<(), BuildingArchetypeReconstructError> {
    let current = world.get_building(building.id).cloned().ok_or(
        BuildingArchetypeReconstructError::RootNotFound,
    )?;
    if current.inventory_id.is_none() {
        let root_profile = snapshot
            .inventories
            .iter()
            .find(|inventory| inventory.local_id == snapshot.root_inventory_local_id)
            .ok_or_else(|| {
                BuildingArchetypeReconstructError::Inventory("missing root inventory".into())
            })?;
        let inventory_id = create_inventory(
            world.inventory_store_mut(),
            ctx.inventory_ctx,
            InventoryProfileId::new(&root_profile.profile_id),
            InventoryOwnerRef::Building(building.id),
        )
        .map_err(|err| BuildingArchetypeReconstructError::Inventory(format!("{err:?}")))?;
        world.mutate_building(building.id, |stored| {
            stored.inventory_id = Some(inventory_id);
        });
    }
    let restored = restore_inventory_subgraph(
        world,
        ctx.inventory_ctx,
        snapshot,
        RestoreInventorySubgraphOptions {
            root_owner: InventoryOwnerRef::Building(building.id),
            existing_root_inventory_id: current.inventory_id,
        },
    )
    .map_err(|err| BuildingArchetypeReconstructError::Inventory(format!("{err:?}")))?;

    if current.inventory_id.is_none() {
        world.mutate_building(building.id, |stored| {
            stored.inventory_id = Some(restored.root_inventory_id);
        });
    }

    Ok(())
}

fn spawn_member(
    world: &mut WorldData,
    root: &BuildingRecord,
    member: &BuildingArchetypeMember,
    ctx: &BuildingArchetypeReconstructCtx<'_>,
    rollback: &mut ReconstructionRollback,
) -> Result<(), BuildingArchetypeReconstructError> {
    let layout = world.layout();
    let (position, rotation) = world_pose_from_local_pose(layout, root, &member.local_pose);
    match member.kind {
        BuildingArchetypeMemberKind::Building => {
            let record = spawn_member_building(world, root, member, position, rotation, ctx)?;
            rollback.member_buildings.push(record.id);
        }
        BuildingArchetypeMemberKind::Doodad => {
            let scale = doodad_scale_from_local_pose(&member.local_pose);
            let record = create_doodad(
                ctx.doodad_catalog,
                world,
                &DoodadDefinitionId::new(&member.definition_id),
                position,
                DoodadSource::Dev,
                DoodadPlacementOverrides {
                    rotation: Some(rotation),
                    scale,
                },
                Some(ctx.occupancy),
            )
            .map_err(BuildingArchetypeReconstructError::Doodad)?;
            rollback.doodads.push(record.id);
        }
        BuildingArchetypeMemberKind::WorldItemPile => {
            let pile_id = spawn_world_item_pile(world, member, position, ctx)?;
            rollback.piles.push(pile_id);
        }
    }
    Ok(())
}

fn spawn_member_building(
    world: &mut WorldData,
    root: &BuildingRecord,
    member: &BuildingArchetypeMember,
    position: WorldPosition,
    rotation: Quat,
    ctx: &BuildingArchetypeReconstructCtx<'_>,
) -> Result<BuildingRecord, BuildingArchetypeReconstructError> {
    let definition_id = BuildingDefinitionId::new(&member.definition_id);
    let state = member.building_state.as_ref();
    let ownership = member_building_ownership(root, state);
    let lifecycle = state
        .map(|state| state.lifecycle_state)
        .unwrap_or(BuildingLifecycleState::Complete);
    let needs_inventory = ctx
        .building_catalog
        .get(&definition_id)
        .is_some_and(definition_requires_inventory_allocation);

    let record = if lifecycle == BuildingLifecycleState::Planned {
        if needs_inventory {
            place_player_building_with_inventory(
                ctx.building_catalog,
                world,
                &definition_id,
                position,
                rotation,
                ownership,
                ctx.occupancy,
                ctx.inventory_ctx,
            )
        } else {
            place_player_building(
                ctx.building_catalog,
                world,
                &definition_id,
                position,
                rotation,
                ownership,
                ctx.occupancy,
            )
        }
    } else if needs_inventory {
        create_dev_complete_building_with_inventory(
            ctx.building_catalog,
            world,
            &definition_id,
            position,
            rotation,
            ownership,
            Some(ctx.occupancy),
            ctx.inventory_ctx,
        )
    } else {
        create_dev_complete_building(
            ctx.building_catalog,
            world,
            &definition_id,
            position,
            rotation,
            ownership,
            Some(ctx.occupancy),
        )
    }
    .map_err(BuildingArchetypeReconstructError::Building)?;

    if let Some(scale) = building_uniform_scale_from_local_pose(&member.local_pose) {
        if (scale - 1.0).abs() > 0.001 {
            if let Ok(fixed) = FixedScale::from_f32(scale) {
                world.mutate_building(record.id, |building| {
                    building.placement.uniform_scale = fixed;
                });
            }
        }
    }

    if let Some(state) = state {
        if state.container_locked {
            let _ = set_building_container_locked(world, record.id, true);
        }
        world.mutate_building(record.id, |building| {
            building.lifecycle_state = state.lifecycle_state;
        });
        apply_durable_extensions(world, &record, &state.extensions, ctx)?;
    }

    world
        .get_building(record.id)
        .cloned()
        .ok_or(BuildingArchetypeReconstructError::Building(
            BuildingAuthoringError::BuildingNotFound(record.id),
        ))
}

fn member_building_ownership(
    root: &BuildingRecord,
    state: Option<&BuildingArchetypeMemberBuildingState>,
) -> BuildingOwnership {
    match state {
        Some(state) => BuildingOwnership {
            owner_id: state.owner_id,
            team_id: state.team_id,
            affiliation: state.affiliation,
        },
        None => root.ownership.clone(),
    }
}

fn spawn_world_item_pile(
    world: &mut WorldData,
    member: &BuildingArchetypeMember,
    position: WorldPosition,
    ctx: &BuildingArchetypeReconstructCtx<'_>,
) -> Result<crate::world::ItemPileId, BuildingArchetypeReconstructError> {
    let state = member
        .world_item_state
        .as_ref()
        .ok_or_else(|| {
            BuildingArchetypeReconstructError::WorldItem("missing world item state".into())
        })?;
    let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
    let source = parse_pile_source(&state.source);
    let record = if let Some(quantity) = state.stack_quantity {
        WorldItemPileRecord::new_stack(
            pile_id,
            position,
            crate::world::SpaceId::SURFACE,
            ItemDefinitionId::new(&member.definition_id),
            quantity,
            state.owner_id,
            state.team_id,
            state.affiliation,
            source,
            ctx.created_tick,
        )
    } else {
        let instance_id = spawn_unique_world_item(world, member, state, ctx)?;
        world
            .item_instance_store_mut()
            .set_world_pile_location(instance_id, pile_id);
        WorldItemPileRecord::new_unique(
            pile_id,
            position,
            crate::world::SpaceId::SURFACE,
            instance_id,
            state.owner_id,
            state.team_id,
            state.affiliation,
            source,
            ctx.created_tick,
        )
    };
    let chunk = crate::world::ChunkId::new(position.chunk);
    world
        .item_pile_store_mut()
        .insert(chunk, record)
        .map_err(|err| BuildingArchetypeReconstructError::WorldItem(format!("{err:?}")))?;
    Ok(pile_id)
}

fn spawn_unique_world_item(
    world: &mut WorldData,
    member: &BuildingArchetypeMember,
    state: &BuildingArchetypeMemberWorldItemState,
    ctx: &BuildingArchetypeReconstructCtx<'_>,
) -> Result<crate::world::inventory::ItemInstanceId, BuildingArchetypeReconstructError> {
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let metadata = ItemInstanceMetadata {
        quality: state.unique_quality,
    };
    let instance_id = crate::world::inventory::create_item_instance(
        inventory_store,
        instance_store,
        ctx.inventory_ctx,
        ItemDefinitionId::new(&member.definition_id),
        metadata,
    )
    .map_err(|err| BuildingArchetypeReconstructError::Inventory(format!("{err:?}")))?;

    if let Some(inventory) = &state.unique_inventory {
        restore_inventory_subgraph(
            world,
            ctx.inventory_ctx,
            inventory,
            RestoreInventorySubgraphOptions {
                root_owner: InventoryOwnerRef::ItemContainer(instance_id),
                existing_root_inventory_id: world
                    .item_instance_store()
                    .get(instance_id)
                    .and_then(|instance| instance.contained_inventory_id),
            },
        )
        .map_err(|err| BuildingArchetypeReconstructError::Inventory(format!("{err:?}")))?;
    }

    Ok(instance_id)
}

fn parse_pile_source(label: &str) -> ItemPileSource {
    match label {
        "Spilled" => ItemPileSource::Spilled,
        "DevSpawned" => ItemPileSource::DevSpawned,
        _ => ItemPileSource::Dropped,
    }
}
