//! Dev inventory mutations via authoritative runtime APIs (DV0).

use crate::dev::dev_mode::DevInventoryEndpoint;
use crate::world::{
    Affiliation, ChunkId, EntryIndex, InventoryCatalogCtx, InventoryEntryContents, InventoryError,
    InventoryId, InventoryOwnerRef, InventoryProfileId, ItemDefinition, ItemDefinitionId,
    ItemInstanceMetadata, ItemInstanceStore, ItemPileId, ItemPileSettings, ItemPileSource,
    PileOwnership, PlacedInventoryEntry, SpaceId, TransferPlacementPolicy, UnitCatalog, UnitId,
    WorldData, WorldPileContents, WorldPosition, create_inventory, create_item_instance,
    create_unit_inventory, drop_stack_from_inventory, pickup_pile_into_inventory, place_stack,
    place_stack_first_fit, place_unique, place_unique_first_fit, remove_entry, transfer_entry_full,
    transfer_stack_quantity,
};

/// Non-stackable items and `unique_instance_required` items occupy inventory as unique entries.
fn item_uses_unique_inventory_entry(item: &ItemDefinition) -> bool {
    !item.stackable || item.unique_instance_required
}

/// One placement action creates a single physical item for unique-entry types.
pub fn dev_effective_placement_quantity(item: &ItemDefinition, requested: u32) -> u32 {
    if item_uses_unique_inventory_entry(item) {
        1
    } else {
        requested.max(1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevInventoryOpError {
    NoEndpoint,
    NoItemSelected,
    NoEntrySelected,
    NoTransferEndpoints,
    NoUnitSelected,
    NoContainerSelected,
    ContainerHasNoInventory,
    Message(String),
}

impl std::fmt::Display for DevInventoryOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoEndpoint => {
                write!(f, "no inventory target — inspect a unit, building, or pile")
            }
            Self::NoItemSelected => write!(f, "select an item in the catalog list first"),
            Self::NoEntrySelected => write!(f, "select an inventory entry first"),
            Self::NoTransferEndpoints => {
                write!(f, "set both transfer source and destination endpoints")
            }
            Self::NoUnitSelected => write!(f, "no unit selected — select a unit first"),
            Self::NoContainerSelected => {
                write!(f, "no container selected — select a building with storage")
            }
            Self::ContainerHasNoInventory => {
                write!(
                    f,
                    "selected building has no inventory — spawn a storage building"
                )
            }
            Self::Message(message) => write!(f, "{message}"),
        }
    }
}

impl From<InventoryError> for DevInventoryOpError {
    fn from(value: InventoryError) -> Self {
        Self::Message(value.to_string())
    }
}

/// Attach a dev backpack when the inspected unit has no inventory yet.
pub fn ensure_dev_unit_inventory(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    ctx: &InventoryCatalogCtx<'_>,
    unit_id: UnitId,
) -> Result<InventoryId, DevInventoryOpError> {
    if let Some(inventory_id) = world.get_unit(unit_id).and_then(|unit| unit.inventory_id) {
        return Ok(inventory_id);
    }

    let mut record = world
        .remove_unit_by_id(unit_id)
        .ok_or_else(|| DevInventoryOpError::Message(format!("unit #{unit_id:?} not found")))?;

    let definition = unit_catalog.get(&record.definition_id).ok_or_else(|| {
        DevInventoryOpError::Message(format!(
            "unit definition `{}` not found",
            record.definition_id.as_str()
        ))
    })?;

    let profile_id = definition
        .inventory_profile_id
        .clone()
        .unwrap_or_else(|| InventoryProfileId::new("unit_backpack_standard"));
    let inventory_id =
        create_unit_inventory(world.inventory_store_mut(), ctx, profile_id, unit_id)?;
    record.inventory_id = Some(inventory_id);

    let chunk = ChunkId::new(record.placement.position.chunk);
    world
        .insert_unit(chunk, record)
        .map_err(|err| DevInventoryOpError::Message(format!("{err:?}")))?;

    Ok(inventory_id)
}

pub fn dev_add_item(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    endpoint: DevInventoryEndpoint,
    item_id: ItemDefinitionId,
    quantity: u32,
    pile_settings: &ItemPileSettings,
    position: WorldPosition,
    tick: u64,
) -> Result<String, DevInventoryOpError> {
    if quantity == 0 {
        return Err(DevInventoryOpError::Message("quantity must be > 0".into()));
    }
    match endpoint {
        DevInventoryEndpoint::Grid(inventory_id) => {
            let item = ctx.require_item(&item_id)?;
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            if item_uses_unique_inventory_entry(item) {
                let instance_id = create_item_instance(
                    instance_store,
                    ctx,
                    item_id,
                    ItemInstanceMetadata::default(),
                )?;
                let index = place_unique_first_fit(
                    inventory_store,
                    instance_store,
                    ctx,
                    inventory_id,
                    instance_id,
                )?;
                Ok(format!("Placed unique item at entry {index}"))
            } else {
                let placement_qty = dev_effective_placement_quantity(item, quantity);
                let index = place_stack_first_fit(
                    inventory_store,
                    instance_store,
                    ctx,
                    inventory_id,
                    item_id,
                    placement_qty,
                )?;
                Ok(format!("Placed stack x{placement_qty} at entry {index}"))
            }
        }
        DevInventoryEndpoint::Pile(pile_id) => {
            if ctx.require_item(&item_id)?.unique_instance_required {
                return Err(DevInventoryOpError::Message(
                    "unique items on piles require grid inventory first — use Add on a unit/building"
                        .into(),
                ));
            }
            let pile = world
                .item_pile_store()
                .get(pile_id)
                .cloned()
                .ok_or_else(|| DevInventoryOpError::Message("pile not found".into()))?;
            if matches!(&pile.contents, WorldPileContents::Stack { item_definition_id, .. } if *item_definition_id == item_id)
            {
                if let Some(record) = world.item_pile_store_mut().get_mut(pile_id) {
                    if let WorldPileContents::Stack {
                        quantity: ref mut q,
                        ..
                    } = record.contents
                    {
                        *q = q.saturating_add(quantity);
                    }
                }
                return Ok(format!("Increased pile #{pile_id:?} by {quantity}"));
            }
            let _ = pile_id;
            let new_id = world.item_pile_store_mut().allocate_item_pile_id();
            let record = crate::world::WorldItemPileRecord::new_stack(
                new_id,
                pile.placement,
                pile.current_space_id,
                item_id,
                quantity,
                pile.owner_id,
                pile.team_id,
                pile.affiliation,
                ItemPileSource::DevSpawned,
                tick,
            );
            let chunk = ChunkId::new(pile.placement.chunk);
            world
                .item_pile_store_mut()
                .insert(chunk, record)
                .map_err(|err| DevInventoryOpError::Message(err.to_string()))?;
            Ok(format!("Spawned new pile #{new_id:?} x{quantity}"))
        }
    }
}

pub fn dev_place_item_at_anchor(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    item_id: ItemDefinitionId,
    quantity: u32,
    anchor_x: u8,
    anchor_y: u8,
) -> Result<EntryIndex, DevInventoryOpError> {
    if quantity == 0 {
        return Err(DevInventoryOpError::Message("quantity must be > 0".into()));
    }
    let item = ctx.require_item(&item_id)?;
    let placement_qty = dev_effective_placement_quantity(item, quantity);
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    if item_uses_unique_inventory_entry(item) {
        let instance_id = create_item_instance(
            instance_store,
            ctx,
            item_id,
            ItemInstanceMetadata::default(),
        )?;
        place_unique(
            inventory_store,
            instance_store,
            ctx,
            inventory_id,
            instance_id,
            anchor_x,
            anchor_y,
        )
        .map_err(DevInventoryOpError::from)
    } else {
        place_stack(
            inventory_store,
            instance_store,
            ctx,
            inventory_id,
            item_id,
            placement_qty,
            anchor_x,
            anchor_y,
        )
        .map_err(DevInventoryOpError::from)
    }
}

pub fn dev_spawn_ground_pile(
    world: &mut WorldData,
    item_id: ItemDefinitionId,
    quantity: u32,
    position: WorldPosition,
    tick: u64,
) -> Result<ItemPileId, DevInventoryOpError> {
    if quantity == 0 {
        return Err(DevInventoryOpError::Message("quantity must be > 0".into()));
    }
    let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
    let record = crate::world::WorldItemPileRecord::new_stack(
        pile_id,
        position,
        SpaceId::SURFACE,
        item_id,
        quantity,
        None,
        None,
        Affiliation::Player,
        ItemPileSource::DevSpawned,
        tick,
    );
    let chunk = ChunkId::new(position.chunk);
    world
        .item_pile_store_mut()
        .insert(chunk, record)
        .map_err(|err| DevInventoryOpError::Message(err.to_string()))?;
    Ok(pile_id)
}

pub fn dev_remove_entry(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    endpoint: DevInventoryEndpoint,
    entry_index: EntryIndex,
) -> Result<String, DevInventoryOpError> {
    match endpoint {
        DevInventoryEndpoint::Grid(inventory_id) => {
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            remove_entry(
                inventory_store,
                instance_store,
                ctx,
                inventory_id,
                entry_index,
            )?;
            Ok(format!("Removed entry {entry_index} from {inventory_id:?}"))
        }
        DevInventoryEndpoint::Pile(pile_id) => {
            world
                .item_pile_store_mut()
                .remove(pile_id)
                .ok_or_else(|| DevInventoryOpError::Message("pile not found".into()))?;
            Ok(format!("Removed pile #{pile_id:?}"))
        }
    }
}

pub fn dev_set_stack_quantity(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    endpoint: DevInventoryEndpoint,
    entry_index: EntryIndex,
    new_quantity: u32,
) -> Result<String, DevInventoryOpError> {
    if new_quantity == 0 {
        return dev_remove_entry(world, ctx, endpoint, entry_index);
    }
    match endpoint {
        DevInventoryEndpoint::Grid(inventory_id) => {
            let (item_definition_id, anchor_x, anchor_y) = {
                let record = world
                    .inventory_store()
                    .get(inventory_id)
                    .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
                let entry = record.placed_entries().get(entry_index).ok_or(
                    InventoryError::EntryNotFound {
                        inventory_id,
                        entry_index,
                    },
                )?;
                match &entry.contents {
                    InventoryEntryContents::Stack {
                        item_definition_id, ..
                    } => (item_definition_id.clone(), entry.anchor_x, entry.anchor_y),
                    _ => {
                        return Err(DevInventoryOpError::Message(
                            "cannot set quantity on unique entry".into(),
                        ));
                    }
                }
            };
            let item = ctx.require_item(&item_definition_id)?;
            let limit = {
                let record = world
                    .inventory_store()
                    .get(inventory_id)
                    .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
                ctx.stack_limit_for(item, record.profile_id())?
            };
            if new_quantity > limit {
                return Err(DevInventoryOpError::Message(format!(
                    "quantity {new_quantity} exceeds stack limit {limit}"
                )));
            }
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            remove_entry(
                inventory_store,
                instance_store,
                ctx,
                inventory_id,
                entry_index,
            )?;
            place_stack(
                inventory_store,
                instance_store,
                ctx,
                inventory_id,
                item_definition_id,
                new_quantity,
                anchor_x,
                anchor_y,
            )?;
            Ok(format!(
                "Set entry {entry_index} quantity to {new_quantity}"
            ))
        }
        DevInventoryEndpoint::Pile(pile_id) => {
            let pile = world
                .item_pile_store_mut()
                .get_mut(pile_id)
                .ok_or_else(|| DevInventoryOpError::Message("pile not found".into()))?;
            match &mut pile.contents {
                WorldPileContents::Stack { quantity, .. } => {
                    *quantity = new_quantity;
                    Ok(format!("Set pile #{pile_id:?} quantity to {new_quantity}"))
                }
                WorldPileContents::Unique { .. } => Err(DevInventoryOpError::Message(
                    "cannot set quantity on unique pile".into(),
                )),
            }
        }
    }
}

pub fn dev_clear_inventory(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    endpoint: DevInventoryEndpoint,
) -> Result<String, DevInventoryOpError> {
    match endpoint {
        DevInventoryEndpoint::Grid(inventory_id) => {
            let entry_count = world
                .inventory_store()
                .get(inventory_id)
                .map(|record| record.placed_entries().len())
                .unwrap_or(0);
            for index in (0..entry_count).rev() {
                let (inventory_store, instance_store) = world.inventory_runtime_mut();
                let _ = remove_entry(inventory_store, instance_store, ctx, inventory_id, index);
            }
            Ok(format!(
                "Cleared {entry_count} entries from {inventory_id:?}"
            ))
        }
        DevInventoryEndpoint::Pile(pile_id) => dev_remove_entry(world, ctx, endpoint, 0),
    }
}

pub fn dev_fill_inventory(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    endpoint: DevInventoryEndpoint,
    item_id: ItemDefinitionId,
    quantity_per_stack: u32,
) -> Result<String, DevInventoryOpError> {
    let item = ctx.require_item(&item_id)?;
    if item_uses_unique_inventory_entry(item) {
        return Err(DevInventoryOpError::Message(
            "fill is for stackable items only".into(),
        ));
    }
    let DevInventoryEndpoint::Grid(inventory_id) = endpoint else {
        return Err(DevInventoryOpError::Message(
            "fill only applies to grid inventories".into(),
        ));
    };
    let mut placed = 0u32;
    loop {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        match place_stack_first_fit(
            inventory_store,
            instance_store,
            ctx,
            inventory_id,
            item_id.clone(),
            quantity_per_stack,
        ) {
            Ok(_) => placed += 1,
            Err(_) => break,
        }
    }
    if placed == 0 {
        return Err(DevInventoryOpError::Message(
            "inventory full or item invalid".into(),
        ));
    }
    Ok(format!(
        "Filled {placed} stacks of `{}` x{quantity_per_stack}",
        item_id.as_str()
    ))
}

pub fn dev_transfer(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    pile_settings: &ItemPileSettings,
    source: DevInventoryEndpoint,
    destination: DevInventoryEndpoint,
    entry_index: EntryIndex,
    quantity: Option<u32>,
    tick: u64,
) -> Result<String, DevInventoryOpError> {
    match (source, destination) {
        (DevInventoryEndpoint::Grid(src), DevInventoryEndpoint::Grid(dst)) => {
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            let report = if let Some(qty) = quantity {
                transfer_stack_quantity(
                    inventory_store,
                    instance_store,
                    ctx,
                    src,
                    entry_index,
                    dst,
                    qty,
                    TransferPlacementPolicy::MergeThenFirstFit,
                    false,
                )
                .map_err(|err| DevInventoryOpError::Message(err.to_string()))?
            } else {
                transfer_entry_full(
                    inventory_store,
                    instance_store,
                    ctx,
                    src,
                    entry_index,
                    dst,
                    TransferPlacementPolicy::MergeThenFirstFit,
                )
                .map_err(|err| DevInventoryOpError::Message(err.to_string()))?
            };
            Ok(format!(
                "Transferred {} (status {:?})",
                report.moved, report.status
            ))
        }
        (DevInventoryEndpoint::Grid(src), DevInventoryEndpoint::Pile(dst)) => {
            let drop_qty = quantity.unwrap_or_else(|| {
                world
                    .inventory_store()
                    .get(src)
                    .and_then(|record| record.placed_entries().get(entry_index))
                    .and_then(|entry| match &entry.contents {
                        InventoryEntryContents::Stack { quantity, .. } => Some(*quantity),
                        _ => None,
                    })
                    .unwrap_or(1)
            });
            let pile =
                world.item_pile_store().get(dst).cloned().ok_or_else(|| {
                    DevInventoryOpError::Message("destination pile missing".into())
                })?;
            let report = drop_stack_from_inventory(
                world,
                ctx,
                pile_settings,
                src,
                entry_index,
                drop_qty,
                pile.placement,
                pile.current_space_id,
                PileOwnership {
                    owner_id: pile.owner_id,
                    team_id: pile.team_id,
                    affiliation: pile.affiliation,
                },
                tick,
            )
            .map_err(|err| DevInventoryOpError::Message(err.to_string()))?;
            Ok(format!(
                "Dropped {} to ground (merged {}, new piles {:?})",
                report.removed_from_inventory,
                report.merged_into_existing_piles,
                report.created_pile_ids
            ))
        }
        (DevInventoryEndpoint::Pile(src), DevInventoryEndpoint::Grid(dst)) => {
            let report = pickup_pile_into_inventory(
                world,
                ctx,
                src,
                dst,
                quantity,
                None,
                None,
                Affiliation::Player,
            )
            .map_err(|err| DevInventoryOpError::Message(err.to_string()))?;
            Ok(format!(
                "Picked up {} (pile removed={})",
                report.transfer.moved, report.pile_removed
            ))
        }
        (DevInventoryEndpoint::Pile(_), DevInventoryEndpoint::Pile(_)) => Err(
            DevInventoryOpError::Message("pile-to-pile transfer: pick up then drop".into()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        InventoryOwnerRef, InventoryProfileCatalog, InventoryProfileId, create_inventory,
    };

    #[test]
    fn clear_grid_inventory() {
        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let items = crate::world::ItemCatalog::default();
        let categories = crate::world::ItemCategoryCatalog::default();
        let profiles = crate::world::InventoryProfileCatalog::default();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let inventory_id = create_inventory(
            world.inventory_store_mut(),
            &ctx,
            InventoryProfileId::new("unit_backpack_standard"),
            InventoryOwnerRef::Detached,
        )
        .unwrap();
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("gold"),
            5,
        )
        .unwrap();
        drop((inventory_store, instance_store));
        dev_clear_inventory(&mut world, &ctx, DevInventoryEndpoint::Grid(inventory_id)).unwrap();
        assert!(
            world
                .inventory_store()
                .get(inventory_id)
                .unwrap()
                .placed_entries()
                .is_empty()
        );
    }

    #[test]
    fn ensure_dev_unit_inventory_attaches_backpack() {
        use crate::world::{
            ChunkCoord, ChunkData, ChunkId, Heightfield, LocalPosition, UnitCatalog,
            UnitDefinitionId, UnitOwnership, UnitSource, WorldPosition, create_unit_with_ownership,
        };
        use bevy::prelude::{Quat, Vec3};

        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        let unit_catalog = UnitCatalog::default();
        let items = crate::world::ItemCatalog::default();
        let categories = crate::world::ItemCategoryCatalog::default();
        let profiles = crate::world::InventoryProfileCatalog::default();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(10.0, 0.0, 10.0)),
        );
        let unit = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            position,
            UnitSource::Dev,
            UnitOwnership::player_default(),
        )
        .unwrap();
        assert!(unit.inventory_id.is_none());

        let inventory_id =
            ensure_dev_unit_inventory(&mut world, &unit_catalog, &ctx, unit.id).unwrap();
        let restored = world.get_unit(unit.id).unwrap();
        assert_eq!(restored.inventory_id, Some(inventory_id));
        assert!(world.inventory_store().get(inventory_id).is_some());
    }

    #[test]
    fn dev_add_item_to_unit_inventory() {
        use crate::world::{
            ChunkCoord, ChunkData, ChunkId, Heightfield, LocalPosition, UnitCatalog,
            UnitDefinitionId, UnitOwnership, UnitSource, WorldPosition, create_unit_with_ownership,
        };
        use bevy::prelude::Vec3;

        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        let unit_catalog = UnitCatalog::default();
        let items = crate::world::ItemCatalog::default();
        let categories = crate::world::ItemCategoryCatalog::default();
        let profiles = crate::world::InventoryProfileCatalog::default();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(10.0, 0.0, 10.0)),
        );
        let unit = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            position,
            UnitSource::Dev,
            UnitOwnership::player_default(),
        )
        .unwrap();
        let inventory_id =
            ensure_dev_unit_inventory(&mut world, &unit_catalog, &ctx, unit.id).unwrap();
        let message = dev_add_item(
            &mut world,
            &ctx,
            DevInventoryEndpoint::Grid(inventory_id),
            ItemDefinitionId::new("gold"),
            25,
            &ItemPileSettings::default(),
            position,
            0,
        )
        .unwrap();
        assert!(message.contains("25"));
        assert_eq!(
            world
                .inventory_store()
                .get(inventory_id)
                .unwrap()
                .placed_entries()
                .len(),
            1
        );
    }

    #[test]
    fn dev_spawn_ground_pile_creates_record() {
        use crate::world::{ChunkCoord, LocalPosition, WorldPosition};
        use bevy::prelude::Vec3;

        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(12.0, 0.0, 12.0)),
        );
        let pile_id =
            dev_spawn_ground_pile(&mut world, ItemDefinitionId::new("gold"), 25, position, 1)
                .unwrap();
        let pile = world.item_pile_store().get(pile_id).unwrap();
        assert_eq!(pile.stack_quantity(), Some(25));
    }

    #[test]
    fn zero_quantity_rejected() {
        use crate::world::{ChunkCoord, LocalPosition, WorldPosition};
        use bevy::prelude::Vec3;

        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let items = crate::world::ItemCatalog::default();
        let categories = crate::world::ItemCategoryCatalog::default();
        let profiles = crate::world::InventoryProfileCatalog::default();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let inventory_id = create_inventory(
            world.inventory_store_mut(),
            &ctx,
            InventoryProfileId::new("unit_backpack_standard"),
            InventoryOwnerRef::Detached,
        )
        .unwrap();
        let err = dev_add_item(
            &mut world,
            &ctx,
            DevInventoryEndpoint::Grid(inventory_id),
            ItemDefinitionId::new("gold"),
            0,
            &ItemPileSettings::default(),
            WorldPosition::new(ChunkCoord::new(0, 0), LocalPosition::new(Vec3::ZERO)),
            0,
        )
        .unwrap_err();
        assert!(err.to_string().contains("quantity"));
    }

    fn placement_test_catalog() -> (
        crate::world::ItemCatalog,
        crate::world::ItemCategoryCatalog,
        InventoryProfileCatalog,
    ) {
        use crate::world::{
            ItemCatalog, ItemCategoryCatalog, ItemCategoryDefinition, ItemCategoryId,
            ItemDefinition,
        };

        let categories = ItemCategoryCatalog::from_definitions(vec![
            ItemCategoryDefinition::new(ItemCategoryId::new("currency"), "Currency", "", true),
            ItemCategoryDefinition::new(ItemCategoryId::new("weapon"), "Weapon", "", true),
            ItemCategoryDefinition::new(ItemCategoryId::new("medicine"), "Medicine", "", true),
        ])
        .unwrap();
        let items = ItemCatalog::from_definitions(
            vec![
                ItemDefinition::new(
                    ItemDefinitionId::new("gold"),
                    "Gold",
                    "",
                    ItemCategoryId::new("currency"),
                    1,
                    1,
                    true,
                    999,
                    1,
                    1,
                    true,
                ),
                ItemDefinition::new(
                    ItemDefinitionId::new("crossbow"),
                    "Crossbow",
                    "",
                    ItemCategoryId::new("weapon"),
                    2,
                    3,
                    false,
                    1,
                    5000,
                    200,
                    true,
                ),
                ItemDefinition::new(
                    ItemDefinitionId::new("healing_kit"),
                    "Healing Kit",
                    "",
                    ItemCategoryId::new("medicine"),
                    2,
                    2,
                    false,
                    1,
                    300,
                    25,
                    true,
                )
                .with_unique_instance_required(true),
            ],
            &categories,
        )
        .unwrap();
        (items, categories, InventoryProfileCatalog::default())
    }

    fn empty_backpack(
        world: &mut crate::world::WorldData,
        ctx: &InventoryCatalogCtx<'_>,
    ) -> InventoryId {
        create_inventory(
            world.inventory_store_mut(),
            ctx,
            InventoryProfileId::new("unit_backpack_standard"),
            InventoryOwnerRef::Detached,
        )
        .unwrap()
    }

    #[test]
    fn dev_place_stackable_item_at_anchor() {
        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let (items, categories, profiles) = placement_test_catalog();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let inventory_id = empty_backpack(&mut world, &ctx);
        dev_place_item_at_anchor(
            &mut world,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("gold"),
            25,
            0,
            0,
        )
        .unwrap();
        let entry = world
            .inventory_store()
            .get(inventory_id)
            .unwrap()
            .placed_entries()
            .first()
            .unwrap();
        assert!(matches!(
            &entry.contents,
            InventoryEntryContents::Stack { quantity: 25, .. }
        ));
    }

    #[test]
    fn dev_place_non_stackable_item_at_anchor() {
        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let (items, categories, profiles) = placement_test_catalog();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let inventory_id = empty_backpack(&mut world, &ctx);
        dev_place_item_at_anchor(
            &mut world,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("crossbow"),
            10,
            0,
            0,
        )
        .unwrap();
        let record = world.inventory_store().get(inventory_id).unwrap();
        assert_eq!(record.placed_entries().len(), 1);
        assert!(matches!(
            record.placed_entries()[0].contents,
            InventoryEntryContents::Unique { .. }
        ));
    }

    #[test]
    fn dev_place_unique_instance_required_item_at_anchor() {
        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let (items, categories, profiles) = placement_test_catalog();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let inventory_id = empty_backpack(&mut world, &ctx);
        dev_place_item_at_anchor(
            &mut world,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("healing_kit"),
            1,
            2,
            2,
        )
        .unwrap();
        let entry = world
            .inventory_store()
            .get(inventory_id)
            .unwrap()
            .placed_entries()
            .first()
            .unwrap();
        assert_eq!(entry.anchor_x, 2);
        assert_eq!(entry.anchor_y, 2);
        assert!(matches!(
            entry.contents,
            InventoryEntryContents::Unique { .. }
        ));
    }

    #[test]
    fn crossbow_fits_in_standard_backpack() {
        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let (items, categories, profiles) = placement_test_catalog();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let inventory_id = empty_backpack(&mut world, &ctx);
        dev_place_item_at_anchor(
            &mut world,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("crossbow"),
            1,
            0,
            0,
        )
        .unwrap();
        let entry = world
            .inventory_store()
            .get(inventory_id)
            .unwrap()
            .placed_entries()
            .first()
            .unwrap();
        assert_eq!(entry.anchor_x, 0);
        assert_eq!(entry.anchor_y, 0);
    }

    #[test]
    fn crossbow_fails_when_out_of_bounds() {
        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let (items, categories, profiles) = placement_test_catalog();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let inventory_id = empty_backpack(&mut world, &ctx);
        let err = dev_place_item_at_anchor(
            &mut world,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("crossbow"),
            1,
            5,
            0,
        )
        .unwrap_err();
        assert!(err.to_string().contains("bounds") || err.to_string().contains("Grid"));
    }

    #[test]
    fn crossbow_fails_when_overlapping() {
        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let (items, categories, profiles) = placement_test_catalog();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let inventory_id = empty_backpack(&mut world, &ctx);
        dev_place_item_at_anchor(
            &mut world,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("crossbow"),
            1,
            0,
            0,
        )
        .unwrap();
        let err = dev_place_item_at_anchor(
            &mut world,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("crossbow"),
            1,
            1,
            0,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("occupied") || err.to_string().contains("Cells"),
            "{}",
            err
        );
    }

    #[test]
    fn dev_add_item_places_non_stackable_via_unique_entry() {
        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let (items, categories, profiles) = placement_test_catalog();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let inventory_id = empty_backpack(&mut world, &ctx);
        dev_add_item(
            &mut world,
            &ctx,
            DevInventoryEndpoint::Grid(inventory_id),
            ItemDefinitionId::new("crossbow"),
            10,
            &ItemPileSettings::default(),
            WorldPosition::new(
                crate::world::ChunkCoord::new(0, 0),
                crate::world::LocalPosition::new(bevy::prelude::Vec3::ZERO),
            ),
            0,
        )
        .unwrap();
        let record = world.inventory_store().get(inventory_id).unwrap();
        assert_eq!(record.placed_entries().len(), 1);
        assert!(matches!(
            record.placed_entries()[0].contents,
            InventoryEntryContents::Unique { .. }
        ));
    }

    #[test]
    fn dev_effective_placement_quantity_clamps_non_stackable() {
        let (items, _, _) = placement_test_catalog();
        let crossbow = items.get(&ItemDefinitionId::new("crossbow")).unwrap();
        assert_eq!(dev_effective_placement_quantity(crossbow, 10), 1);
        let gold = items.get(&ItemDefinitionId::new("gold")).unwrap();
        assert_eq!(dev_effective_placement_quantity(gold, 10), 10);
    }
}
