//! Authoritative in-memory session snapshot (Chunk 1B).

use crate::world::WorldData;

/// Complete authoritative session state protected by load transactions.
///
/// This is a direct clone of runtime truth, not a dev-scene export. Scene
/// snapshots remain a separate, partial interchange format.
#[derive(Debug, Clone)]
pub struct AuthoritativeSessionSnapshot {
    pub world: WorldData,
    pub simulation_tick: u64,
}

impl AuthoritativeSessionSnapshot {
    pub fn capture(world: &WorldData, simulation_tick: u64) -> Self {
        Self {
            world: world.clone(),
            simulation_tick,
        }
    }

    pub fn restore_into(self, world: &mut WorldData, simulation_tick: &mut u64) {
        *world = self.world;
        *simulation_tick = self.simulation_tick;
    }
}
