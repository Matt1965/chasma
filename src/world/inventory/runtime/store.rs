use std::collections::BTreeMap;

use bevy::prelude::Reflect;

use super::entry::EntryIndex;
use super::id::{InventoryId, ItemInstanceId};
use super::instance::ItemInstance;
use super::instance_location::ItemInstanceLocation;
use super::record::InventoryRecord;

/// Runtime inventory container index on [`crate::world::WorldData`] (ADR-088 I2).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct InventoryStore {
    next_inventory_id: u32,
    inventories: BTreeMap<InventoryId, InventoryRecord>,
}

impl Default for InventoryStore {
    fn default() -> Self {
        Self {
            next_inventory_id: 1,
            inventories: BTreeMap::new(),
        }
    }
}

impl InventoryStore {
    pub fn allocate_inventory_id(&mut self) -> InventoryId {
        let id = InventoryId::new(self.next_inventory_id);
        self.next_inventory_id = self.next_inventory_id.saturating_add(1);
        id
    }

    pub fn get(&self, id: InventoryId) -> Option<&InventoryRecord> {
        self.inventories.get(&id)
    }

    pub fn get_mut(&mut self, id: InventoryId) -> Option<&mut InventoryRecord> {
        self.inventories.get_mut(&id)
    }

    pub fn insert(&mut self, record: InventoryRecord) -> Result<(), super::error::InventoryError> {
        let id = record.id();
        if self.inventories.contains_key(&id) {
            return Err(super::error::InventoryError::DuplicateInventory(id));
        }
        self.inventories.insert(id, record);
        Ok(())
    }

    pub fn remove(&mut self, id: InventoryId) -> Option<InventoryRecord> {
        self.inventories.remove(&id)
    }

    pub fn sorted_inventory_ids(&self) -> Vec<InventoryId> {
        self.inventories.keys().copied().collect()
    }

    pub fn next_id(&self) -> u32 {
        self.next_inventory_id
    }

    pub fn restore_next_id(&mut self, next: u32) {
        self.next_inventory_id = self.next_inventory_id.max(next);
    }

    pub fn clear(&mut self) {
        self.next_inventory_id = 1;
        self.inventories.clear();
    }

    /// Replace all inventory records (scene restore — ADR-094 I8).
    pub fn restore_snapshot(
        &mut self,
        records: Vec<InventoryRecord>,
        next_id: u32,
    ) -> Result<(), super::error::InventoryError> {
        self.clear();
        self.restore_next_id(next_id);
        for record in records {
            self.insert(record)?;
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.inventories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inventories.is_empty()
    }
}

/// Runtime unique item instance index on [`crate::world::WorldData`] (ADR-088 I2).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ItemInstanceStore {
    next_item_instance_id: u32,
    instances: BTreeMap<ItemInstanceId, ItemInstance>,
    /// Authoritative containment: instance → location.
    instance_location: BTreeMap<ItemInstanceId, ItemInstanceLocation>,
}

impl Default for ItemInstanceStore {
    fn default() -> Self {
        Self {
            next_item_instance_id: 1,
            instances: BTreeMap::new(),
            instance_location: BTreeMap::new(),
        }
    }
}

impl ItemInstanceStore {
    pub fn allocate_item_instance_id(&mut self) -> ItemInstanceId {
        let id = ItemInstanceId::new(self.next_item_instance_id);
        self.next_item_instance_id = self.next_item_instance_id.saturating_add(1);
        id
    }

    pub fn get(&self, id: ItemInstanceId) -> Option<&ItemInstance> {
        self.instances.get(&id)
    }

    pub fn get_mut(&mut self, id: ItemInstanceId) -> Option<&mut ItemInstance> {
        self.instances.get_mut(&id)
    }

    pub fn insert(&mut self, instance: ItemInstance) -> Result<(), super::error::InventoryError> {
        if self.instances.contains_key(&instance.id) {
            return Err(super::error::InventoryError::DuplicateItemInstance(
                instance.id,
            ));
        }
        self.instances.insert(instance.id, instance);
        Ok(())
    }

    pub fn remove(&mut self, id: ItemInstanceId) -> Option<ItemInstance> {
        self.instance_location.remove(&id);
        self.instances.remove(&id)
    }

    pub fn location(&self, id: ItemInstanceId) -> Option<ItemInstanceLocation> {
        self.instance_location.get(&id).copied()
    }

    pub fn inventory_location(&self, id: ItemInstanceId) -> Option<(InventoryId, EntryIndex)> {
        self.location(id).and_then(ItemInstanceLocation::inventory)
    }

    pub fn set_location(&mut self, id: ItemInstanceId, location: ItemInstanceLocation) {
        if location.is_detached() {
            self.instance_location.remove(&id);
        } else {
            self.instance_location.insert(id, location);
        }
    }

    pub fn set_inventory_location(
        &mut self,
        id: ItemInstanceId,
        inventory_id: InventoryId,
        entry_index: EntryIndex,
    ) {
        self.set_location(
            id,
            ItemInstanceLocation::Inventory {
                inventory_id,
                entry_index,
            },
        );
    }

    pub fn set_world_pile_location(
        &mut self,
        id: ItemInstanceId,
        pile_id: crate::world::ItemPileId,
    ) {
        self.set_location(id, ItemInstanceLocation::WorldPile(pile_id));
    }

    pub fn clear_location(&mut self, id: ItemInstanceId) {
        self.set_location(id, ItemInstanceLocation::Detached);
    }

    pub fn sorted_item_instance_ids(&self) -> Vec<ItemInstanceId> {
        self.instances.keys().copied().collect()
    }

    pub fn next_id(&self) -> u32 {
        self.next_item_instance_id
    }

    pub fn restore_next_id(&mut self, next: u32) {
        self.next_item_instance_id = self.next_item_instance_id.max(next);
    }

    pub fn clear(&mut self) {
        self.next_item_instance_id = 1;
        self.instances.clear();
        self.instance_location.clear();
    }

    /// Replace all item instances and locations (scene restore — ADR-094 I8).
    pub fn restore_snapshot(
        &mut self,
        instances: Vec<ItemInstance>,
        locations: Vec<(ItemInstanceId, ItemInstanceLocation)>,
        next_id: u32,
    ) -> Result<(), super::error::InventoryError> {
        self.clear();
        self.restore_next_id(next_id);
        for instance in instances {
            self.insert(instance)?;
        }
        for (id, location) in locations {
            if !location.is_detached() {
                self.set_location(id, location);
            }
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }
}
