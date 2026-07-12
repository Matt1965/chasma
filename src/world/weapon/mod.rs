//! Weapon data layer (ADR-054 C1).
//!
//! Owns weapon type definitions in [`catalog::WeaponCatalog`]. Combat behavior
//! is deferred to later phases; this module is catalog data only.

mod catalog;

#[cfg(any(test, feature = "dev"))]
pub use catalog::starter_definitions;
pub use catalog::{
    AttackPlaybackPolicy, DamageType, HitMode, TargetFilter, WeaponAttackAnimation, WeaponCatalog,
    WeaponCatalogError, WeaponDefinition, WeaponDefinitionId,
};
