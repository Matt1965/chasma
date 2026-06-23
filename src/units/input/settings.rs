use bevy::prelude::*;

/// Client-local player interaction settings (ADR-033 U8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource, Reflect)]
#[reflect(Resource)]
pub struct PlayerInteractionSettings {
    /// Log terrain click conversion and issued move paths when enabled.
    pub debug_unit_interaction: bool,
    /// Log client intents and dispatch results each frame when enabled.
    pub debug_intents: bool,
}

impl Default for PlayerInteractionSettings {
    fn default() -> Self {
        Self {
            debug_unit_interaction: false,
            debug_intents: false,
        }
    }
}
