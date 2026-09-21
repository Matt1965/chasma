//! Durable authored state capture for building archetypes (Chunk 2).

use crate::world::building::operation::BuildingOperationPolicy;
use crate::world::building::storage_policy::BuildingStoragePolicy;
use crate::world::building::BuildingRecord;
use crate::world::inventory::{
    InventorySubgraphSnapshot, capture_inventory_subgraph, inventory_subgraph_item_count,
    validate_inventory_subgraph,
};
use crate::world::item_pile::{ItemPileSource, WorldItemPileRecord, WorldPileContents};
use crate::world::{
    BuildingCatalog, BuildingDefinitionId, DoodadCatalog, DoodadDefinitionId, ItemCatalog,
    OperationCatalog, WorldData,
};

use super::building::{
    BuildingArchetypeDefinition, BuildingArchetypeDurableExtensions,
    BuildingArchetypeMember, BuildingArchetypeMemberBuildingState, BuildingArchetypeMemberKind,
    BuildingArchetypeMemberWorldItemState, BuildingArchetypeSnapshot,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildingArchetypeValidationError {
    BaseBuildingNotFound(BuildingDefinitionId),
    MemberDefinitionNotFound(String),
    MissingItemDefinition(String),
    InvalidWorldItemQuantity(String),
    InvalidLocalPose(String),
    InvalidOperation(String),
    Inventory(InventorySubgraphValidationError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventorySubgraphValidationError {
    Inner(crate::world::inventory::InventorySubgraphError),
}

pub fn capture_building_durable_extensions(
    world: &WorldData,
    building: &BuildingRecord,
) -> BuildingArchetypeDurableExtensions {
    let operation_policy = world
        .building_production_store()
        .get_policy(building.id)
        .cloned();
    let storage_policy = world
        .building_storage_policy_store()
        .policy(building.id)
        .cloned();
    let inventory = building
        .inventory_id
        .and_then(|inventory_id| capture_inventory_subgraph(world, inventory_id));
    BuildingArchetypeDurableExtensions {
        operation_policy,
        storage_policy,
        inventory,
    }
}

pub fn capture_building_archetype_snapshot(
    world: &WorldData,
    building: &BuildingRecord,
) -> BuildingArchetypeSnapshot {
    let yaw_deg = building
        .placement
        .rotation
        .to_euler(bevy::prelude::EulerRot::YXZ)
        .0
        .to_degrees();
    BuildingArchetypeSnapshot {
        affiliation: building.ownership.affiliation,
        team_id: building.ownership.team_id,
        owner_id: building.ownership.owner_id,
        lifecycle_state: building.lifecycle_state,
        container_locked: building.container_locked,
        uniform_scale: building.placement.uniform_scale_f32(),
        placement_yaw_deg: yaw_deg,
        extensions: capture_building_durable_extensions(world, building),
    }
}

pub fn capture_world_item_member_state(
    world: &WorldData,
    pile: &WorldItemPileRecord,
) -> (String, BuildingArchetypeMemberWorldItemState) {
    match &pile.contents {
        WorldPileContents::Stack {
            item_definition_id,
            quantity,
        } => (
            item_definition_id.as_str().to_string(),
            BuildingArchetypeMemberWorldItemState {
                stack_quantity: Some(*quantity),
                unique_quality: None,
                unique_inventory: None,
                affiliation: pile.affiliation,
                team_id: pile.team_id,
                owner_id: pile.owner_id,
                source: pile_source_label(pile.source),
            },
        ),
        WorldPileContents::Unique { item_instance_id } => {
            let instance = world
                .item_instance_store()
                .get(*item_instance_id)
                .expect("world pile unique item");
            let unique_inventory = instance
                .contained_inventory_id
                .and_then(|inventory_id| capture_inventory_subgraph(world, inventory_id));
            (
                instance.definition_id.as_str().to_string(),
                BuildingArchetypeMemberWorldItemState {
                    stack_quantity: None,
                    unique_quality: instance.metadata.quality,
                    unique_inventory,
                    affiliation: pile.affiliation,
                    team_id: pile.team_id,
                    owner_id: pile.owner_id,
                    source: pile_source_label(pile.source),
                },
            )
        }
    }
}

pub fn capture_building_member_building_state(
    world: &WorldData,
    building: &BuildingRecord,
) -> BuildingArchetypeMemberBuildingState {
    BuildingArchetypeMemberBuildingState {
        affiliation: building.ownership.affiliation,
        team_id: building.ownership.team_id,
        owner_id: building.ownership.owner_id,
        lifecycle_state: building.lifecycle_state,
        container_locked: building.container_locked,
        extensions: capture_building_durable_extensions(world, building),
    }
}

pub fn validate_building_archetype_definition(
    definition: &BuildingArchetypeDefinition,
    item_catalog: &ItemCatalog,
    building_catalog: &BuildingCatalog,
    doodad_catalog: &DoodadCatalog,
    operation_catalog: &OperationCatalog,
) -> Result<(), BuildingArchetypeValidationError> {
    if building_catalog.get(&definition.base_building_id).is_none() {
        return Err(BuildingArchetypeValidationError::BaseBuildingNotFound(
            definition.base_building_id.clone(),
        ));
    }

    validate_durable_extensions(
        &definition.snapshot.extensions,
        item_catalog,
        operation_catalog,
    )?;

    for member in &definition.members {
        validate_member_pose(member)?;
        match member.kind {
            BuildingArchetypeMemberKind::Building => {
                if building_catalog
                    .get(&BuildingDefinitionId::new(&member.definition_id))
                    .is_none()
                {
                    return Err(BuildingArchetypeValidationError::MemberDefinitionNotFound(
                        member.definition_id.clone(),
                    ));
                }
                if let Some(state) = &member.building_state {
                    validate_durable_extensions(&state.extensions, item_catalog, operation_catalog)?;
                }
            }
            BuildingArchetypeMemberKind::Doodad => {
                if doodad_catalog
                    .get(&DoodadDefinitionId::new(&member.definition_id))
                    .is_none()
                {
                    return Err(BuildingArchetypeValidationError::MemberDefinitionNotFound(
                        member.definition_id.clone(),
                    ));
                }
            }
            BuildingArchetypeMemberKind::WorldItemPile => {
                validate_world_item_member(member, item_catalog)?;
            }
        }
    }

    Ok(())
}

fn validate_member_pose(member: &BuildingArchetypeMember) -> Result<(), BuildingArchetypeValidationError> {
    if !member.local_pose.local_position.iter().all(|value| value.is_finite()) {
        return Err(BuildingArchetypeValidationError::InvalidLocalPose(
            member.definition_id.clone(),
        ));
    }
    if !member.local_pose.local_rotation.iter().all(|value| value.is_finite()) {
        return Err(BuildingArchetypeValidationError::InvalidLocalPose(
            member.definition_id.clone(),
        ));
    }
    Ok(())
}

fn validate_world_item_member(
    member: &BuildingArchetypeMember,
    item_catalog: &ItemCatalog,
) -> Result<(), BuildingArchetypeValidationError> {
    let item_id = crate::world::ItemDefinitionId::new(&member.definition_id);
    if item_catalog.get(&item_id).is_none() {
        return Err(BuildingArchetypeValidationError::MissingItemDefinition(
            member.definition_id.clone(),
        ));
    }
    let Some(state) = &member.world_item_state else {
        return Err(BuildingArchetypeValidationError::MemberDefinitionNotFound(
            member.definition_id.clone(),
        ));
    };
    if let Some(quantity) = state.stack_quantity {
        if quantity == 0 {
            return Err(BuildingArchetypeValidationError::InvalidWorldItemQuantity(
                member.definition_id.clone(),
            ));
        }
    }
    if let Some(inventory) = &state.unique_inventory {
        validate_inventory_subgraph(inventory, |item_id| {
            item_catalog
                .get(&crate::world::ItemDefinitionId::new(item_id))
                .is_some()
        })
        .map_err(|error| {
            BuildingArchetypeValidationError::Inventory(InventorySubgraphValidationError::Inner(
                error,
            ))
        })?;
    }
    Ok(())
}

fn validate_durable_extensions(
    extensions: &BuildingArchetypeDurableExtensions,
    item_catalog: &ItemCatalog,
    operation_catalog: &OperationCatalog,
) -> Result<(), BuildingArchetypeValidationError> {
    if let Some(policy) = &extensions.operation_policy {
        if let Some(operation_id) = &policy.selected_operation {
            if operation_catalog.get(operation_id).is_none() {
                return Err(BuildingArchetypeValidationError::InvalidOperation(
                    operation_id.as_str().to_string(),
                ));
            }
        }
    }

    if let Some(inventory) = &extensions.inventory {
        validate_inventory_subgraph(inventory, |item_id| item_catalog.get(&crate::world::ItemDefinitionId::new(item_id)).is_some())
            .map_err(|error| {
                BuildingArchetypeValidationError::Inventory(InventorySubgraphValidationError::Inner(
                    error,
                ))
            })?;
    }

    Ok(())
}

pub fn world_item_member_summary(state: &BuildingArchetypeMemberWorldItemState) -> Option<String> {
    if let Some(quantity) = state.stack_quantity {
        if quantity > 1 {
            return Some(format!("x{quantity}"));
        }
    }
    if state.unique_inventory.is_some() {
        return Some("container".into());
    }
    None
}

fn pile_source_label(source: ItemPileSource) -> String {
    match source {
        ItemPileSource::Dropped => "Dropped".into(),
        ItemPileSource::Spilled => "Spilled".into(),
        ItemPileSource::DevSpawned => "DevSpawned".into(),
    }
}

pub fn durable_extensions_summary(extensions: &BuildingArchetypeDurableExtensions) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(inventory) = &extensions.inventory {
        let count = inventory_subgraph_item_count(inventory);
        if count > 0 {
            parts.push(format!("{count} items"));
        }
    }
    if extensions.operation_policy.is_some() {
        parts.push("configured".into());
    }
    if extensions.storage_policy.is_some() {
        parts.push("storage policy".into());
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}
