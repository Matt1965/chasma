//! Authoritative per-building inventory binding storage on WorldData (EP4).

use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::BuildingId;
use crate::world::InventoryId;

use super::binding::BuildingInventoryBinding;
use super::binding_id::BuildingInventoryBindingId;
use super::role::BuildingInventoryRole;

/// Indexed runtime bindings for one building (EP4).
#[derive(Debug, Clone, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct BuildingInventoryBindingSet {
    bindings: Vec<BuildingInventoryBinding>,
    #[serde(skip)]
    by_id: HashMap<BuildingInventoryBindingId, usize>,
}

impl BuildingInventoryBindingSet {
    pub fn from_bindings(bindings: Vec<BuildingInventoryBinding>) -> Self {
        let mut set = Self {
            bindings,
            by_id: HashMap::new(),
        };
        set.rebuild_index();
        set
    }

    pub fn bindings(&self) -> &[BuildingInventoryBinding] {
        &self.bindings
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn get(&self, binding_id: &BuildingInventoryBindingId) -> Option<&BuildingInventoryBinding> {
        self.by_id
            .get(binding_id)
            .map(|&index| &self.bindings[index])
    }

    pub fn resolve_inventory(
        &self,
        binding_id: &BuildingInventoryBindingId,
    ) -> Option<InventoryId> {
        self.get(binding_id).map(|binding| binding.inventory_id)
    }

    pub fn bindings_with_role(
        &self,
        role: BuildingInventoryRole,
    ) -> impl Iterator<Item = &BuildingInventoryBinding> {
        self.bindings
            .iter()
            .filter(move |binding| binding.role == role)
    }

    pub fn default_binding(&self) -> Option<&BuildingInventoryBinding> {
        self.bindings
            .iter()
            .find(|binding| binding.is_default)
            .or_else(|| self.bindings.first())
    }

    pub fn default_inventory_id(&self) -> Option<InventoryId> {
        self.default_binding().map(|binding| binding.inventory_id)
    }

    pub fn rebuild_index(&mut self) {
        self.by_id.clear();
        for (index, binding) in self.bindings.iter().enumerate() {
            self.by_id.insert(binding.binding_id.clone(), index);
        }
    }
}

/// Authoritative building inventory binding store (EP4).
#[derive(Debug, Clone, Default, Reflect, Serialize, Deserialize)]
pub struct BuildingInventoryBindingStore {
    buildings: HashMap<u64, BuildingInventoryBindingSet>,
}

impl BuildingInventoryBindingStore {
    pub fn clear(&mut self) {
        self.buildings.clear();
    }

    pub fn remove(&mut self, building_id: BuildingId) {
        self.buildings.remove(&building_id.raw());
    }

    pub fn get(&self, building_id: BuildingId) -> Option<&BuildingInventoryBindingSet> {
        self.buildings.get(&building_id.raw())
    }

    pub fn get_mut(&mut self, building_id: BuildingId) -> Option<&mut BuildingInventoryBindingSet> {
        self.buildings.get_mut(&building_id.raw())
    }

    pub fn set(&mut self, building_id: BuildingId, bindings: BuildingInventoryBindingSet) {
        self.buildings.insert(building_id.raw(), bindings);
    }

    pub fn resolve_inventory(
        &self,
        building_id: BuildingId,
        binding_id: &BuildingInventoryBindingId,
    ) -> Option<InventoryId> {
        self.get(building_id)
            .and_then(|set| set.resolve_inventory(binding_id))
    }

    pub fn building_ids(&self) -> impl Iterator<Item = BuildingId> + '_ {
        self.buildings
            .keys()
            .copied()
            .map(BuildingId::new)
    }

    pub fn export_buildings(&self) -> HashMap<u64, BuildingInventoryBindingSet> {
        self.buildings.clone()
    }

    pub fn import_buildings(&mut self, buildings: HashMap<u64, BuildingInventoryBindingSet>) {
        self.buildings = buildings;
        for set in self.buildings.values_mut() {
            set.rebuild_index();
        }
    }
}
