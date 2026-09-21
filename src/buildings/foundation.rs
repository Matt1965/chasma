//! Derived foundation skirt mesh for level buildings (presentation only).

use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use crate::terrain::render_height;
use crate::world::{
    FoundationPerimeterVertex, FoundationSkirtSpec, FOUNDATION_SLOPE_DEGREES,
    FOUNDATION_TEXTURE_TILE_METERS, FOUNDATION_TERRAIN_PENETRATION_FUDGE_METERS,
    foundation_slope_run_per_meter_drop,
};

/// UV generation mode for foundation skirt bisection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundationUvMode {
    /// Constant per-vertex UVs (texture visibility without tiling).
    Trivial,
    /// World/local distance tiling.
    Tiled,
}

/// Build an anchor-local skirt mesh from a derived foundation specification.
pub fn build_foundation_skirt_mesh(
    spec: &FoundationSkirtSpec,
    anchor_world_y: f32,
    vertical_scale: f32,
    uv_mode: FoundationUvMode,
) -> Mesh {
    let anchor_render_y = render_height(anchor_world_y, vertical_scale);
    let floor_local_y = render_height(spec.floor_world_y, vertical_scale) - anchor_render_y;
    let run_per_drop = foundation_slope_run_per_meter_drop();

    let perimeter = &spec.perimeter;
    if perimeter.len() < 3 {
        return empty_mesh();
    }

    let centroid = perimeter
        .iter()
        .fold(Vec2::ZERO, |acc, v| acc + v.local_xz)
        / perimeter.len() as f32;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for i in 0..perimeter.len() {
        let a = perimeter[i];
        let b = perimeter[(i + 1) % perimeter.len()];
        let edge_len = a.local_xz.distance(b.local_xz);
        if edge_len < 1e-3 {
            continue;
        }
        let (top_a, bottom_a) = skirt_vertex_pair(
            &a,
            floor_local_y,
            anchor_render_y,
            vertical_scale,
            centroid,
            run_per_drop,
        );
        let (top_b, bottom_b) = skirt_vertex_pair(
            &b,
            floor_local_y,
            anchor_render_y,
            vertical_scale,
            centroid,
            run_per_drop,
        );
        let corner_uvs = match uv_mode {
            FoundationUvMode::Trivial => trivial_quad_uvs(),
            FoundationUvMode::Tiled => {
                skirt_wall_uvs(floor_local_y, top_a, top_b, bottom_a, bottom_b)
            }
        };
        push_quad(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut indices,
            corner_uvs,
            top_a,
            top_b,
            bottom_b,
            bottom_a,
        );
    }

    mesh_from_parts(positions, normals, uvs, indices)
}

/// Validate generated foundation mesh attributes for rendering.
pub fn validate_foundation_mesh(mesh: &Mesh) -> Result<(), String> {
    let positions = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .and_then(|attr| attr.as_float3())
        .ok_or("missing ATTRIBUTE_POSITION")?;
    if positions.is_empty() {
        return Err("zero vertices".to_string());
    }
    for pos in positions {
        if !pos[0].is_finite() || !pos[1].is_finite() || !pos[2].is_finite() {
            return Err(format!("non-finite position {:?}", pos));
        }
    }
    let uvs = mesh_uvs(mesh).ok_or("missing ATTRIBUTE_UV_0")?;
    if uvs.len() != positions.len() {
        return Err(format!(
            "uv count {} != vertex count {}",
            uvs.len(),
            positions.len()
        ));
    }
    for uv in uvs {
        if !uv[0].is_finite() || !uv[1].is_finite() {
            return Err(format!("non-finite uv {:?}", uv));
        }
    }
    let index_count = mesh
        .indices()
        .map(|indices| indices.len())
        .unwrap_or(0);
    if index_count == 0 {
        return Err("zero indices".to_string());
    }
    Ok(())
}

/// Structured mesh attribute presence for Stage B bisection logging.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundationMeshAttributeReport {
    pub vertex_count: usize,
    pub index_count: usize,
    pub has_position: bool,
    pub has_normal: bool,
    pub has_uv_0: bool,
    pub uv_count: usize,
    pub uv_matches_vertices: bool,
    pub uv_min: Option<[f32; 2]>,
    pub uv_max: Option<[f32; 2]>,
    pub all_uv_finite: bool,
}

/// Collect attribute presence and UV sanity for one foundation skirt mesh.
pub fn foundation_mesh_attribute_report(mesh: &Mesh) -> FoundationMeshAttributeReport {
    let positions = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .and_then(|attr| attr.as_float3());
    let vertex_count = positions.map(|p| p.len()).unwrap_or(0);
    let index_count = mesh.indices().map(|i| i.len()).unwrap_or(0);
    let has_position = positions.is_some();
    let has_normal = mesh
        .attribute(Mesh::ATTRIBUTE_NORMAL)
        .and_then(|attr| attr.as_float3())
        .is_some();
    let uvs = mesh_uvs(mesh);
    let has_uv_0 = uvs.is_some();
    let uv_count = uvs.map(|values| values.len()).unwrap_or(0);
    let uv_matches_vertices = uvs.is_some_and(|values| values.len() == vertex_count);
    let mut uv_min = [f32::INFINITY, f32::INFINITY];
    let mut uv_max = [f32::NEG_INFINITY, f32::NEG_INFINITY];
    let mut all_uv_finite = true;
    if let Some(values) = uvs {
        for uv in values {
            if !uv[0].is_finite() || !uv[1].is_finite() {
                all_uv_finite = false;
            }
            uv_min[0] = uv_min[0].min(uv[0]);
            uv_min[1] = uv_min[1].min(uv[1]);
            uv_max[0] = uv_max[0].max(uv[0]);
            uv_max[1] = uv_max[1].max(uv[1]);
        }
    } else {
        all_uv_finite = false;
    }
    FoundationMeshAttributeReport {
        vertex_count,
        index_count,
        has_position,
        has_normal,
        has_uv_0,
        uv_count,
        uv_matches_vertices,
        uv_min: has_uv_0.then_some(uv_min),
        uv_max: has_uv_0.then_some(uv_max),
        all_uv_finite,
    }
}

/// Human-readable mesh stats for runtime logging.
pub fn foundation_mesh_diagnostics(mesh: &Mesh) -> String {
    let positions = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .and_then(|attr| attr.as_float3())
        .map(|p| p.len())
        .unwrap_or(0);
    let indices = mesh.indices().map(|i| i.len()).unwrap_or(0);
    let aabb = mesh_aabb(mesh)
        .map(|(min, max)| format!("min={:?} max={:?}", min, max))
        .unwrap_or_else(|| "unavailable".to_string());
    let uv_range = mesh_uvs(mesh)
        .map(|uvs| {
            let mut min_u = f32::INFINITY;
            let mut min_v = f32::INFINITY;
            let mut max_u = f32::NEG_INFINITY;
            let mut max_v = f32::NEG_INFINITY;
            for uv in uvs {
                min_u = min_u.min(uv[0]);
                min_v = min_v.min(uv[1]);
                max_u = max_u.max(uv[0]);
                max_v = max_v.max(uv[1]);
            }
            format!("u=[{:.3},{:.3}] v=[{:.3},{:.3}]", min_u, max_u, min_v, max_v)
        })
        .unwrap_or_else(|| "uv missing".to_string());
    format!(
        "mesh vertex count: {}\n  mesh index count: {}\n  mesh AABB: {}\n  uv range: {}",
        positions,
        indices,
        aabb,
        uv_range
    )
}

fn mesh_uvs(mesh: &Mesh) -> Option<&[[f32; 2]]> {
    match mesh.attribute(Mesh::ATTRIBUTE_UV_0)? {
        VertexAttributeValues::Float32x2(values) => Some(values),
        _ => None,
    }
}

fn mesh_aabb(mesh: &Mesh) -> Option<(Vec3, Vec3)> {
    let positions = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .and_then(|attr| attr.as_float3())?;
    if positions.is_empty() {
        return None;
    }
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for pos in positions {
        let p = Vec3::from(*pos);
        min = min.min(p);
        max = max.max(p);
    }
    Some((min, max))
}

fn skirt_vertex_pair(
    vertex: &FoundationPerimeterVertex,
    floor_local_y: f32,
    anchor_render_y: f32,
    vertical_scale: f32,
    centroid: Vec2,
    run_per_drop: f32,
) -> (Vec3, Vec3) {
    let terrain_local_y =
        render_height(vertex.terrain_world_y, vertical_scale) - anchor_render_y;
    let vertical_drop = (floor_local_y - terrain_local_y).max(0.0);
    let outward = (vertex.local_xz - centroid).normalize_or_zero();
    let expansion = vertical_drop * run_per_drop;
    let bottom_xz = vertex.local_xz + outward * expansion;
    let top = Vec3::new(vertex.local_xz.x, floor_local_y, vertex.local_xz.y);
    let bottom = Vec3::new(
        bottom_xz.x,
        terrain_local_y - FOUNDATION_TERRAIN_PENETRATION_FUDGE_METERS,
        bottom_xz.y,
    );
    (top, bottom)
}

fn trivial_quad_uvs() -> [[f32; 2]; 4] {
    [[0.5, 0.5], [0.5, 0.5], [0.5, 0.5], [0.5, 0.5]]
}

/// UVs for one outward-facing skirt wall panel.
///
/// U is measured along the panel's top edge in anchor-local XZ (per-segment, not world-locked).
/// V is vertical height below the level floor. Outward slope expansion is handled by projecting
/// bottom corners onto the top-edge tangent so trapezoidal panels keep scale.
fn skirt_wall_uvs(
    floor_local_y: f32,
    top_a: Vec3,
    top_b: Vec3,
    bottom_a: Vec3,
    bottom_b: Vec3,
) -> [[f32; 2]; 4] {
    let tile = FOUNDATION_TEXTURE_TILE_METERS.max(0.01);
    let edge_xz = Vec2::new(top_b.x - top_a.x, top_b.z - top_a.z);
    let edge_len = edge_xz.length();
    if edge_len < 1e-6 {
        return trivial_quad_uvs();
    }
    let u_dir = edge_xz / edge_len;
    let u_at = |p: Vec3| -> f32 {
        let rel = Vec2::new(p.x - top_a.x, p.z - top_a.z);
        rel.dot(u_dir) / tile
    };
    let v_at = |p: Vec3| -> f32 { (floor_local_y - p.y).max(0.0) / tile };
    [
        [u_at(top_a), v_at(top_a)],
        [u_at(top_b), v_at(top_b)],
        [u_at(bottom_b), v_at(bottom_b)],
        [u_at(bottom_a), v_at(bottom_a)],
    ]
}

fn push_quad(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    corner_uvs: [[f32; 2]; 4],
    a: Vec3,
    b: Vec3,
    c: Vec3,
    d: Vec3,
) {
    let base = positions.len() as u32;
    for (p, uv) in [a, b, c, d].into_iter().zip(corner_uvs) {
        positions.push([p.x, p.y, p.z]);
        normals.push([0.0, 0.0, 0.0]);
        uvs.push(uv);
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
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
    mesh.compute_smooth_normals();
    mesh
}

fn empty_mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outward_slope_expansion_matches_drop_at_forty_five_degrees() {
        let spec = FoundationSkirtSpec {
            perimeter: vec![
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(0.0, -2.0),
                    terrain_world_y: 0.0,
                },
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(2.0, 0.0),
                    terrain_world_y: 0.0,
                },
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(0.0, 2.0),
                    terrain_world_y: 0.0,
                },
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(-2.0, 0.0),
                    terrain_world_y: -1.0,
                },
            ],
            floor_world_y: 2.0,
            max_depth_meters: 2.0,
            presentation_depth_meters: 2.0,
            outward_expansion_meters: 2.0,
        };
        let mesh = build_foundation_skirt_mesh(&spec, 2.0, 1.0, FoundationUvMode::Tiled);
        validate_foundation_mesh(&mesh).expect("valid foundation mesh");
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let min_z = positions.iter().map(|p| p[2]).fold(f32::INFINITY, f32::min);
        assert!(
            min_z < -2.4,
            "bottom perimeter should expand outward/downward (min_z={min_z})"
        );
        assert!((FOUNDATION_SLOPE_DEGREES - 45.0).abs() < 0.01);
    }

    #[test]
    fn skirt_wall_uvs_tile_perimeter_and_height_independently() {
        let tile = FOUNDATION_TEXTURE_TILE_METERS;
        let floor = tile;
        let top_a = Vec3::new(0.0, floor, 0.0);
        let top_b = Vec3::new(tile, floor, 0.0);
        let bottom_a = Vec3::new(0.0, 0.0, 0.0);
        let bottom_b = Vec3::new(tile, 0.0, 0.0);
        let uvs = skirt_wall_uvs(floor, top_a, top_b, bottom_a, bottom_b);
        assert!((uvs[0][0] - 0.0).abs() < 0.001);
        assert!((uvs[1][0] - 1.0).abs() < 0.001);
        assert!((uvs[3][1] - 1.0).abs() < 0.001);
    }

    #[test]
    fn skirt_wall_uvs_widen_u_span_when_bottom_expands_outward() {
        let floor = 2.0;
        let top_a = Vec3::new(0.0, floor, 0.0);
        let top_b = Vec3::new(2.0, floor, 0.0);
        let bottom_a = Vec3::new(-0.5, 0.0, 0.0);
        let bottom_b = Vec3::new(2.5, 0.0, 0.0);
        let uvs = skirt_wall_uvs(floor, top_a, top_b, bottom_a, bottom_b);
        let top_u_span = uvs[1][0] - uvs[0][0];
        let bottom_u_span = uvs[2][0] - uvs[3][0];
        assert!(
            bottom_u_span > top_u_span,
            "bottom edge UV span should exceed top when skirt expands outward"
        );
    }

    #[test]
    fn skirt_wall_uvs_uses_z_axis_when_wall_runs_along_z() {
        let floor = 3.0;
        let tile = FOUNDATION_TEXTURE_TILE_METERS;
        let top_a = Vec3::new(0.0, floor, 0.0);
        let top_b = Vec3::new(0.0, floor, tile);
        let bottom_a = Vec3::new(0.0, 0.0, 0.0);
        let bottom_b = Vec3::new(0.0, 0.0, tile);
        let uvs = skirt_wall_uvs(floor, top_a, top_b, bottom_a, bottom_b);
        assert!((uvs[1][0] - 1.0).abs() < 0.001);
    }

    #[test]
    fn skirt_wall_uvs_tiles_diagonal_wall_segment() {
        let floor = 3.0;
        let tile = FOUNDATION_TEXTURE_TILE_METERS;
        let top_a = Vec3::new(0.0, floor, 0.0);
        let top_b = Vec3::new(tile, floor, tile);
        let bottom_a = Vec3::new(0.0, 0.0, 0.0);
        let bottom_b = Vec3::new(tile, 0.0, tile);
        let uvs = skirt_wall_uvs(floor, top_a, top_b, bottom_a, bottom_b);
        let edge_len = (top_b - top_a).length();
        assert!((uvs[1][0] - edge_len / tile).abs() < 0.001);
        assert!((uvs[0][0]).abs() < 0.001);
    }

    #[test]
    fn hut_rectangle_perimeter_quads_all_have_uv_span() {
        let half = 2.0;
        let corners = [
            Vec2::new(-half, -half),
            Vec2::new(half, -half),
            Vec2::new(half, half),
            Vec2::new(-half, half),
        ];
        let step = 1.0;
        let mut local_points = Vec::new();
        for i in 0..4 {
            let a = corners[i];
            let b = corners[(i + 1) % 4];
            let edge = b - a;
            let len = edge.length();
            let count = ((len / step).ceil() as usize).max(1);
            for j in 0..=count {
                let t = j as f32 / count as f32;
                local_points.push(a + edge * t);
            }
        }
        let perimeter = local_points
            .iter()
            .map(|xz| FoundationPerimeterVertex {
                local_xz: *xz,
                terrain_world_y: 0.0,
            })
            .collect::<Vec<_>>();
        let spec = FoundationSkirtSpec {
            perimeter,
            floor_world_y: 1.5,
            max_depth_meters: 1.5,
            presentation_depth_meters: 1.5,
            outward_expansion_meters: 1.5,
        };
        let mesh = build_foundation_skirt_mesh(&spec, 0.0, 1.0, FoundationUvMode::Tiled);
        let uvs = mesh_uvs(&mesh).expect("uvs");
        assert_eq!(uvs.len() % 4, 0, "quads should use 4 verts each");
        for quad in uvs.chunks(4) {
            let min_u = quad.iter().map(|uv| uv[0]).fold(f32::INFINITY, f32::min);
            let max_u = quad.iter().map(|uv| uv[0]).fold(f32::NEG_INFINITY, f32::max);
            let min_v = quad.iter().map(|uv| uv[1]).fold(f32::INFINITY, f32::min);
            let max_v = quad.iter().map(|uv| uv[1]).fold(f32::NEG_INFINITY, f32::max);
            assert!(
                max_u - min_u > 0.05,
                "quad u span too small: {:?}",
                quad
            );
            assert!(
                max_v - min_v > 0.05,
                "quad v span too small: {:?}",
                quad
            );
        }
    }

    #[test]
    fn tiled_skirt_mesh_has_non_constant_uvs() {
        let spec = FoundationSkirtSpec {
            perimeter: vec![
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(-2.0, -2.0),
                    terrain_world_y: 0.0,
                },
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(2.0, -2.0),
                    terrain_world_y: 0.0,
                },
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(2.0, 2.0),
                    terrain_world_y: -0.5,
                },
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(-2.0, 2.0),
                    terrain_world_y: -0.5,
                },
            ],
            floor_world_y: 1.0,
            max_depth_meters: 1.0,
            presentation_depth_meters: 1.0,
            outward_expansion_meters: 1.0,
        };
        let mesh = build_foundation_skirt_mesh(&spec, 0.0, 1.0, FoundationUvMode::Tiled);
        let report = foundation_mesh_attribute_report(&mesh);
        assert!(report.has_uv_0);
        let (min, max) = (report.uv_min.unwrap(), report.uv_max.unwrap());
        assert!(max[0] - min[0] > 0.1, "u should span perimeter");
        assert!(max[1] - min[1] > 0.1, "v should span skirt height");
    }

    #[test]
    fn stage_b_trivial_uv_mesh_has_uv0_matching_vertices() {
        let spec = FoundationSkirtSpec {
            perimeter: vec![
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(-2.0, -2.0),
                    terrain_world_y: 0.0,
                },
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(2.0, -2.0),
                    terrain_world_y: 0.0,
                },
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(2.0, 2.0),
                    terrain_world_y: -0.5,
                },
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(-2.0, 2.0),
                    terrain_world_y: -0.5,
                },
            ],
            floor_world_y: 1.0,
            max_depth_meters: 1.0,
            presentation_depth_meters: 1.0,
            outward_expansion_meters: 1.0,
        };
        let mesh = build_foundation_skirt_mesh(&spec, 0.0, 1.0, FoundationUvMode::Trivial);
        let report = foundation_mesh_attribute_report(&mesh);
        assert!(report.has_position, "ATTRIBUTE_POSITION required");
        assert!(report.has_normal, "ATTRIBUTE_NORMAL required");
        assert!(report.has_uv_0, "Stage B requires ATTRIBUTE_UV_0");
        assert!(report.uv_matches_vertices, "UV count must match vertex count");
        assert!(report.all_uv_finite, "UVs must be finite");
        assert!(report.vertex_count > 0);
        assert!(report.index_count > 0);
    }

    #[test]
    fn foundation_mesh_has_finite_positions_and_matching_uvs() {
        let spec = FoundationSkirtSpec {
            perimeter: vec![
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(-2.0, -2.0),
                    terrain_world_y: 0.0,
                },
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(2.0, -2.0),
                    terrain_world_y: 0.0,
                },
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(2.0, 2.0),
                    terrain_world_y: -0.5,
                },
                FoundationPerimeterVertex {
                    local_xz: Vec2::new(-2.0, 2.0),
                    terrain_world_y: -0.5,
                },
            ],
            floor_world_y: 1.0,
            max_depth_meters: 1.0,
            presentation_depth_meters: 1.0,
            outward_expansion_meters: 1.0,
        };
        for mode in [FoundationUvMode::Trivial, FoundationUvMode::Tiled] {
            let mesh = build_foundation_skirt_mesh(&spec, 0.0, 1.0, mode);
            validate_foundation_mesh(&mesh).expect("mesh should validate");
        }
    }
}
