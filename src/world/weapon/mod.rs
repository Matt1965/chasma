//! Weapon data layer (ADR-054 C1).
//!
//! Owns weapon type definitions in [`catalog::WeaponCatalog`]. Combat behavior
//! is deferred to later phases; this module is catalog data only.

mod catalog;

pub use catalog::{
    DamageType, HitMode, TargetFilter, WeaponCatalog, WeaponCatalogError, WeaponDefinition,
    WeaponDefinitionId,
};
#[cfg(any(test, feature = "dev"))]
pub use catalog::starter_definitions;
