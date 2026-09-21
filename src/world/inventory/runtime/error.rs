use super::entry::EntryIndex;
use super::id::{InventoryId, ItemInstanceId};
use super::owner::InventoryOwnerRef;
use crate::world::equipment::EquipmentSlot;
use crate::world::{InventoryProfileId, ItemDefinitionId, UnitId};

/// Structured inventory mutation and validation errors (ADR-088 I2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryError {
    InventoryNotFound(InventoryId),
    DuplicateInventory(InventoryId),
    UnitInventoryProfileMissing(UnitId),
    InventoryAllocationFailed(UnitId),
    UnitInventoryOwnerMismatch {
        unit_id: UnitId,
        inventory_id: InventoryId,
    },
    InventoryAlreadyOwned {
        inventory_id: InventoryId,
        owner: super::owner::InventoryOwnerRef,
    },
    DeathInventoryInvariantViolation {
        unit_id: UnitId,
        message: String,
    },
    ItemDefinitionNotFound(ItemDefinitionId),
    ItemDefinitionDisabled(ItemDefinitionId),
    ItemInstanceNotFound(ItemInstanceId),
    DuplicateItemInstance(ItemInstanceId),
    InvalidStackQuantity {
        quantity: u32,
        limit: u32,
    },
    StackLimitExceeded {
        item_definition_id: ItemDefinitionId,
        requested: u32,
        limit: u32,
    },
    GridOutOfBounds {
        inventory_id: InventoryId,
        x: u8,
        y: u8,
    },
    CellsOccupied {
        inventory_id: InventoryId,
    },
    UniqueItemAlreadyContained {
        item_instance_id: ItemInstanceId,
        inventory_id: InventoryId,
    },
    ItemInstanceUncontainedRequired(ItemInstanceId),
    CannotMergeDifferentItems,
    CannotMergeUniqueItem,
    InvalidSwap {
        inventory_id: InventoryId,
        entry_a: EntryIndex,
        entry_b: EntryIndex,
    },
    EntryNotFound {
        inventory_id: InventoryId,
        entry_index: EntryIndex,
    },
    QuantityUnderflow,
    QuantityOverflow,
    MassOverflow,
    CacheInconsistent {
        inventory_id: InventoryId,
    },
    AutoSortNoFit {
        inventory_id: InventoryId,
        leftover_entries: usize,
    },
    ProfileNotFound(InventoryProfileId),
    InvalidProfileMigration {
        inventory_id: InventoryId,
        message: String,
    },
    OwnerMismatch {
        inventory_id: InventoryId,
        expected: InventoryOwnerRef,
    },
    NotStackEntry {
        inventory_id: InventoryId,
        entry_index: EntryIndex,
    },
    NotUniqueEntry {
        inventory_id: InventoryId,
        entry_index: EntryIndex,
    },
    NoFitPosition {
        inventory_id: InventoryId,
    },
    NonStackableItem(ItemDefinitionId),
    UniqueItemRequired(ItemDefinitionId),
    MaxPlacedEntriesExceeded {
        inventory_id: InventoryId,
        max_entries: u8,
    },
    IncompatibleEquipmentSlot {
        inventory_id: InventoryId,
        slot: EquipmentSlot,
        item_definition_id: ItemDefinitionId,
    },
    LoadedContainerInPersonalInventory {
        unit_id: UnitId,
        item_definition_id: ItemDefinitionId,
    },
    ContainerLoadingForbiddenInPersonalInventory {
        unit_id: UnitId,
        container_instance_id: ItemInstanceId,
        inventory_id: InventoryId,
    },
    ContainerRecursionForbidden {
        item_definition_id: ItemDefinitionId,
    },
    DuplicateContainerInventory(ItemInstanceId),
    ContainerNotEmptyOnRelease {
        item_instance_id: ItemInstanceId,
        inventory_id: InventoryId,
    },
    UnitEquipmentMissing(UnitId),
    UnitEquipmentOwnerMismatch {
        unit_id: UnitId,
        slot: EquipmentSlot,
        inventory_id: InventoryId,
    },
    UnitEquipmentProfileMismatch {
        unit_id: UnitId,
        slot: EquipmentSlot,
        inventory_id: InventoryId,
        profile_id: InventoryProfileId,
    },
    ContainerOwnerInstanceMissing {
        item_instance_id: ItemInstanceId,
        inventory_id: InventoryId,
    },
    ContainerInventoryMismatch {
        item_instance_id: ItemInstanceId,
        inventory_id: InventoryId,
    },
}

impl std::fmt::Display for InventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InventoryNotFound(id) => write!(f, "inventory not found `{id:?}`"),
            Self::DuplicateInventory(id) => write!(f, "duplicate inventory `{id:?}`"),
            Self::UnitInventoryProfileMissing(id) => {
                write!(f, "unit inventory profile missing for `{id:?}`")
            }
            Self::InventoryAllocationFailed(id) => {
                write!(f, "inventory allocation failed for unit `{id:?}`")
            }
            Self::UnitInventoryOwnerMismatch {
                unit_id,
                inventory_id,
            } => write!(
                f,
                "unit `{unit_id:?}` inventory owner mismatch for `{inventory_id:?}`"
            ),
            Self::InventoryAlreadyOwned {
                inventory_id,
                owner,
            } => write!(
                f,
                "inventory `{inventory_id:?}` already owned by `{owner:?}`"
            ),
            Self::DeathInventoryInvariantViolation { unit_id, message } => write!(
                f,
                "death inventory invariant for unit `{unit_id:?}`: {message}"
            ),
            Self::ItemDefinitionNotFound(id) => {
                write!(f, "item definition not found `{}`", id.as_str())
            }
            Self::ItemDefinitionDisabled(id) => {
                write!(f, "item definition disabled `{}`", id.as_str())
            }
            Self::ItemInstanceNotFound(id) => write!(f, "item instance not found `{id:?}`"),
            Self::DuplicateItemInstance(id) => write!(f, "duplicate item instance `{id:?}`"),
            Self::InvalidStackQuantity { quantity, limit } => {
                write!(f, "invalid stack quantity {quantity} (limit {limit})")
            }
            Self::StackLimitExceeded {
                item_definition_id,
                requested,
                limit,
            } => write!(
                f,
                "stack limit exceeded for `{}`: requested {requested}, limit {limit}",
                item_definition_id.as_str()
            ),
            Self::GridOutOfBounds { inventory_id, x, y } => write!(
                f,
                "grid out of bounds at ({x},{y}) in inventory `{inventory_id:?}`"
            ),
            Self::CellsOccupied { inventory_id } => {
                write!(f, "cells occupied in inventory `{inventory_id:?}`")
            }
            Self::UniqueItemAlreadyContained {
                item_instance_id,
                inventory_id,
            } => write!(
                f,
                "unique item `{item_instance_id:?}` already in inventory `{inventory_id:?}`"
            ),
            Self::ItemInstanceUncontainedRequired(id) => {
                write!(f, "item instance `{id:?}` must be uncontained")
            }
            Self::CannotMergeDifferentItems => write!(f, "cannot merge different items"),
            Self::CannotMergeUniqueItem => write!(f, "cannot merge unique item"),
            Self::InvalidSwap {
                inventory_id,
                entry_a,
                entry_b,
            } => write!(
                f,
                "invalid swap entries {entry_a}/{entry_b} in inventory `{inventory_id:?}`"
            ),
            Self::EntryNotFound {
                inventory_id,
                entry_index,
            } => write!(
                f,
                "entry {entry_index} not found in inventory `{inventory_id:?}`"
            ),
            Self::QuantityUnderflow => write!(f, "quantity underflow"),
            Self::QuantityOverflow => write!(f, "quantity overflow"),
            Self::MassOverflow => write!(f, "mass overflow"),
            Self::CacheInconsistent { inventory_id } => {
                write!(f, "cache inconsistent for inventory `{inventory_id:?}`")
            }
            Self::AutoSortNoFit {
                inventory_id,
                leftover_entries,
            } => write!(
                f,
                "auto-sort no fit for inventory `{inventory_id:?}` ({leftover_entries} leftovers)"
            ),
            Self::ProfileNotFound(id) => write!(f, "profile not found `{id}`", id = id.as_str()),
            Self::InvalidProfileMigration {
                inventory_id,
                message,
            } => write!(
                f,
                "invalid profile migration for `{inventory_id:?}`: {message}"
            ),
            Self::OwnerMismatch {
                inventory_id,
                expected,
            } => write!(
                f,
                "owner mismatch for inventory `{inventory_id:?}` (expected {expected:?})"
            ),
            Self::NotStackEntry {
                inventory_id,
                entry_index,
            } => write!(
                f,
                "entry {entry_index} is not a stack in inventory `{inventory_id:?}`"
            ),
            Self::NotUniqueEntry {
                inventory_id,
                entry_index,
            } => write!(
                f,
                "entry {entry_index} is not unique in inventory `{inventory_id:?}`"
            ),
            Self::NoFitPosition { inventory_id } => {
                write!(f, "no fit position in inventory `{inventory_id:?}`")
            }
            Self::NonStackableItem(id) => write!(f, "item `{}` is not stackable", id.as_str()),
            Self::UniqueItemRequired(id) => {
                write!(f, "item `{}` requires unique instance", id.as_str())
            }
            Self::MaxPlacedEntriesExceeded {
                inventory_id,
                max_entries,
            } => write!(
                f,
                "inventory `{inventory_id:?}` already has max {max_entries} placed entries"
            ),
            Self::IncompatibleEquipmentSlot {
                inventory_id,
                slot,
                item_definition_id,
            } => write!(
                f,
                "item `{}` incompatible with equipment slot `{slot:?}` in inventory `{inventory_id:?}`",
                item_definition_id.as_str()
            ),
            Self::LoadedContainerInPersonalInventory {
                unit_id,
                item_definition_id,
            } => write!(
                f,
                "loaded container `{}` cannot enter personal inventory of unit `{unit_id:?}`",
                item_definition_id.as_str()
            ),
            Self::ContainerLoadingForbiddenInPersonalInventory {
                unit_id,
                container_instance_id,
                inventory_id,
            } => write!(
                f,
                "cannot load container instance `{container_instance_id:?}` while it is in personal inventory of unit `{unit_id:?}` (internal inventory `{inventory_id:?}`)"
            ),
            Self::ContainerRecursionForbidden { item_definition_id } => write!(
                f,
                "container item `{}` cannot be placed inside another container",
                item_definition_id.as_str()
            ),
            Self::DuplicateContainerInventory(id) => {
                write!(
                    f,
                    "item instance `{id:?}` already owns a container inventory"
                )
            }
            Self::ContainerNotEmptyOnRelease {
                item_instance_id,
                inventory_id,
            } => write!(
                f,
                "container `{item_instance_id:?}` inventory `{inventory_id:?}` is not empty"
            ),
            Self::UnitEquipmentMissing(unit_id) => {
                write!(f, "unit `{unit_id:?}` has no equipment inventories")
            }
            Self::UnitEquipmentOwnerMismatch {
                unit_id,
                slot,
                inventory_id,
            } => write!(
                f,
                "unit `{unit_id:?}` equipment slot `{slot:?}` owner mismatch for inventory `{inventory_id:?}`"
            ),
            Self::UnitEquipmentProfileMismatch {
                unit_id,
                slot,
                inventory_id,
                profile_id,
            } => write!(
                f,
                "unit `{unit_id:?}` slot `{slot:?}` inventory `{inventory_id:?}` profile mismatch (`{}`)",
                profile_id.as_str()
            ),
            Self::ContainerOwnerInstanceMissing {
                item_instance_id,
                inventory_id,
            } => write!(
                f,
                "container inventory `{inventory_id:?}` references missing instance `{item_instance_id:?}`"
            ),
            Self::ContainerInventoryMismatch {
                item_instance_id,
                inventory_id,
            } => write!(
                f,
                "item instance `{item_instance_id:?}` container inventory mismatch (`{inventory_id:?}`)"
            ),
        }
    }
}

impl std::error::Error for InventoryError {}
