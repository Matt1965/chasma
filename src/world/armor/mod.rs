//! Armor profile data layer and combat mitigation (Slice 3/4).

mod catalog;
mod mitigation;

#[cfg(test)]
pub use catalog::starter_definitions;
pub use catalog::{
    ArmorProfileCatalog, ArmorProfileCatalogError, ArmorProfileDefinition, ArmorProfileId,
};
pub use mitigation::{
    ARMOR_MITIGATION_K, damage_multiplier_for_armor, resolve_applied_combat_damage,
    resolve_damage_after_armor,
};
