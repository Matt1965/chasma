//! Authoritative corpse records and lifecycle (ADR-089 I3).

mod authoring;
mod error;
mod id;
mod lifecycle;
mod record;
mod settings;
mod store;

pub use authoring::{
    corpse_lifetime_ticks, create_corpse_from_unit, remove_corpse_with_inventory,
    transfer_inventory_to_corpse,
};
pub use error::CorpseError;
pub use id::CorpseId;
#[cfg(feature = "dev")]
pub use lifecycle::dev_expire_corpse;
pub use lifecycle::{CorpseLifecycleReport, step_corpse_lifecycle};
pub use record::{CorpseRecord, CorpseState};
pub use settings::{CorpseSettings, DEFAULT_CORPSE_LIFETIME_TICKS};
pub use store::{ChunkCorpseStore, CorpseStore};
