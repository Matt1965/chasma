use bevy::prelude::*;

/// Runtime configuration for the unit layer (ADR-028).
///
/// Reserved for future streaming and presentation toggles. U3 does not consume
/// additional fields beyond defaults.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Resource, Reflect)]
#[reflect(Resource)]
pub struct UnitsRuntimeSettings;
