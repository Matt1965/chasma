//! Client-local view presentation resources (ADR-012, ADR-014).
//!
//! Generic view state shared across layers without coupling camera to terrain or
//! world. The app composition root bridges camera state into these resources.

mod focus;

pub use focus::PrimaryViewFocus;

use bevy::prelude::*;

/// Registers view presentation resources.
pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PrimaryViewFocus>();
    }
}
