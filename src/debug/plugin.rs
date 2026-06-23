//! Debug overlay plugin registration.

use bevy::prelude::*;

use crate::player::PlayerControlSystems;

use super::boundaries::advance_client_frame_index;
use super::dispatch_pending::PendingDispatchTrace;
use super::pending::PendingSimulationTrace;
use super::settings::DebugOverlaySettings;
use super::trace::{ClientFrameIndex, CommandTraceBuffer, IntentDispatchHistory};

/// Registers command trace resources and the client frame counter.
pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommandTraceBuffer>()
            .init_resource::<ClientFrameIndex>()
            .init_resource::<IntentDispatchHistory>()
            .init_resource::<DebugOverlaySettings>()
            .init_resource::<PendingDispatchTrace>()
            .init_resource::<PendingSimulationTrace>()
            .init_resource::<super::boundaries::ClientBoundaryGuard>()
            .register_type::<DebugOverlaySettings>()
            .add_systems(
                Update,
                advance_client_frame_index
                    .before(crate::player::tick_unit_movement)
                    .in_set(PlayerControlSystems),
            );
    }
}
