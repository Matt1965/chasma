//! Terrain height binding for the RTS orbit camera (ADR-014).
//!
//! Keeps the camera focus on the visible terrain surface and prevents the eye
//! from dipping below resident heightfield geometry. Reads [`WorldData`] for
//! presentation only — does not mutate simulation state.

use bevy::prelude::*;

use crate::terrain::render_height;
use crate::world::{ground_world_position, ChunkLayout, WorldConfig, WorldData, WorldPosition};

use super::components::RtsCameraState;
use super::control::orbit_transform;
use super::settings::CameraSettings;

/// Sample visible terrain Y at global XZ (render space).
pub fn render_terrain_height_at_global_xz(
    global_x: f32,
    global_z: f32,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
) -> Option<f32> {
    let candidate = WorldPosition::from_global(Vec3::new(global_x, 0.0, global_z), layout);
    let grounded = ground_world_position(world, candidate)?;
    Some(render_height(grounded.local.0.y, vertical_scale))
}

/// Glue orbit focus to terrain and clamp the camera eye above the surface.
pub fn apply_rts_camera_terrain_binding(
    state: &mut RtsCameraState,
    transform: &mut Transform,
    settings: &CameraSettings,
    world: &WorldData,
    config: &WorldConfig,
    vertical_scale: f32,
) {
    bind_focus_to_terrain(state, settings, world, config, vertical_scale);
    *transform = orbit_transform(state.focus, state.yaw, state.pitch, state.distance);
    clamp_camera_above_terrain(transform, settings, world, config, vertical_scale);
}

fn bind_focus_to_terrain(
    state: &mut RtsCameraState,
    settings: &CameraSettings,
    world: &WorldData,
    config: &WorldConfig,
    vertical_scale: f32,
) {
    let layout = config.chunk_layout();
    if let Some(terrain_y) =
        render_terrain_height_at_global_xz(state.target_focus.x, state.target_focus.z, world, layout, vertical_scale)
    {
        let y = terrain_y + settings.focus_terrain_offset;
        state.target_focus.y = y;
    }
    if let Some(terrain_y) =
        render_terrain_height_at_global_xz(state.focus.x, state.focus.z, world, layout, vertical_scale)
    {
        let y = terrain_y + settings.focus_terrain_offset;
        state.focus.y = y;
    }
}

fn clamp_camera_above_terrain(
    transform: &mut Transform,
    settings: &CameraSettings,
    world: &WorldData,
    config: &WorldConfig,
    vertical_scale: f32,
) {
    let layout = config.chunk_layout();
    let Some(terrain_y) = render_terrain_height_at_global_xz(
        transform.translation.x,
        transform.translation.z,
        world,
        layout,
        vertical_scale,
    ) else {
        return;
    };

    let min_y = terrain_y + settings.terrain_clearance;
    if transform.translation.y < min_y {
        transform.translation.y = min_y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, ChunkData, ChunkId, Heightfield, WorldConfig};

    fn flat_world(height: f32) -> WorldData {
        let mut world = WorldData::new(WorldConfig::default().chunk_layout());
        let heightfield = Heightfield::from_samples(3, 256.0, vec![height; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(1, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    #[test]
    fn render_height_matches_vertical_scale() {
        let world = flat_world(40.0);
        let layout = WorldConfig::default().chunk_layout();
        let y = render_terrain_height_at_global_xz(300.0, 100.0, &world, layout, 2.0).unwrap();
        assert!((y - 80.0).abs() < 1e-4);
    }

    #[test]
    fn focus_y_follows_terrain_surface() {
        let world = flat_world(25.0);
        let config = WorldConfig::default();
        let mut settings = CameraSettings::default();
        settings.focus_terrain_offset = 0.0;
        let mut state = RtsCameraState::new(Vec3::new(300.0, 0.0, 100.0), 0.0, 0.6, 200.0);
        let mut transform = Transform::IDENTITY;

        apply_rts_camera_terrain_binding(
            &mut state,
            &mut transform,
            &settings,
            &world,
            &config,
            1.0,
        );

        assert!((state.focus.y - 25.0).abs() < 1e-4);
        assert!((state.target_focus.y - 25.0).abs() < 1e-4);
    }

    #[test]
    fn camera_eye_is_clamped_above_terrain() {
        let world = flat_world(100.0);
        let config = WorldConfig::default();
        let mut settings = CameraSettings::default();
        settings.terrain_clearance = 12.0;
        settings.focus_terrain_offset = 0.0;
        let mut state = RtsCameraState::new(Vec3::new(300.0, 100.0, 100.0), 0.0, 0.2, 40.0);
        let mut transform = Transform::IDENTITY;

        apply_rts_camera_terrain_binding(
            &mut state,
            &mut transform,
            &settings,
            &world,
            &config,
            1.0,
        );

        assert!(transform.translation.y >= 112.0 - 1e-4);
    }

    #[test]
    fn missing_terrain_leaves_focus_y_unchanged() {
        let world = WorldData::new(WorldConfig::default().chunk_layout());
        let config = WorldConfig::default();
        let settings = CameraSettings::default();
        let mut state = RtsCameraState::new(Vec3::new(300.0, 15.0, 100.0), 0.0, 0.6, 200.0);
        let mut transform = Transform::IDENTITY;

        apply_rts_camera_terrain_binding(
            &mut state,
            &mut transform,
            &settings,
            &world,
            &config,
            1.0,
        );

        assert!((state.focus.y - 15.0).abs() < 1e-4);
    }
}
