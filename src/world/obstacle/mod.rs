//! Doodad obstacle queries against authoritative world data (ADR-031).
//!
//! Movement blocking reads [`crate::world::DoodadRecord`] stores and
//! [`crate::world::DoodadCatalog`] definitions only — never ECS render entities.

mod query;

pub use query::{blocking_doodad_at_position, is_position_blocked_by_doodads};
