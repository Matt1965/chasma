use bevy::prelude::*;

/// Default world seed for future procedural doodad materialization (ADR-023).
pub const DEFAULT_DOODAD_WORLD_SEED: u64 = 0x0045_4A5_5EED;

/// Runtime configuration for the doodad layer (ADR-023).
///
/// `world_seed` drives dev procedural materialization (ADR-018/019) when the
/// `dev` feature is enabled. Production streaming may consume this later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource, Reflect)]
#[reflect(Resource)]
pub struct DoodadsRuntimeSettings {
    pub world_seed: u64,
}

impl Default for DoodadsRuntimeSettings {
    fn default() -> Self {
        Self {
            world_seed: DEFAULT_DOODAD_WORLD_SEED,
        }
    }
}
