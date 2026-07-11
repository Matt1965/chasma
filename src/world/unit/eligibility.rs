//! Shared unit execution eligibility (REVIEW-A4).
//!
//! Authoritative combat, movement, and order systems must agree on when a unit
//! may act. Centralizes dead / zero-HP / queued-for-removal checks.

use super::id::UnitId;
use super::record::UnitRecord;
use crate::world::WorldData;
use crate::world::is_unit_alive;

/// Whether a unit may perform authoritative simulation actions this tick.
pub fn unit_can_execute_actions(world: &WorldData, unit_id: UnitId) -> bool {
    world
        .get_unit(unit_id)
        .is_some_and(|record| unit_record_can_execute_actions(world, record))
}

/// Whether an existing unit record may act this tick.
pub fn unit_record_can_execute_actions(world: &WorldData, record: &UnitRecord) -> bool {
    if !is_unit_alive(record) {
        return false;
    }
    if world.removal_queue().is_queued(record.id) {
        return false;
    }
    true
}
