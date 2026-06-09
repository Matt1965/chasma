use bevy::prelude::*;

use crate::world::WorldFoundationPlugin;

/// Composition root for the application.
///
/// `AppPlugin` is the single place where architectural layer plugins are
/// registered, in dependency order. It owns wiring only: no data and no
/// systems. Future layers (terrain, doodad, occupancy, rendering, gameplay,
/// simulation) are added here as they gain real content. See ADR-007.
pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WorldFoundationPlugin);
    }
}
