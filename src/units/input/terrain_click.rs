//! Render-space terrain click â†’ authoritative [`WorldPosition`] (ADR-033).

use bevy::prelude::*;

use crate::terrain::render_height;
use crate::world::{ground_world_position, ChunkLayout, WorldData, WorldPosition};

/// Maximum ray distance for terrain picking (meters).
const TERRAIN_RAY_MAX_DISTANCE: f32 = 5_000.0;

/// Coarse march steps to bracket a surface crossing.
const TERRAIN_RAY_COARSE_STEPS: usize = 256;

/// Binary-search iterations to refine the crossing distance along the ray.
const TERRAIN_RAY_REFINE_ITERATIONS: usize = 24;

/// Result of converting a cursor ray into an authoritative move target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainClickResult {
    /// Render-space global position where the ray met the exaggerated surface.
    pub render_hit: Vec3,
    /// Authoritative grounded target (heightfield Y, unscaled).
    pub world_position: WorldPosition,
}

/// Convert a camera ray into a grounded authoritative move target.
///
/// Render geometry is used only to find clicked X/Z. Authoritative Y always
/// comes from resident heightfield sampling via [`ground_world_position`].
pub fn terrain_click_to_world_position(
    ray: &Ray3d,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
) -> Option<TerrainClickResult> {
    let t = find_terrain_ray_hit_distance(ray, world, layout, vertical_scale)?;
    let render_hit = ray.get_point(t);
    let world_position = authoritative_position_at_global_xz(render_hit.x, render_hit.z, world, layout)?;
    Some(TerrainClickResult {
        render_hit,
        world_position,
    })
}

/// Map render/global XZ to a grounded authoritative [`WorldPosition`].
pub fn authoritative_position_at_global_xz(
    global_x: f32,
    global_z: f32,
    world: &WorldData,
    layout: ChunkLayout,
) -> Option<WorldPosition> {
    let candidate = WorldPosition::from_global(Vec3::new(global_x, 0.0, global_z), layout);
    ground_world_position(world, candidate)
}

fn find_terrain_ray_hit_distance(
    ray: &Ray3d,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
) -> Option<f32> {
    let step = TERRAIN_RAY_MAX_DISTANCE / TERRAIN_RAY_COARSE_STEPS as f32;
    let mut previous_t = 0.0;
    let mut previous_above = ray_height_error(ray, world, layout, vertical_scale, previous_t)? > 0.0;

    for step_index in 1..=TERRAIN_RAY_COARSE_STEPS {
        let t = step_index as f32 * step;
        let Some(error) = ray_height_error(ray, world, layout, vertical_scale, t) else {
            continue;
        };
        let above = error > 0.0;
        if previous_above && !above {
            return Some(refine_terrain_crossing(
                ray,
                world,
                layout,
                vertical_scale,
                previous_t,
                t,
            ));
        }
        previous_t = t;
        previous_above = above;
    }

    None
}

fn refine_terrain_crossing(
    ray: &Ray3d,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
    mut t_low: f32,
    mut t_high: f32,
) -> f32 {
    for _ in 0..TERRAIN_RAY_REFINE_ITERATIONS {
        let mid = (t_low + t_high) * 0.5;
        let above = ray_height_error(ray, world, layout, vertical_scale, mid)
            .map(|error| error > 0.0)
            .unwrap_or(false);
        if above {
            t_low = mid;
        } else {
            t_high = mid;
        }
    }
    (t_low + t_high) * 0.5
}

/// Positive when the ray point is above the render surface at the same X/Z.
fn ray_height_error(
    ray: &Ray3d,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
    distance: f32,
) -> Option<f32> {
    let point = ray.get_point(distance);
    let candidate = WorldPosition::from_global(Vec3::new(point.x, 0.0, point.z), layout);
    let height = world.sample_height_at_position(candidate)?;
    let surface_y = render_height(height, vertical_scale);
    Some(point.y - surface_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, ChunkData, ChunkId, Heightfield};

    fn sloped_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn flat_world(height: f32) -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = Heightfield::from_samples(3, 128.0, vec![height; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(1, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    #[test]
    fn authoritative_xz_matches_render_hit_xz() {
        let world = flat_world(12.0);
        let layout = world.layout();
        let ray = Ray3d {
            origin: Vec3::new(280.0, 200.0, 64.0),
            direction: Dir3::new(Vec3::new(0.0, -1.0, 0.0)).unwrap(),
        };
        let result = terrain_click_to_world_position(&ray, &world, layout, 3.0).unwrap();
        assert!((result.render_hit.x - result.world_position.to_global(layout).x).abs() < 0.05);
        assert!((result.render_hit.z - result.world_position.to_global(layout).z).abs() < 0.05);
    }

    #[test]
    fn vertical_scale_does_not_alter_authoritative_y() {
        let world = flat_world(12.0);
        let layout = world.layout();
        let ray = Ray3d {
            origin: Vec3::new(300.0, 200.0, 128.0),
            direction: Dir3::new(Vec3::new(0.0, -1.0, 0.0)).unwrap(),
        };
        let unscaled = terrain_click_to_world_position(&ray, &world, layout, 1.0).unwrap();
        let scaled = terrain_click_to_world_position(&ray, &world, layout, 4.0).unwrap();
        assert_eq!(unscaled.world_position, scaled.world_position);
        assert!((unscaled.world_position.local.0.y - 12.0).abs() < 1e-3);
    }

    #[test]
    fn authoritative_y_comes_from_heightfield_not_render_hit_y() {
        let world = sloped_world();
        let layout = world.layout();
        let ray = Ray3d {
            origin: Vec3::new(140.0, 300.0, 140.0),
            direction: Dir3::new(Vec3::new(0.0, -1.0, 0.0)).unwrap(),
        };
        let result = terrain_click_to_world_position(&ray, &world, layout, 2.0).unwrap();
        assert!(result.render_hit.y > result.world_position.local.0.y);
        assert!(result.world_position.local.0.y > 0.0);
    }

    #[test]
    fn chunk_local_conversion_is_consistent() {
        let world = flat_world(5.0);
        let layout = world.layout();
        let ray = Ray3d {
            origin: Vec3::new(280.0, 100.0, 64.0),
            direction: Dir3::new(Vec3::new(0.0, -1.0, 0.0)).unwrap(),
        };
        let result = terrain_click_to_world_position(&ray, &world, layout, 1.0).unwrap();
        assert_eq!(result.world_position.chunk, ChunkCoord::new(1, 0));
        assert!((result.world_position.local.0.x - 24.0).abs() < 0.1);
        assert!((result.world_position.local.0.z - 64.0).abs() < 0.1);
    }
}
