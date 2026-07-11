//! Future unit spatial and affiliation query API (reserved â€” not implemented in U2).
//!
//! U3+ will expose chunk-local and radius queries without scanning the full world:
//!
//! - `units_near(position, radius)`
//! - `units_in_chunk(chunk)` â€” thin wrapper over [`crate::world::WorldData::units_in_chunk`]
//! - `nearest_unit(position)`
//! - `friendly_units_near(position, radius, affiliation)` / `enemy_units_near(...)`
//!
//! Friendly/enemy classification will use runtime `OwnerId` / `TeamId` /
//! `AffiliationId` on instance state â€” **not** catalog `faction_tag`.
//!
//! Implementation will read authoritative unit stores on [`crate::world::WorldData`],
//! not ECS render entities.
