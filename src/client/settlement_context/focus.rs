//! Camera view-direction focus point for settlement context.

use bevy::camera::Camera;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::terrain::TerrainRenderAssets;
use crate::units::input::terrain_click_to_world_position;
use crate::world::{ChunkLayout, WorldData, WorldPosition};

/// Build a world-space ray through the viewport center (camera view direction).
pub fn viewport_center_world_ray(
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera: &Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
) -> Option<Ray3d> {
    let window = windows.single().ok()?;
    let (camera, camera_transform) = camera.single().ok()?;
    let center = Vec2::new(window.width() * 0.5, window.height() * 0.5);
    camera.viewport_to_world(camera_transform, center).ok()
}

/// Derive the authoritative ground focus point from the camera view ray.
pub fn camera_view_focus_position(
    ray: &Ray3d,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
) -> Option<WorldPosition> {
    if let Some(click) = terrain_click_to_world_position(ray, world, layout, vertical_scale) {
        return Some(click.world_position);
    }
    ray_intersect_horizontal_plane(ray, 0.0)
        .map(|global| WorldPosition::from_global(global, layout))
}

fn ray_intersect_horizontal_plane(ray: &Ray3d, plane_y: f32) -> Option<Vec3> {
    let direction = ray.direction.as_vec3();
    if direction.y.abs() < 1e-6 {
        return None;
    }
    let t = (plane_y - ray.origin.y) / direction.y;
    if t <= 0.0 {
        return None;
    }
    Some(ray.origin + direction * t)
}

/// Convenience for systems: viewport center ray + terrain focus point.
pub fn derive_camera_focus_position(
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera: &Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    world: &WorldData,
    layout: ChunkLayout,
    render_assets: Option<&TerrainRenderAssets>,
) -> Option<WorldPosition> {
    let ray = viewport_center_world_ray(windows, camera)?;
    let vertical_scale = render_assets
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    camera_view_focus_position(&ray, world, layout, vertical_scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{RtsCameraState, orbit_position, orbit_transform};
    use crate::world::{
        ChunkCoord, ChunkData, ChunkLayout, Heightfield, LocalPosition, SettlementKind,
        SettlementOwnership, WorldData, WorldPosition, create_settlement,
    };
    use bevy::prelude::Vec3;

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(layout());
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(
            crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    #[test]
    fn view_ray_focus_can_land_in_settlement_while_camera_position_is_outside() {
        let world = flat_world();
        let settlement_center = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(160.0, 0.0, 160.0)),
        );
        let mut world_with_settlement = world;
        create_settlement(
            &mut world_with_settlement,
            settlement_center,
            "A",
            SettlementOwnership::player_default(),
            SettlementKind::Town,
            Some(48.0),
            None,
            0,
        )
        .expect("settlement");

        let focus = Vec3::new(160.0, 0.0, 160.0);
        let state = RtsCameraState::new(Vec3::new(40.0, 0.0, 160.0), 0.0, 0.45, 120.0);
        let transform = orbit_transform(state.focus, state.yaw, state.pitch, state.distance);
        let camera_pos = orbit_position(state.focus, state.yaw, state.pitch, state.distance);
        assert!(camera_pos.distance(focus) > 48.0);

        let direction = (focus - transform.translation).normalize();
        let ray = Ray3d {
            origin: transform.translation,
            direction: Dir3::new(direction).expect("direction"),
        };
        let hit =
            camera_view_focus_position(&ray, &world_with_settlement, layout(), 1.0).expect("focus");
        let hit_global = hit.to_global(layout());
        assert!((hit_global.x - focus.x).abs() < 2.0);
        assert!((hit_global.z - focus.z).abs() < 2.0);
    }

    #[test]
    fn view_direction_not_camera_xz_position_picks_different_settlement() {
        let mut world = flat_world();
        create_settlement(
            &mut world,
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(64.0, 0.0, 64.0)),
            ),
            "A",
            SettlementOwnership::player_default(),
            SettlementKind::Town,
            Some(48.0),
            None,
            0,
        )
        .expect("A");
        create_settlement(
            &mut world,
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(220.0, 0.0, 64.0)),
            ),
            "B",
            SettlementOwnership::player_default(),
            SettlementKind::Town,
            Some(48.0),
            None,
            0,
        )
        .expect("B");

        let camera_pos = Vec3::new(220.0, 80.0, 20.0);
        let look_at = Vec3::new(64.0, 0.0, 64.0);
        let direction = (look_at - camera_pos).normalize();
        let ray = Ray3d {
            origin: camera_pos,
            direction: Dir3::new(direction).expect("direction"),
        };
        let focus = camera_view_focus_position(&ray, &world, layout(), 1.0).expect("focus");
        let focus_xz = focus.to_global(layout());
        assert!((focus_xz.x - 64.0).abs() < 4.0);
        assert!((focus_xz.z - 64.0).abs() < 4.0);
        assert!((camera_pos.x - 64.0).abs() > 48.0);
    }
}
