use bevy::prelude::*;

use super::catalog_ctx::InventoryCatalogCtx;
use super::entry::{EntryIndex, PlacedInventoryEntry};
use super::grid::{rebuild_derived_state, validate_inventory_caches};
use super::id::InventoryId;
use super::owner::InventoryOwnerRef;
use crate::world::InventoryProfileId;

/// Authoritative inventory grid state (ADR-088 I2).
///
/// Derived occupancy and mass caches are private; mutate only through inventory APIs.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct InventoryRecord {
    id: InventoryId,
    owner: InventoryOwnerRef,
    profile_id: InventoryProfileId,
    grid_width: u8,
    grid_height: u8,
    placed_entries: Vec<PlacedInventoryEntry>,
    #[reflect(ignore)]
    cell_owner: Vec<Option<EntryIndex>>,
    total_mass_grams: u64,
}

impl InventoryRecord {
    pub fn new(
        id: InventoryId,
        owner: InventoryOwnerRef,
        profile_id: InventoryProfileId,
        grid_width: u8,
        grid_height: u8,
    ) -> Self {
        let cell_count = usize::from(grid_width) * usize::from(grid_height);
        Self {
            id,
            owner,
            profile_id,
            grid_width,
            grid_height,
            placed_entries: Vec::new(),
            cell_owner: vec![None; cell_count],
            total_mass_grams: 0,
        }
    }

    pub fn id(&self) -> InventoryId {
        self.id
    }

    pub fn owner(&self) -> &InventoryOwnerRef {
        &self.owner
    }

    pub fn set_owner(&mut self, owner: InventoryOwnerRef) {
        self.owner = owner;
    }

    pub fn profile_id(&self) -> &InventoryProfileId {
        &self.profile_id
    }

    pub fn grid_width(&self) -> u8 {
        self.grid_width
    }

    pub fn grid_height(&self) -> u8 {
        self.grid_height
    }

    pub fn placed_entries(&self) -> &[PlacedInventoryEntry] {
        &self.placed_entries
    }

    pub fn placed_entries_mut(&mut self) -> &mut Vec<PlacedInventoryEntry> {
        &mut self.placed_entries
    }

    pub fn total_mass_grams(&self) -> u64 {
        self.total_mass_grams
    }

    pub fn cell_owner(&self) -> &[Option<EntryIndex>] {
        &self.cell_owner
    }

    pub(crate) fn set_total_mass_grams(&mut self, mass: u64) {
        self.total_mass_grams = mass;
    }

    pub(crate) fn set_cell_owner(&mut self, cells: Vec<Option<EntryIndex>>) {
        self.cell_owner = cells;
    }

    pub fn entry_at_cell(&self, x: u8, y: u8) -> Option<EntryIndex> {
        let index = cell_index(self.grid_width, x, y)?;
        self.cell_owner.get(index).and_then(|entry| *entry)
    }

    pub fn rebuild_derived(
        &mut self,
        ctx: &InventoryCatalogCtx<'_>,
        instance_definition: impl Fn(
            super::id::ItemInstanceId,
        ) -> Result<
            crate::world::ItemDefinitionId,
            super::error::InventoryError,
        >,
    ) -> Result<(), super::error::InventoryError> {
        rebuild_derived_state(self, ctx, instance_definition)
    }

    pub fn validate_caches(
        &self,
        ctx: &InventoryCatalogCtx<'_>,
        instance_definition: impl Fn(
            super::id::ItemInstanceId,
        ) -> Result<
            crate::world::ItemDefinitionId,
            super::error::InventoryError,
        >,
    ) -> Result<(), super::error::InventoryError> {
        validate_inventory_caches(self, ctx, instance_definition)
    }
}

pub fn cell_index(grid_width: u8, x: u8, y: u8) -> Option<usize> {
    Some(usize::from(y) * usize::from(grid_width) + usize::from(x))
}
