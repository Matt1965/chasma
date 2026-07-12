//! Weapon type catalog — authoritative attack definitions (ADR-054 C1).

mod animation;
mod definition;
mod definition_id;
mod registry;
mod starter;

pub use animation::{AttackPlaybackPolicy, WeaponAttackAnimation};
pub use definition::{DamageType, HitMode, TargetFilter, WeaponDefinition};
pub use definition_id::WeaponDefinitionId;
pub use registry::{WeaponCatalog, WeaponCatalogError};
#[cfg(any(test, feature = "dev"))]
pub use starter::starter_definitions;
