use bevy::prelude::*;

/// Authoritative task instance identifier (ADR-085 B8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct TaskId(pub u32);

impl TaskId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}
