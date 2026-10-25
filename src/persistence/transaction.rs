//! Transactional mutation of authoritative session state (Chunk 1B).

use super::session::AuthoritativeSessionSnapshot;
use crate::world::WorldData;

/// Operation failed after the transaction opened; session was rolled back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRolledBack<E> {
    pub error: E,
}

/// Runs `operation` against `world` inside a full-session transaction.
///
/// On any error, restores the prior `WorldData` and `simulation_tick` from a
/// clone taken before the operation begins. Success leaves mutations in place.
///
/// Derived ECS/presentation state is not part of the snapshot; consumers must
/// resync after a successful load (existing runtime sync systems).
pub fn transactional_session_load<E, T>(
    world: &mut WorldData,
    simulation_tick: &mut u64,
    operation: impl FnOnce(&mut WorldData) -> Result<T, E>,
) -> Result<T, TransactionRolledBack<E>> {
    let snapshot = AuthoritativeSessionSnapshot::capture(world, *simulation_tick);
    match operation(world) {
        Ok(value) => Ok(value),
        Err(error) => {
            snapshot.restore_into(world, simulation_tick);
            Err(TransactionRolledBack { error })
        }
    }
}
