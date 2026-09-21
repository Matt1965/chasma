//! Scoped inventory subgraph snapshots for templates (building archetypes, etc.).

use std::collections::{BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::{
    InventoryEntryContents, InventoryId, InventoryRecord, ItemInstanceId, PlacedInventoryEntry,
};
use crate::world::WorldData;

/// One placed grid entry inside a template inventory subgraph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventorySubgraphPlacedEntry {
    pub anchor_x: u8,
    pub anchor_y: u8,
    pub entry_kind: String,
    #[serde(default)]
    pub item_definition_id: Option<String>,
    #[serde(default)]
    pub item_instance_local_id: Option<u32>,
    #[serde(default)]
    pub quantity: Option<u32>,
}

/// One inventory container inside a template subgraph (local IDs only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventorySubgraphInventory {
    pub local_id: u32,
    pub profile_id: String,
    pub grid_width: u8,
    pub grid_height: u8,
    pub entries: Vec<InventorySubgraphPlacedEntry>,
}

/// One unique item instance inside a template subgraph (local IDs only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventorySubgraphItemInstance {
    pub local_id: u32,
    pub definition_id: String,
    #[serde(default)]
    pub quality: Option<u32>,
    #[serde(default)]
    pub contained_inventory_local_id: Option<u32>,
}

/// Placement of a unique item instance inside a template inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventorySubgraphItemInstanceLocation {
    pub instance_local_id: u32,
    pub inventory_local_id: u32,
    pub entry_index: usize,
}

/// Scoped inventory graph using template-local IDs only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventorySubgraphSnapshot {
    pub root_inventory_local_id: u32,
    pub inventories: Vec<InventorySubgraphInventory>,
    pub item_instances: Vec<InventorySubgraphItemInstance>,
    pub item_instance_locations: Vec<InventorySubgraphItemInstanceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventorySubgraphError {
    RootInventoryMissing,
    DuplicateLocalInventoryId(u32),
    DuplicateLocalInstanceId(u32),
    MissingInventoryLocalId(u32),
    MissingInstanceLocalId(u32),
    MissingItemDefinition(String),
    OrphanedInstanceLocation(u32),
    InvalidContainedInventoryReference(u32),
}

/// Capture one inventory graph reachable from `root`, remapping runtime IDs to local IDs.
pub fn capture_inventory_subgraph(
    world: &WorldData,
    root: InventoryId,
) -> Option<InventorySubgraphSnapshot> {
    if world.inventory_store().get(root).is_none() {
        return None;
    }

    let inventory_ids = collect_reachable_inventory_ids(world, root);
    if inventory_ids.is_empty() {
        return None;
    }

    let inventory_local_ids = assign_local_ids(inventory_ids.iter().map(|id| id.raw()));
    let instance_ids = collect_referenced_instance_ids(world, &inventory_ids);
    let instance_local_ids = assign_local_ids(instance_ids.iter().map(|id| id.raw()));

    let inventories = inventory_ids
        .iter()
        .filter_map(|inventory_id| {
            let record = world.inventory_store().get(*inventory_id)?;
            let local_id = inventory_local_ids
                .get(&inventory_id.raw())
                .copied()
                .expect("local inventory id");
            Some(InventorySubgraphInventory {
                local_id,
                profile_id: record.profile_id().as_str().to_string(),
                grid_width: record.grid_width(),
                grid_height: record.grid_height(),
                entries: record
                    .placed_entries()
                    .iter()
                    .map(|entry| placed_entry_to_snapshot(entry, &instance_local_ids))
                    .collect(),
            })
        })
        .collect();

    let item_instances = instance_ids
        .iter()
        .filter_map(|instance_id| {
            let instance = world.item_instance_store().get(*instance_id)?;
            let local_id = instance_local_ids
                .get(&instance_id.raw())
                .copied()
                .expect("local instance id");
            Some(InventorySubgraphItemInstance {
                local_id,
                definition_id: instance.definition_id.as_str().to_string(),
                quality: instance.metadata.quality,
                contained_inventory_local_id: instance
                    .contained_inventory_id
                    .and_then(|id| inventory_local_ids.get(&id.raw()).copied()),
            })
        })
        .collect();

    let item_instance_locations = instance_ids
        .iter()
        .filter_map(|instance_id| {
            let location = world.item_instance_store().location(*instance_id)?;
            match location {
                super::ItemInstanceLocation::Inventory {
                    inventory_id,
                    entry_index,
                } => Some(InventorySubgraphItemInstanceLocation {
                    instance_local_id: instance_local_ids
                        .get(&instance_id.raw())
                        .copied()
                        .expect("local instance id"),
                    inventory_local_id: inventory_local_ids
                        .get(&inventory_id.raw())
                        .copied()
                        .expect("local inventory id"),
                    entry_index,
                }),
                _ => None,
            }
        })
        .collect();

    Some(InventorySubgraphSnapshot {
        root_inventory_local_id: inventory_local_ids
            .get(&root.raw())
            .copied()
            .expect("root local inventory id"),
        inventories,
        item_instances,
        item_instance_locations,
    })
}

pub fn validate_inventory_subgraph(
    snapshot: &InventorySubgraphSnapshot,
    item_definition_exists: impl Fn(&str) -> bool,
) -> Result<(), InventorySubgraphError> {
    let mut inventory_ids = HashSet::new();
    for inventory in &snapshot.inventories {
        if !inventory_ids.insert(inventory.local_id) {
            return Err(InventorySubgraphError::DuplicateLocalInventoryId(inventory.local_id));
        }
        for entry in &inventory.entries {
            if entry.entry_kind == "stack" {
                let item_id = entry
                    .item_definition_id
                    .as_deref()
                    .ok_or(InventorySubgraphError::MissingItemDefinition(String::new()))?;
                if !item_definition_exists(item_id) {
                    return Err(InventorySubgraphError::MissingItemDefinition(item_id.to_string()));
                }
            } else if entry.entry_kind == "unique" {
                entry
                    .item_instance_local_id
                    .ok_or(InventorySubgraphError::MissingInstanceLocalId(0))?;
            }
        }
    }

    if !inventory_ids.contains(&snapshot.root_inventory_local_id) {
        return Err(InventorySubgraphError::MissingInventoryLocalId(
            snapshot.root_inventory_local_id,
        ));
    }

    let mut instance_ids = HashSet::new();
    for instance in &snapshot.item_instances {
        if !instance_ids.insert(instance.local_id) {
            return Err(InventorySubgraphError::DuplicateLocalInstanceId(instance.local_id));
        }
        if !item_definition_exists(&instance.definition_id) {
            return Err(InventorySubgraphError::MissingItemDefinition(
                instance.definition_id.clone(),
            ));
        }
        if let Some(contained) = instance.contained_inventory_local_id {
            if !inventory_ids.contains(&contained) {
                return Err(InventorySubgraphError::InvalidContainedInventoryReference(contained));
            }
        }
    }

    for location in &snapshot.item_instance_locations {
        if !instance_ids.contains(&location.instance_local_id) {
            return Err(InventorySubgraphError::OrphanedInstanceLocation(
                location.instance_local_id,
            ));
        }
        if !inventory_ids.contains(&location.inventory_local_id) {
            return Err(InventorySubgraphError::MissingInventoryLocalId(
                location.inventory_local_id,
            ));
        }
        let inventory = snapshot
            .inventories
            .iter()
            .find(|record| record.local_id == location.inventory_local_id)
            .ok_or(InventorySubgraphError::MissingInventoryLocalId(
                location.inventory_local_id,
            ))?;
        if location.entry_index >= inventory.entries.len() {
            return Err(InventorySubgraphError::OrphanedInstanceLocation(
                location.instance_local_id,
            ));
        }
        let entry = &inventory.entries[location.entry_index];
        if entry.item_instance_local_id != Some(location.instance_local_id) {
            return Err(InventorySubgraphError::OrphanedInstanceLocation(
                location.instance_local_id,
            ));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventorySubgraphRestoreError {
    Validation(InventorySubgraphError),
    MissingInventoryLocalId(u32),
    MissingInstanceLocalId(u32),
    MissingItemDefinition(String),
    ProfileNotFound(String),
    Inventory(crate::world::inventory::InventoryError),
    OwnerResolutionFailed(u32),
}

pub struct RestoreInventorySubgraphOptions {
    pub root_owner: super::InventoryOwnerRef,
    pub existing_root_inventory_id: Option<InventoryId>,
}

pub struct RestoredInventorySubgraph {
    pub root_inventory_id: InventoryId,
}

/// Reconstruct a template inventory graph with fresh runtime IDs.
pub fn restore_inventory_subgraph(
    world: &mut WorldData,
    ctx: &super::InventoryCatalogCtx<'_>,
    snapshot: &InventorySubgraphSnapshot,
    options: RestoreInventorySubgraphOptions,
) -> Result<RestoredInventorySubgraph, InventorySubgraphRestoreError> {
    validate_inventory_subgraph(snapshot, |item_id| {
        ctx.require_item(&crate::world::ItemDefinitionId::new(item_id))
            .is_ok()
    })
    .map_err(InventorySubgraphRestoreError::Validation)?;

    let mut inventory_map: HashMap<u32, InventoryId> = HashMap::new();
    for inventory in &snapshot.inventories {
        let runtime_id = if inventory.local_id == snapshot.root_inventory_local_id {
            if let Some(existing) = options.existing_root_inventory_id {
                let record = world
                    .inventory_store()
                    .get(existing)
                    .ok_or(InventorySubgraphRestoreError::Inventory(
                        super::InventoryError::InventoryNotFound(existing),
                    ))?;
                if record.profile_id().as_str() != inventory.profile_id {
                    return Err(InventorySubgraphRestoreError::ProfileNotFound(
                        inventory.profile_id.clone(),
                    ));
                }
                existing
            } else {
                world.inventory_store_mut().allocate_inventory_id()
            }
        } else {
            world.inventory_store_mut().allocate_inventory_id()
        };
        inventory_map.insert(inventory.local_id, runtime_id);
    }

    let mut instance_map: HashMap<u32, ItemInstanceId> = HashMap::new();
    for instance in &snapshot.item_instances {
        ctx.require_item(&crate::world::ItemDefinitionId::new(&instance.definition_id))
            .map_err(|_| {
                InventorySubgraphRestoreError::MissingItemDefinition(instance.definition_id.clone())
            })?;
        let id = world.item_instance_store_mut().allocate_item_instance_id();
        let metadata = super::ItemInstanceMetadata {
            quality: instance.quality,
        };
        let item = super::ItemInstance::new(
            id,
            crate::world::ItemDefinitionId::new(&instance.definition_id),
        )
        .with_metadata(metadata);
        world
            .item_instance_store_mut()
            .insert(item)
            .map_err(InventorySubgraphRestoreError::Inventory)?;
        instance_map.insert(instance.local_id, id);
    }

    for inventory in &snapshot.inventories {
        let runtime_id = inventory_map
            .get(&inventory.local_id)
            .copied()
            .ok_or(InventorySubgraphRestoreError::MissingInventoryLocalId(
                inventory.local_id,
            ))?;
        let owner = if inventory.local_id == snapshot.root_inventory_local_id {
            options.root_owner.clone()
        } else {
            let parent_instance_local = snapshot
                .item_instances
                .iter()
                .find(|instance| instance.contained_inventory_local_id == Some(inventory.local_id))
                .map(|instance| instance.local_id)
                .ok_or(InventorySubgraphRestoreError::OwnerResolutionFailed(
                    inventory.local_id,
                ))?;
            let parent_instance = instance_map
                .get(&parent_instance_local)
                .copied()
                .ok_or(InventorySubgraphRestoreError::MissingInstanceLocalId(
                    parent_instance_local,
                ))?;
            super::InventoryOwnerRef::ItemContainer(parent_instance)
        };

        if options.existing_root_inventory_id == Some(runtime_id) {
            let record = world
                .inventory_store_mut()
                .get_mut(runtime_id)
                .ok_or(InventorySubgraphRestoreError::Inventory(
                    super::InventoryError::InventoryNotFound(runtime_id),
                ))?;
            record.set_owner(owner);
            record.placed_entries_mut().clear();
        } else {
            let record = InventoryRecord::new(
                runtime_id,
                owner,
                super::InventoryProfileId::new(&inventory.profile_id),
                inventory.grid_width,
                inventory.grid_height,
            );
            world
                .inventory_store_mut()
                .insert(record)
                .map_err(InventorySubgraphRestoreError::Inventory)?;
        }

        let mut entries = Vec::new();
        for entry in &inventory.entries {
            let placed = match entry.entry_kind.as_str() {
                "stack" => {
                    let item_id = entry
                        .item_definition_id
                        .as_deref()
                        .ok_or(InventorySubgraphRestoreError::MissingItemDefinition(
                            String::new(),
                        ))?;
                    PlacedInventoryEntry::stack(
                        entry.anchor_x,
                        entry.anchor_y,
                        crate::world::ItemDefinitionId::new(item_id),
                        entry.quantity.unwrap_or(0),
                    )
                }
                "unique" => {
                    let local_id = entry
                        .item_instance_local_id
                        .ok_or(InventorySubgraphRestoreError::MissingInstanceLocalId(0))?;
                    let runtime_instance = instance_map
                        .get(&local_id)
                        .copied()
                        .ok_or(InventorySubgraphRestoreError::MissingInstanceLocalId(local_id))?;
                    PlacedInventoryEntry::unique(
                        entry.anchor_x,
                        entry.anchor_y,
                        runtime_instance,
                    )
                }
                _ => continue,
            };
            entries.push(placed);
        }
        world
            .inventory_store_mut()
            .get_mut(runtime_id)
            .ok_or(InventorySubgraphRestoreError::MissingInventoryLocalId(
                inventory.local_id,
            ))?
            .placed_entries_mut()
            .extend(entries);
    }

    for instance in &snapshot.item_instances {
        let runtime_id = instance_map
            .get(&instance.local_id)
            .copied()
            .ok_or(InventorySubgraphRestoreError::MissingInstanceLocalId(instance.local_id))?;
        if let Some(contained_local) = instance.contained_inventory_local_id {
            let contained_runtime = inventory_map
                .get(&contained_local)
                .copied()
                .ok_or(InventorySubgraphRestoreError::MissingInventoryLocalId(contained_local))?;
            world
                .item_instance_store_mut()
                .get_mut(runtime_id)
                .expect("instance")
                .contained_inventory_id = Some(contained_runtime);
        }
    }

    for location in &snapshot.item_instance_locations {
        let runtime_instance = instance_map
            .get(&location.instance_local_id)
            .copied()
            .ok_or(InventorySubgraphRestoreError::MissingInstanceLocalId(
                location.instance_local_id,
            ))?;
        let runtime_inventory = inventory_map
            .get(&location.inventory_local_id)
            .copied()
            .ok_or(InventorySubgraphRestoreError::MissingInventoryLocalId(
                location.inventory_local_id,
            ))?;
        world
            .item_instance_store_mut()
            .set_inventory_location(runtime_instance, runtime_inventory, location.entry_index);
    }

    let root_inventory_id = inventory_map
        .get(&snapshot.root_inventory_local_id)
        .copied()
        .ok_or(InventorySubgraphRestoreError::MissingInventoryLocalId(
            snapshot.root_inventory_local_id,
        ))?;

    crate::world::rebuild_all_inventory_derived(world, ctx)
        .map_err(InventorySubgraphRestoreError::Inventory)?;

    Ok(RestoredInventorySubgraph {
        root_inventory_id,
    })
}

pub fn inventory_subgraph_item_count(snapshot: &InventorySubgraphSnapshot) -> usize {
    snapshot
        .inventories
        .iter()
        .flat_map(|inventory| inventory.entries.iter())
        .count()
}

fn collect_reachable_inventory_ids(world: &WorldData, root: InventoryId) -> Vec<InventoryId> {
    let mut discovered = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(inventory_id) = pending.pop() {
        if !discovered.insert(inventory_id) {
            continue;
        }
        let record = world
            .inventory_store()
            .get(inventory_id)
            .expect("reachable inventory");
        for entry in record.placed_entries() {
            if let InventoryEntryContents::Unique { item_instance_id } = &entry.contents {
                if let Some(instance) = world.item_instance_store().get(*item_instance_id) {
                    if let Some(nested) = instance.contained_inventory_id {
                        pending.push(nested);
                    }
                }
            }
        }
    }

    discovered.into_iter().collect()
}

fn collect_referenced_instance_ids(
    world: &WorldData,
    inventory_ids: &[InventoryId],
) -> Vec<ItemInstanceId> {
    let mut instances = BTreeSet::new();
    for inventory_id in inventory_ids {
        let record = world
            .inventory_store()
            .get(*inventory_id)
            .expect("inventory in subgraph");
        for entry in record.placed_entries() {
            if let InventoryEntryContents::Unique { item_instance_id } = &entry.contents {
                instances.insert(*item_instance_id);
            }
        }
    }
    instances.into_iter().collect()
}

fn assign_local_ids(runtime_ids: impl IntoIterator<Item = u32>) -> HashMap<u32, u32> {
    let mut sorted = runtime_ids.into_iter().collect::<BTreeSet<_>>();
    sorted
        .into_iter()
        .enumerate()
        .map(|(index, runtime_id)| (runtime_id, index as u32 + 1))
        .collect()
}

fn placed_entry_to_snapshot(
    entry: &PlacedInventoryEntry,
    instance_local_ids: &HashMap<u32, u32>,
) -> InventorySubgraphPlacedEntry {
    match &entry.contents {
        InventoryEntryContents::Stack {
            item_definition_id,
            quantity,
        } => InventorySubgraphPlacedEntry {
            anchor_x: entry.anchor_x,
            anchor_y: entry.anchor_y,
            entry_kind: "stack".into(),
            item_definition_id: Some(item_definition_id.as_str().to_string()),
            item_instance_local_id: None,
            quantity: Some(*quantity),
        },
        InventoryEntryContents::Unique { item_instance_id } => InventorySubgraphPlacedEntry {
            anchor_x: entry.anchor_x,
            anchor_y: entry.anchor_y,
            entry_kind: "unique".into(),
            item_definition_id: None,
            item_instance_local_id: Some(
                instance_local_ids
                    .get(&item_instance_id.raw())
                    .copied()
                    .expect("local instance id"),
            ),
            quantity: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::inventory::{
        InventoryCatalogCtx, InventoryOwnerRef, ItemInstanceMetadata, create_item_instance,
        place_stack_first_fit, place_unique_first_fit,
    };
    use crate::world::{
        Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingLifecycleState,
        BuildingOwnership, BuildingSource, ChunkCoord, ChunkExtent, ItemCatalog,
        ItemCategoryCatalog, LocalPosition, WorldData, WorldPosition, create_building_with_inventory,
        starter_building_definitions, starter_inventory_profile_definitions,
        starter_item_category_definitions, starter_item_definitions,
        test_equipment_fixture_definitions,
    };
    use bevy::prelude::{Quat, Vec3};

    fn flat_world() -> WorldData {
        let layout = crate::world::WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        });
        world
    }

    fn test_inventory_ctx() -> &'static InventoryCatalogCtx<'static> {
        static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
        CTX.get_or_init(|| {
            let categories = ItemCategoryCatalog::from_definitions(starter_item_category_definitions())
                .unwrap();
            let mut items = starter_item_definitions();
            items.extend(test_equipment_fixture_definitions());
            let items = ItemCatalog::from_definitions(items, &categories).unwrap();
            let profiles = crate::world::InventoryProfileCatalog::from_definitions(
                starter_inventory_profile_definitions(),
            )
            .unwrap();
            let items = Box::leak(Box::new(items));
            let categories = Box::leak(Box::new(categories));
            let profiles = Box::leak(Box::new(profiles));
            InventoryCatalogCtx::new(items, categories, profiles)
        })
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn spawn_chest(world: &mut WorldData) -> crate::world::InventoryId {
        let categories = BuildingCategoryCatalog::default();
        let catalog =
            crate::world::BuildingCatalog::from_definitions(starter_building_definitions(), &categories)
                .unwrap();
        let created = create_building_with_inventory(
            &catalog,
            world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(10.0, 10.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap();
        world.mutate_building(created.id, |record| {
            record.lifecycle_state = BuildingLifecycleState::Complete;
        });
        created.inventory_id.expect("chest inventory")
    }

    #[test]
    fn empty_container_captures_root_inventory_only() {
        let world = flat_world();
        let mut world = world;
        let inventory_id = spawn_chest(&mut world);
        let snapshot = capture_inventory_subgraph(&world, inventory_id).expect("snapshot");
        assert_eq!(snapshot.inventories.len(), 1);
        assert!(snapshot.item_instances.is_empty());
    }

    #[test]
    fn simple_stack_and_nested_container_capture() {
        let mut world = flat_world();
        let inventory_id = spawn_chest(&mut world);
        let ctx = test_inventory_ctx();
        {
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            place_stack_first_fit(
                inventory_store,
                instance_store,
                ctx,
                inventory_id,
                crate::world::ItemDefinitionId::new("iron_ore"),
                20,
            )
            .unwrap();
            let backpack = create_item_instance(
                inventory_store,
                instance_store,
                ctx,
                crate::world::ItemDefinitionId::new("leather_backpack"),
                ItemInstanceMetadata::default(),
            )
            .unwrap();
            place_unique_first_fit(inventory_store, instance_store, ctx, inventory_id, backpack)
                .unwrap();
            let internal = instance_store
                .get(backpack)
                .unwrap()
                .contained_inventory_id
                .expect("backpack internal inventory");
            place_stack_first_fit(
                inventory_store,
                instance_store,
                ctx,
                internal,
                crate::world::ItemDefinitionId::new("prispod"),
                3,
            )
            .unwrap();
        }
        let snapshot = capture_inventory_subgraph(&world, inventory_id).expect("snapshot");
        assert_eq!(snapshot.inventories.len(), 2);
        assert_eq!(snapshot.item_instances.len(), 1);
        assert_eq!(inventory_subgraph_item_count(&snapshot), 3);
        let serialized = ron::ser::to_string(&snapshot).unwrap();
        assert!(!serialized.contains("building_id"));
        assert!(!serialized.contains("InventoryId"));
    }

    #[test]
    fn restore_round_trips_nested_container() {
        let mut world = flat_world();
        let inventory_id = spawn_chest(&mut world);
        let ctx = test_inventory_ctx();
        {
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            place_stack_first_fit(
                inventory_store,
                instance_store,
                ctx,
                inventory_id,
                crate::world::ItemDefinitionId::new("iron_ore"),
                5,
            )
            .unwrap();
            let backpack = create_item_instance(
                inventory_store,
                instance_store,
                ctx,
                crate::world::ItemDefinitionId::new("leather_backpack"),
                ItemInstanceMetadata::default(),
            )
            .unwrap();
            place_unique_first_fit(inventory_store, instance_store, ctx, inventory_id, backpack)
                .unwrap();
            let internal = instance_store
                .get(backpack)
                .unwrap()
                .contained_inventory_id
                .expect("internal");
            place_stack_first_fit(
                inventory_store,
                instance_store,
                ctx,
                internal,
                crate::world::ItemDefinitionId::new("prispod"),
                2,
            )
            .unwrap();
        }
        let snapshot = capture_inventory_subgraph(&world, inventory_id).expect("snapshot");
        let restored = restore_inventory_subgraph(
            &mut world,
            ctx,
            &snapshot,
            RestoreInventorySubgraphOptions {
                root_owner: InventoryOwnerRef::Detached,
                existing_root_inventory_id: None,
            },
        )
        .expect("restore");
        assert_ne!(restored.root_inventory_id, inventory_id);
        let restored_snapshot =
            capture_inventory_subgraph(&world, restored.root_inventory_id).expect("recapture");
        assert_eq!(
            inventory_subgraph_item_count(&snapshot),
            inventory_subgraph_item_count(&restored_snapshot)
        );
        assert_eq!(snapshot.inventories.len(), restored_snapshot.inventories.len());
    }

    #[test]
    fn unrelated_inventory_not_captured() {
        let mut world = flat_world();
        let captured = spawn_chest(&mut world);
        let unrelated = spawn_chest(&mut world);
        let ctx = test_inventory_ctx();
        {
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            place_stack_first_fit(
                inventory_store,
                instance_store,
                ctx,
                captured,
                crate::world::ItemDefinitionId::new("iron_ore"),
                5,
            )
            .unwrap();
            place_stack_first_fit(
                inventory_store,
                instance_store,
                ctx,
                unrelated,
                crate::world::ItemDefinitionId::new("bread"),
                9,
            )
            .unwrap();
        }
        let snapshot = capture_inventory_subgraph(&world, captured).expect("snapshot");
        assert_eq!(inventory_subgraph_item_count(&snapshot), 1);
        let has_bread = snapshot.inventories.iter().any(|inventory| {
            inventory.entries.iter().any(|entry| {
                entry.item_definition_id.as_deref() == Some("bread")
            })
        });
        assert!(!has_bread);
    }
}
