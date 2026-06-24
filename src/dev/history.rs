//! Dev spawn history — session-local audit trail (ADR-047). Not simulation truth.

use std::collections::VecDeque;

use crate::world::WorldPosition;

use super::dev_mode::{DefinitionId, SpawnMode};

/// One dev-mode spawn event (WorldData mutation via catalog APIs).
#[derive(Debug, Clone, PartialEq)]
pub struct DevSpawnRecord {
    pub definition: DefinitionId,
    pub position: WorldPosition,
    pub spawn_type: SpawnMode,
    pub simulation_tick: u64,
}

/// Rolling in-memory spawn log for debugging and future undo/replay.
#[derive(Debug, Clone, PartialEq)]
pub struct DevSpawnHistory {
    pub entries: VecDeque<DevSpawnRecord>,
    pub max_entries: usize,
}

impl Default for DevSpawnHistory {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries: 256,
        }
    }
}

impl DevSpawnHistory {
    pub fn push(&mut self, record: DevSpawnRecord) {
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(record);
    }

    pub fn last(&self) -> Option<&DevSpawnRecord> {
        self.entries.back()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
