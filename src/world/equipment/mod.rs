//! Authoritative equipment-slot inventories (Slice 1).

mod access;
mod armor_resolve;
mod attach;
mod cargo;
mod container;
mod effective_weapon;
mod inventories;
mod placement;
mod presentation;
mod profiles;
mod slot;
mod visual_catalog;

#[cfg(test)]
mod armor_mitigation_tests;
#[cfg(test)]
mod cargo_tests;
#[cfg(test)]
mod effective_weapon_tests;
#[cfg(test)]
mod tests;

pub use access::unit_owns_inventory;
pub use armor_resolve::{
    ArmorResolveError, EquippedArmorEntry, equipped_armor_for_unit, total_armor_rating_for_unit,
};
pub use attach::{
    attach_equipment_on_unit_create, cleanup_unit_equipment_on_delete, minimal_catalog_ctx,
    reconcile_legacy_unit_equipment, unit_equipment_slots_are_empty, validate_unit_equipment_links,
};
pub use cargo::{
    WorkerCargoResolveError, carried_quantity_in_worker_cargo,
    equipped_backpack_internal_inventory, resolve_equipped_backpack_internal,
    worker_cargo_capacity_for_item, worker_cargo_inventories,
};
pub use container::{
    container_inventory_is_empty, container_inventory_is_loaded, create_container_inventory,
    is_container_item, release_container_inventory_if_empty,
};
pub use effective_weapon::{effective_weapon_for_unit, effective_weapon_id_for_unit};
pub use inventories::UnitEquipmentInventories;
pub use placement::validate_item_placement;
pub use presentation::{
    EquipmentAttachmentSocket, EquipmentPresentationAuthoring, EquipmentPresentationMode,
    default_socket_for_slot, slot_supports_equipment_presentation,
    slot_supports_rigid_presentation,
};
pub use profiles::equipment_slot_profile_definitions;
pub use slot::EquipmentSlot;
pub use visual_catalog::{EquipmentVisualCatalog, EquipmentVisualMapping};
