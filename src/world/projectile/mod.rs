//! Authoritative projectile simulation (ADR-060 C7).

mod id;
mod record;
mod report;
mod simulation;

pub use id::ProjectileId;
pub use record::{ProjectileRecord, ProjectileStatus};
pub use report::{ProjectileEvent, ProjectileReport, ProjectileTrace};
pub use simulation::{spawn_projectile_from_strike, step_all_projectiles};
