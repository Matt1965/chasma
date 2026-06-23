//! Pending simulation-side trace events flushed after movement tick.

use bevy::prelude::*;

use crate::world::CommandBufferResolveReport;

/// Resolve report awaiting trace flush (simulation writes, debug reads).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct PendingSimulationTrace {
    pub resolve: Option<CommandBufferResolveReport>,
}
