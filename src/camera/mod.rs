//! Camera layer (ADR-014).
//!
//! Owns client-local RTS orbit camera presentation: input, smoothing, and the
//! main `Camera3d` entity. Terrain height binding reads [`WorldData`] for
//! presentation only and does not mutate authoritative simulation state.

use bevy::prelude::*;

mod components;
mod control;
mod settings;
mod setup;
mod terrain_bind;

pub use components::{RtsCamera, RtsCameraState};
pub use control::{CameraControlSystems, orbit_position, orbit_transform};
pub use settings::CameraSettings;
pub use terrain_bind::render_terrain_height_at_global_xz;

/// Owns the Camera layer (ADR-014).
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CameraSettings>()
            .register_type::<RtsCamera>()
            .register_type::<RtsCameraState>()
            .init_resource::<CameraSettings>()
            .add_systems(Startup, setup::spawn_rts_camera)
            .configure_sets(Update, CameraControlSystems)
            .add_systems(
                Update,
                control::apply_rts_camera_control.in_set(CameraControlSystems),
            );
    }
}
