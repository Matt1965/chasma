//! Equipped-armor resolution and total armor rating (Slice 3/4).

use crate::world::armor::{ArmorProfileCatalog, ArmorProfileDefinition, ArmorProfileId};
use crate::world::inventory::InventoryEntryContents;
use crate::world::unit::UnitRecord;
use crate::world::{ItemCatalog, ItemDefinitionId, WorldData};

use super::slot::EquipmentSlot;

/// Why equipped armor could not be resolved from authoritative equipment state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArmorResolveError {
    MissingEquipmentInventory {
        slot: EquipmentSlot,
    },
    MissingEquippedEntry {
        slot: EquipmentSlot,
    },
    InvalidEquippedEntry {
        slot: EquipmentSlot,
    },
    MissingItemInstance {
        slot: EquipmentSlot,
    },
    MissingItemDefinition {
        slot: EquipmentSlot,
        item_definition_id: ItemDefinitionId,
    },
    MissingArmorProfileLink {
        slot: EquipmentSlot,
        item_definition_id: ItemDefinitionId,
    },
    MissingArmorProfile {
        slot: EquipmentSlot,
        armor_profile_id: ArmorProfileId,
    },
    DisabledArmorProfile {
        slot: EquipmentSlot,
        armor_profile_id: ArmorProfileId,
    },
}

impl std::fmt::Display for ArmorResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingEquipmentInventory { slot } => {
                write!(
                    f,
                    "missing equipment inventory for {} slot",
                    slot.display_name()
                )
            }
            Self::MissingEquippedEntry { slot } => {
                write!(f, "missing equipped entry in {} slot", slot.display_name())
            }
            Self::InvalidEquippedEntry { slot } => {
                write!(
                    f,
                    "invalid equipped entry contents in {} slot",
                    slot.display_name()
                )
            }
            Self::MissingItemInstance { slot } => {
                write!(
                    f,
                    "missing item instance for {} slot equipment",
                    slot.display_name()
                )
            }
            Self::MissingItemDefinition {
                slot,
                item_definition_id,
            } => write!(
                f,
                "missing item definition `{}` for {} slot equipment",
                item_definition_id.as_str(),
                slot.display_name()
            ),
            Self::MissingArmorProfileLink {
                slot,
                item_definition_id,
            } => write!(
                f,
                "item `{}` in {} slot lacks armor_profile_id",
                item_definition_id.as_str(),
                slot.display_name()
            ),
            Self::MissingArmorProfile {
                slot,
                armor_profile_id,
            } => write!(
                f,
                "missing armor profile `{}` for {} slot equipment",
                armor_profile_id.as_str(),
                slot.display_name()
            ),
            Self::DisabledArmorProfile {
                slot,
                armor_profile_id,
            } => write!(
                f,
                "armor profile `{}` for {} slot equipment is disabled",
                armor_profile_id.as_str(),
                slot.display_name()
            ),
        }
    }
}

impl std::error::Error for ArmorResolveError {}

/// One equipped armor piece resolved through authoritative equipment inventories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquippedArmorEntry {
    pub slot: EquipmentSlot,
    pub item_definition_id: ItemDefinitionId,
    pub armor_profile_id: ArmorProfileId,
    pub profile: ArmorProfileDefinition,
}

const ARMOR_SLOTS: [EquipmentSlot; 5] = [
    EquipmentSlot::Head,
    EquipmentSlot::Body,
    EquipmentSlot::Arms,
    EquipmentSlot::Legs,
    EquipmentSlot::Feet,
];

/// Resolve equipped armor profiles from authoritative equipment slot inventories.
pub fn equipped_armor_for_unit(
    world: &WorldData,
    unit: &UnitRecord,
    item_catalog: &ItemCatalog,
    armor_catalog: &ArmorProfileCatalog,
) -> Result<Vec<EquippedArmorEntry>, ArmorResolveError> {
    let Some(equipment) = unit.equipment else {
        return Ok(Vec::new());
    };
    let mut entries = Vec::new();
    for slot in ARMOR_SLOTS {
        let inventory_id = equipment.inventory_id(slot);
        let Some(record) = world.inventory_store().get(inventory_id) else {
            return Err(ArmorResolveError::MissingEquipmentInventory { slot });
        };
        let Some(entry) = record.placed_entries().first() else {
            continue;
        };
        let instance_id = match &entry.contents {
            InventoryEntryContents::Unique { item_instance_id } => *item_instance_id,
            _ => return Err(ArmorResolveError::InvalidEquippedEntry { slot }),
        };
        let Some(instance) = world.item_instance_store().get(instance_id) else {
            return Err(ArmorResolveError::MissingItemInstance { slot });
        };
        let Some(item) = item_catalog.get(&instance.definition_id) else {
            return Err(ArmorResolveError::MissingItemDefinition {
                slot,
                item_definition_id: instance.definition_id.clone(),
            });
        };
        let Some(armor_profile_id) = item.armor_profile_id.clone() else {
            return Err(ArmorResolveError::MissingArmorProfileLink {
                slot,
                item_definition_id: instance.definition_id.clone(),
            });
        };
        let Some(profile) = armor_catalog.get(&armor_profile_id) else {
            return Err(ArmorResolveError::MissingArmorProfile {
                slot,
                armor_profile_id,
            });
        };
        if !profile.enabled {
            return Err(ArmorResolveError::DisabledArmorProfile {
                slot,
                armor_profile_id,
            });
        }
        entries.push(EquippedArmorEntry {
            slot,
            item_definition_id: instance.definition_id.clone(),
            armor_profile_id,
            profile: profile.clone(),
        });
    }
    Ok(entries)
}

/// Sum armor ratings from equipped Head/Body/Arms/Legs/Feet pieces.
pub fn total_armor_rating_for_unit(
    world: &WorldData,
    unit: &UnitRecord,
    item_catalog: &ItemCatalog,
    armor_catalog: &ArmorProfileCatalog,
) -> Result<u32, ArmorResolveError> {
    let entries = equipped_armor_for_unit(world, unit, item_catalog, armor_catalog)?;
    Ok(entries.iter().map(|entry| entry.profile.armor_rating).sum())
}
