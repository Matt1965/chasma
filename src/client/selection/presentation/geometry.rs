//! Selection outline mesh builders (Slice 2).

use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::player::selection_ring_mesh::{
    SELECTION_RING_SEGMENTS, build_terrain_selection_ring_mesh,
};
use crate::world::FootprintShape;
use crate::world::{ChunkLayout, WorldData};

use super::resolve::ResolvedSelectionFootprint;

pub const OUTLINE_LIFT_METERS: f32 = 0.06;
pub const OUTLINE_LINE_WIDTH_METERS: f32 = 0.12;
const ELLIPSE_SEGMENTS: usize = 32;

/// Standard Chasma selection green (matches unit rings).
pub const SELECTION_GREEN: Color = Color::srgba(0.15, 0.95, 0.25, 0.85);
pub const SELECTION_GREEN_PRIMARY: Color = Color::srgba(0.15, 0.95, 0.25, 0.95);

/// Build or rebuild the outline mesh for a resolved footprint.
pub fn build_selection_outline_mesh(
    footprint: &ResolvedSelectionFootprint,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
) -> Mesh {
    if footprint.terrain_conforming {
        return build_terrain_conforming_circle_mesh(footprint, world, layout, vertical_scale);
    }
    build_authoritative_footprint_mesh(&footprint.shape, footprint.yaw_radians)
}

fn build_terrain_conforming_circle_mesh(
    footprint: &ResolvedSelectionFootprint,
    world: &WorldData,
    layout: ChunkLayout,
    vertical_scale: f32,
) -> Mesh {
    let FootprintShape::Circle { radius_meters } = footprint.shape else {
        return build_flat_annulus_mesh(0.5, 0.62, SELECTION_RING_SEGMENTS, OUTLINE_LIFT_METERS);
    };
    let outer = radius_meters;
    let inner = (outer * 0.82).max(outer - OUTLINE_LINE_WIDTH_METERS);
    build_terrain_selection_ring_mesh(
        footprint.anchor_render,
        inner,
        outer,
        world,
        layout,
        vertical_scale,
    )
}

fn build_authoritative_footprint_mesh(shape: &FootprintShape, yaw_radians: f32) -> Mesh {
    match shape {
        FootprintShape::Circle { radius_meters } => {
            let outer = *radius_meters;
            let inner = (outer * 0.82).max(outer - OUTLINE_LINE_WIDTH_METERS);
            build_flat_annulus_mesh(inner, outer, SELECTION_RING_SEGMENTS, OUTLINE_LIFT_METERS)
        }
        FootprintShape::Ellipse {
            radius_x_meters,
            radius_z_meters,
        } => build_flat_ellipse_annulus_mesh(
            *radius_x_meters,
            *radius_z_meters,
            0.82,
            OUTLINE_LINE_WIDTH_METERS,
            ELLIPSE_SEGMENTS,
            OUTLINE_LIFT_METERS,
        ),
        FootprintShape::Rectangle {
            width_meters,
            depth_meters,
        } => build_rect_frame_mesh(
            *width_meters,
            *depth_meters,
            OUTLINE_LINE_WIDTH_METERS,
            OUTLINE_LIFT_METERS,
        ),
        FootprintShape::BakedCellMask(mask) => build_baked_mask_frame_mesh(
            mask,
            yaw_radians,
            OUTLINE_LINE_WIDTH_METERS,
            OUTLINE_LIFT_METERS,
        ),
    }
}

/// Flat annulus in the XZ plane (local space; entity transform supplies world pose).
pub fn build_flat_annulus_mesh(
    inner_radius: f32,
    outer_radius: f32,
    segments: usize,
    lift: f32,
) -> Mesh {
    let mut positions = Vec::with_capacity(segments * 2);
    let mut normals = Vec::with_capacity(segments * 2);
    let mut uvs = Vec::with_capacity(segments * 2);
    let mut indices = Vec::with_capacity(segments * 6);

    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        for (radius, v) in [(inner_radius, 0.0), (outer_radius, 1.0)] {
            positions.push([cos * radius, lift, sin * radius]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([v, i as f32 / segments as f32]);
        }
    }

    for i in 0..segments {
        let next = (i + 1) % segments;
        let inner = (i * 2) as u32;
        let outer = inner + 1;
        let inner_next = (next * 2) as u32;
        let outer_next = inner_next + 1;
        indices.extend_from_slice(&[inner, outer, outer_next, inner, outer_next, inner_next]);
    }

    mesh_from_parts(positions, normals, uvs, indices)
}

fn build_flat_ellipse_annulus_mesh(
    radius_x: f32,
    radius_z: f32,
    inner_scale: f32,
    min_thickness: f32,
    segments: usize,
    lift: f32,
) -> Mesh {
    let inner_x = (radius_x * inner_scale).max(radius_x - min_thickness);
    let inner_z = (radius_z * inner_scale).max(radius_z - min_thickness);
    let mut positions = Vec::with_capacity(segments * 2);
    let mut normals = Vec::with_capacity(segments * 2);
    let mut uvs = Vec::with_capacity(segments * 2);
    let mut indices = Vec::with_capacity(segments * 6);

    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        for (rx, rz, v) in [(inner_x, inner_z, 0.0), (radius_x, radius_z, 1.0)] {
            positions.push([cos * rx, lift, sin * rz]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([v, i as f32 / segments as f32]);
        }
    }

    for i in 0..segments {
        let next = (i + 1) % segments;
        let inner = (i * 2) as u32;
        let outer = inner + 1;
        let inner_next = (next * 2) as u32;
        let outer_next = inner_next + 1;
        indices.extend_from_slice(&[inner, outer, outer_next, inner, outer_next, inner_next]);
    }

    mesh_from_parts(positions, normals, uvs, indices)
}

/// Rectangular frame in local XZ centered on the anchor.
pub fn build_rect_frame_mesh(width: f32, depth: f32, line_width: f32, lift: f32) -> Mesh {
    let half_w = width * 0.5;
    let half_d = depth * 0.5;
    let t = line_width * 0.5;
    let y = lift;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let mut quad = |a: Vec3, b: Vec3, c: Vec3, d: Vec3| {
        let base = positions.len() as u32;
        for p in [a, b, c, d] {
            positions.push([p.x, p.y, p.z]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([0.0, 0.0]);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };

    // Top edge (+Z)
    quad(
        Vec3::new(-half_w, y, half_d - t),
        Vec3::new(half_w, y, half_d - t),
        Vec3::new(half_w, y, half_d + t),
        Vec3::new(-half_w, y, half_d + t),
    );
    // Bottom edge (-Z)
    quad(
        Vec3::new(-half_w, y, -half_d - t),
        Vec3::new(half_w, y, -half_d - t),
        Vec3::new(half_w, y, -half_d + t),
        Vec3::new(-half_w, y, -half_d + t),
    );
    // Right edge (+X)
    quad(
        Vec3::new(half_w - t, y, -half_d),
        Vec3::new(half_w + t, y, -half_d),
        Vec3::new(half_w + t, y, half_d),
        Vec3::new(half_w - t, y, half_d),
    );
    // Left edge (-X)
    quad(
        Vec3::new(-half_w - t, y, -half_d),
        Vec3::new(-half_w + t, y, -half_d),
        Vec3::new(-half_w + t, y, half_d),
        Vec3::new(-half_w - t, y, half_d),
    );

    mesh_from_parts(positions, normals, uvs, indices)
}

fn build_baked_mask_frame_mesh(
    mask: &crate::world::BakedCellMask,
    yaw_radians: f32,
    line_width: f32,
    lift: f32,
) -> Mesh {
    use std::collections::HashSet;

    let cell_size = mask.cell_size_meters;
    let mut blocked = HashSet::new();
    for z in 0..mask.depth_cells {
        for x in 0..mask.width_cells {
            if mask.is_blocked_local(x as i32, z as i32) {
                blocked.insert((x, z));
            }
        }
    }

    let (sin, cos) = yaw_radians.sin_cos();
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let mut segment = |from: Vec2, to: Vec2| {
        let dir = (to - from).normalize_or_zero();
        let perp = Vec2::new(-dir.y, dir.x) * (line_width * 0.5);
        let a = from - perp;
        let b = from + perp;
        let c = to + perp;
        let d = to - perp;
        let base = positions.len() as u32;
        for local in [a, b, c, d] {
            let rotated = Vec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos);
            positions.push([rotated.x, lift, rotated.y]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([0.0, 0.0]);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };

    for &(x, z) in &blocked {
        let ox = mask.local_origin.x + x as f32 * cell_size;
        let oz = mask.local_origin.y + z as f32 * cell_size;
        let east = (x + 1, z);
        let north = (x, z + 1);
        if x + 1 >= mask.width_cells || !blocked.contains(&east) {
            segment(
                Vec2::new(ox + cell_size, oz),
                Vec2::new(ox + cell_size, oz + cell_size),
            );
        }
        if z + 1 >= mask.depth_cells || !blocked.contains(&north) {
            segment(
                Vec2::new(ox + cell_size, oz + cell_size),
                Vec2::new(ox, oz + cell_size),
            );
        }
    }

    if positions.is_empty() {
        return build_flat_annulus_mesh(0.4, 0.55, SELECTION_RING_SEGMENTS, lift);
    }

    mesh_from_parts(positions, normals, uvs, indices)
}

fn mesh_from_parts(
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
) -> Mesh {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::BakedCellMask;

    #[test]
    fn rect_frame_has_triangles() {
        let mesh = build_rect_frame_mesh(4.0, 6.0, 0.1, 0.05);
        assert!(mesh.indices().unwrap().len() >= 24);
    }

    #[test]
    fn circle_annulus_has_expected_vertex_count() {
        let mesh = build_flat_annulus_mesh(0.8, 1.0, 16, 0.06);
        let positions: Vec<[f32; 3]> = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap()
            .to_vec();
        assert_eq!(positions.len(), 32);
    }

    #[test]
    fn baked_mask_frame_non_empty_for_single_cell() {
        let mask = BakedCellMask {
            cell_size_meters: 1.0,
            width_cells: 1,
            depth_cells: 1,
            local_origin: Vec2::ZERO,
            blocked_cells: [0].into_iter().collect(),
            forced_open_cells: Default::default(),
            forced_blocked_cells: Default::default(),
            space_id: 0,
        };
        let mesh = build_baked_mask_frame_mesh(&mask, 0.0, 0.1, 0.05);
        assert!(!mesh.indices().unwrap().is_empty());
    }
}
