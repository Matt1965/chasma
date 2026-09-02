//! Terrain-conforming selection ring mesh (DV3 presentation).

use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::camera::render_terrain_height_at_global_xz;
use crate::world::{ChunkLayout, UnitCatalog, UnitId, WorldData};

/// Ring segment count — enough for smooth slopes without heavy cost.
pub const SELECTION_RING_SEGMENTS: usize = 32;
const RING_LIFT_METERS: f32 = 0.06;

/// Build an annulus mesh in parent-local space with vertices draped on terrain.
pub fn build_terrain_selection_ring_mesh(
    parent_global: Vec3,
    inner_radius: f32,
    outer_radius: f32,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
) -> Mesh {
    let parent_y = render_terrain_height_at_global_xz(
        parent_global.x,
        parent_global.z,
        world,
        layout,
        vertical_scale,
    )
    .unwrap_or(parent_global.y);

    let mut positions = Vec::with_capacity(SELECTION_RING_SEGMENTS * 2);
    let mut normals = Vec::with_capacity(SELECTION_RING_SEGMENTS * 2);
    let mut uvs = Vec::with_capacity(SELECTION_RING_SEGMENTS * 2);
    let mut indices = Vec::with_capacity(SELECTION_RING_SEGMENTS * 6);

    for i in 0..SELECTION_RING_SEGMENTS {
        let angle = (i as f32 / SELECTION_RING_SEGMENTS as f32) * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();

        for (radius, v) in [(inner_radius, 0.0), (outer_radius, 1.0)] {
            let world_x = parent_global.x + cos * radius;
            let world_z = parent_global.z + sin * radius;
            let terrain_y =
                render_terrain_height_at_global_xz(world_x, world_z, world, layout, vertical_scale)
                    .unwrap_or(parent_y);
            let local_y = terrain_y - parent_y + RING_LIFT_METERS;
            positions.push([cos * radius, local_y, sin * radius]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([v, i as f32 / SELECTION_RING_SEGMENTS as f32]);
        }
    }

    for i in 0..SELECTION_RING_SEGMENTS {
        let next = (i + 1) % SELECTION_RING_SEGMENTS;
        let inner = (i * 2) as u32;
        let outer = inner + 1;
        let inner_next = (next * 2) as u32;
        let outer_next = inner_next + 1;
        indices.extend_from_slice(&[inner, outer, outer_next, inner, outer_next, inner_next]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// World-space render positions along a terrain-conforming ring (outer edge).
pub fn sample_terrain_ring_render_points(
    center_render: Vec3,
    radius: f32,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
    segments: usize,
) -> Vec<Vec3> {
    let center_y = render_terrain_height_at_global_xz(
        center_render.x,
        center_render.z,
        world,
        layout,
        vertical_scale,
    )
    .unwrap_or(center_render.y);

    let mut points = Vec::with_capacity(segments);
    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let world_x = center_render.x + cos * radius;
        let world_z = center_render.z + sin * radius;
        let terrain_y =
            render_terrain_height_at_global_xz(world_x, world_z, world, layout, vertical_scale)
                .unwrap_or(center_y);
        points.push(Vec3::new(world_x, terrain_y + RING_LIFT_METERS, world_z));
    }
    points
}

/// Draw a closed terrain ring loop via gizmos.
pub fn draw_terrain_ring_gizmos(gizmos: &mut Gizmos, points: &[Vec3], color: Color) {
    if points.len() < 2 {
        return;
    }
    for i in 0..points.len() {
        let next = (i + 1) % points.len();
        gizmos.line(points[i], points[next], color);
    }
}

/// Shared selection ring radius from collision footprint (DV3).
pub fn selection_ring_radius(world: &WorldData, catalog: &UnitCatalog, unit_id: UnitId) -> f32 {
    let Some(record) = world.get_unit(unit_id) else {
        return 1.0;
    };
    let Some(definition) = catalog.get(&record.definition_id) else {
        return 1.0;
    };
    (definition.collision_radius_meters * 2.0).max(0.9)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, ChunkData, ChunkId, Heightfield, LocalPosition, WorldPosition};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn sloped_world() -> WorldData {
        let mut world = WorldData::new(layout());
        let samples = vec![0.0, 2.0, 4.0, 2.0, 6.0, 8.0, 4.0, 8.0, 10.0];
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    #[test]
    fn terrain_ring_mesh_has_expected_topology() {
        let world = sloped_world();
        let mesh = build_terrain_selection_ring_mesh(
            Vec3::new(64.0, 0.0, 64.0),
            0.7,
            1.0,
            &world,
            layout(),
            1.0,
        );
        let positions: Vec<[f32; 3]> = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .expect("positions")
            .as_float3()
            .expect("float3")
            .to_vec();
        assert_eq!(positions.len(), SELECTION_RING_SEGMENTS * 2);
        assert_eq!(
            mesh.indices().expect("indices").len(),
            SELECTION_RING_SEGMENTS * 6
        );
    }

    #[test]
    fn ring_vertices_follow_uneven_height() {
        let world = sloped_world();
        let mesh = build_terrain_selection_ring_mesh(
            Vec3::new(64.0, 0.0, 64.0),
            0.7,
            1.0,
            &world,
            layout(),
            1.0,
        );
        let positions: Vec<[f32; 3]> = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap()
            .to_vec();
        let ys: Vec<f32> = positions.iter().map(|p| p[1]).collect();
        let min_y = ys.iter().copied().fold(f32::INFINITY, f32::min);
        let max_y = ys.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(max_y - min_y > 0.01);
    }
}
