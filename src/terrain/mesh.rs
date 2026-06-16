//! Pure terrain mesh generation (ADR-013).
//!
//! [`build_chunk_mesh`] is a pure function: it consumes a [`Heightfield`] and
//! produces a Bevy [`Mesh`]. It does not read the ECS, does not touch
//! [`crate::world::WorldData`], and does not require a running renderer, so it is
//! fully unit-testable. Vertices are emitted in the chunk's local space (origin
//! at the chunk minimum corner); the spawning system places the chunk in the
//! world via its `Transform` (ADR-001 minimum-corner origin).

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::world::Heightfield;

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
static BUILD_MESH_CALLS: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
pub(crate) fn test_reset_build_mesh_calls() {
    BUILD_MESH_CALLS.store(0, Ordering::SeqCst);
}

#[cfg(test)]
pub(crate) fn test_build_mesh_call_count() -> usize {
    BUILD_MESH_CALLS.load(Ordering::SeqCst)
}

/// Mesh level of detail for a chunk.
///
/// Phase 2A emits a single full-resolution level. Subsampled levels, distance
/// selection, and skirts are deferred to Phase 2C (ADR-013), but the parameter
/// exists now so the builder signature is stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkLod {
    /// One mesh vertex per heightfield sample.
    Full,
}

/// Neighbor height strips used for mesh positions and cross-chunk normals.
#[derive(Debug, Clone, Default)]
pub struct ChunkMeshSeamWeld {
    /// −X neighbor +X edge (`col == N`), welded at `col == 0`.
    pub west_edge: Option<Vec<f32>>,
    /// −Z neighbor +Z edge (`row == N`), welded at `row == 0`.
    pub south_edge: Option<Vec<f32>>,
    /// +X neighbor `col == 1`, for normals at this chunk's +X edge.
    pub east_interior: Option<Vec<f32>>,
    /// +Z neighbor `row == 1`, for normals at this chunk's +Z edge.
    pub north_interior: Option<Vec<f32>>,
    /// −X neighbor `col == N - 1`, for normals at `col == 0`.
    pub west_interior: Option<Vec<f32>>,
    /// −Z neighbor `row == N - 1`, for normals at `row == 0`.
    pub south_interior: Option<Vec<f32>>,
}

/// Interior samples to linearly ramp toward the stitched +X / +Z boundary.
const NON_OVERLAP_EDGE_RAMP_SAMPLES: usize = 0;

/// Linearly ramp the last few interior columns/rows toward the stitched boundary.
///
/// Non-overlapping Gaea tiles store the neighbor boundary at `col == N` while
/// the local tile's trailing samples often disagree one meter inside, leaving a
/// steep 1 m ditch along every chunk edge. Mesh generation re-slopes that strip
/// toward the boundary without mutating authoritative [`Heightfield`] data.
fn repair_non_overlap_edge_slopes(heights: &mut [f32], spe: usize) {
    if spe < 2 {
        return;
    }
    let last = spe - 1;
    let ramp_len = last.min(NON_OVERLAP_EDGE_RAMP_SAMPLES);
    if ramp_len < 2 {
        return;
    }
    let ramp_start = last - ramp_len;

    for row in 0..last {
        let base = row * spe;
        let hi = heights[base + ramp_start];
        let hb = heights[base + last];
        for step in 1..ramp_len {
            let t = step as f32 / ramp_len as f32;
            heights[base + ramp_start + step] = hi + (hb - hi) * t;
        }
    }

    for col in 0..ramp_start {
        let hi = heights[ramp_start * spe + col];
        let hb = heights[last * spe + col];
        for step in 1..ramp_len {
            let t = step as f32 / ramp_len as f32;
            heights[(ramp_start + step) * spe + col] = hi + (hb - hi) * t;
        }
    }
}

fn build_mesh_height_grid(
    samples: &[f32],
    spe: usize,
    seam_weld: &ChunkMeshSeamWeld,
) -> Vec<f32> {
    let mut heights = samples.to_vec();

    for row in 0..spe {
        for col in 0..spe {
            let idx = row * spe + col;
            if col == 0 {
                if let Some(west) = seam_weld.west_edge.as_ref().and_then(|strip| strip.get(row)) {
                    heights[idx] = *west;
                }
            }
            if row == 0 {
                if let Some(south) = seam_weld.south_edge.as_ref().and_then(|strip| strip.get(col))
                {
                    heights[idx] = *south;
                }
            }
        }
    }

    repair_non_overlap_edge_slopes(&mut heights, spe);
    heights
}

fn normal_stencil_x(
    row: usize,
    col: usize,
    spe: usize,
    heights: &[f32],
    h: f32,
    scale: f32,
    spacing: f32,
    seam_weld: &ChunkMeshSeamWeld,
) -> (f32, f32, f32) {
    let sample = |row: usize, col: usize| heights[row * spe + col];
    if col == 0 {
        if let Some(west) = seam_weld
            .west_interior
            .as_ref()
            .and_then(|strip| strip.get(row))
        {
            return (*west * scale, sample(row, col + 1) * scale, 2.0 * spacing);
        }
        return (h, sample(row, col + 1) * scale, spacing);
    }
    if col == spe - 1 {
        if let Some(east) = seam_weld
            .east_interior
            .as_ref()
            .and_then(|strip| strip.get(row))
        {
            return (
                sample(row, col - 1) * scale,
                *east * scale,
                2.0 * spacing,
            );
        }
        return (sample(row, col - 1) * scale, h, spacing);
    }
    (
        sample(row, col - 1) * scale,
        sample(row, col + 1) * scale,
        2.0 * spacing,
    )
}

fn normal_stencil_z(
    row: usize,
    col: usize,
    spe: usize,
    heights: &[f32],
    h: f32,
    scale: f32,
    spacing: f32,
    seam_weld: &ChunkMeshSeamWeld,
) -> (f32, f32, f32) {
    let sample = |row: usize, col: usize| heights[row * spe + col];
    if row == 0 {
        if let Some(south) = seam_weld
            .south_interior
            .as_ref()
            .and_then(|strip| strip.get(col))
        {
            return (*south * scale, sample(row + 1, col) * scale, 2.0 * spacing);
        }
        return (h, sample(row + 1, col) * scale, spacing);
    }
    if row == spe - 1 {
        if let Some(north) = seam_weld
            .north_interior
            .as_ref()
            .and_then(|strip| strip.get(col))
        {
            return (
                sample(row - 1, col) * scale,
                *north * scale,
                2.0 * spacing,
            );
        }
        return (sample(row - 1, col) * scale, h, spacing);
    }
    (
        sample(row - 1, col) * scale,
        sample(row + 1, col) * scale,
        2.0 * spacing,
    )
}

/// Build a renderable mesh from a chunk's authoritative heightfield.
///
/// Generates positions, analytic normals, UVs, and triangle indices. Normals
/// are computed from the height gradient (central differences) so lighting does
/// not depend on triangle winding; winding is chosen so front faces point up
/// (+Y), matching `StandardMaterial`'s default back-face culling.
pub fn build_chunk_mesh(heightfield: &Heightfield, lod: ChunkLod) -> Mesh {
    build_chunk_mesh_scaled(heightfield, lod, 1.0, &ChunkMeshSeamWeld::default())
}

/// Like [`build_chunk_mesh`], but multiplies sample heights for visualization.
///
/// Authoritative [`Heightfield`] data is unchanged; only vertex positions and
/// normals are scaled. Values above `1.0` exaggerate relief (useful when source
/// export heights are correct but too subtle to see at RTS camera distances).
pub fn build_chunk_mesh_scaled(
    heightfield: &Heightfield,
    lod: ChunkLod,
    vertical_scale: f32,
    seam_weld: &ChunkMeshSeamWeld,
) -> Mesh {
    #[cfg(test)]
    BUILD_MESH_CALLS.fetch_add(1, Ordering::Relaxed);

    let ChunkLod::Full = lod;

    let spe = heightfield.samples_per_edge() as usize;
    let spacing = heightfield.spacing_meters();
    let heights = build_mesh_height_grid(heightfield.samples(), spe, seam_weld);
    let last = (spe - 1) as f32;

    let vertex_count = spe * spe;
    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut uvs = Vec::with_capacity(vertex_count);

    let height = |row: usize, col: usize| heights[row * spe + col];

    for row in 0..spe {
        for col in 0..spe {
            let h = height(row, col) * vertical_scale;
            positions.push([col as f32 * spacing, h, row as f32 * spacing]);
            uvs.push([col as f32 / last, row as f32 / last]);

            let (hx0, hx1, dx) = normal_stencil_x(
                row,
                col,
                spe,
                &heights,
                h,
                vertical_scale,
                spacing,
                seam_weld,
            );
            let (hz0, hz1, dz) = normal_stencil_z(
                row,
                col,
                spe,
                &heights,
                h,
                vertical_scale,
                spacing,
                seam_weld,
            );

            let dhdx = (hx1 - hx0) / dx;
            let dhdz = (hz1 - hz0) / dz;
            let normal = Vec3::new(-dhdx, 1.0, -dhdz).normalize();
            normals.push([normal.x, normal.y, normal.z]);
        }
    }

    let cells = (spe - 1) * (spe - 1);
    let mut indices = Vec::with_capacity(cells * 6);
    let idx = |row: usize, col: usize| (row * spe + col) as u32;
    for row in 0..spe - 1 {
        for col in 0..spe - 1 {
            let a = idx(row, col);
            let b = idx(row, col + 1);
            let c = idx(row + 1, col);
            let d = idx(row + 1, col + 1);
            // Two triangles, both wound for an upward (+Y) front face.
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// CPU geometry counts for a generated chunk mesh.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChunkMeshGeometry {
    pub vertices: usize,
    pub indices: usize,
    pub triangles: usize,
}

/// Count vertices, indices, and triangles on a built chunk mesh.
pub fn chunk_mesh_geometry(mesh: &Mesh) -> ChunkMeshGeometry {
    let vertices = mesh.count_vertices();
    let indices = mesh
        .indices()
        .map(|indices| indices.iter().count())
        .unwrap_or(0);
    ChunkMeshGeometry {
        vertices,
        indices,
        triangles: indices / 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_tile(spe: u32) -> Heightfield {
        let n = (spe * spe) as usize;
        Heightfield::from_samples(spe, 1.0, vec![0.0; n]).unwrap()
    }

    #[test]
    fn produces_expected_vertex_and_index_counts() {
        let hf = flat_tile(3);
        let mesh = build_chunk_mesh(&hf, ChunkLod::Full);

        // 3x3 grid -> 9 vertices, 4 cells * 2 triangles * 3 indices = 24.
        assert_eq!(mesh.count_vertices(), 9);
        match mesh.indices().unwrap() {
            Indices::U32(i) => assert_eq!(i.len(), 24),
            _ => panic!("expected u32 indices"),
        }
    }

    #[test]
    fn includes_position_normal_and_uv_attributes() {
        let hf = flat_tile(4);
        let mesh = build_chunk_mesh(&hf, ChunkLod::Full);

        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
    }

    #[test]
    fn flat_terrain_has_upward_normals() {
        let hf = flat_tile(3);
        let mesh = build_chunk_mesh(&hf, ChunkLod::Full);
        let bevy::mesh::VertexAttributeValues::Float32x3(normals) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap()
        else {
            panic!("expected Float32x3 normals");
        };
        for n in normals {
            assert!((n[1] - 1.0).abs() < 1e-5, "expected +Y normal, got {n:?}");
        }
    }

    #[test]
    fn west_seam_weld_snaps_col_zero_to_neighbor_edge() {
        let hf = Heightfield::from_samples(3, 1.0, vec![0.0, 0.0, 0.0, 9.0, 1.0, 2.0, 0.0, 0.0, 0.0])
            .unwrap();
        let mesh = build_chunk_mesh_scaled(
            &hf,
            ChunkLod::Full,
            1.0,
            &ChunkMeshSeamWeld {
                west_edge: Some(vec![5.0, 6.0, 7.0]),
                south_edge: None,
                east_interior: None,
                north_interior: None,
                west_interior: None,
                south_interior: None,
            },
        );
        let bevy::mesh::VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!("expected Float32x3 positions");
        };
        // row 1, col 0 -> index 3
        assert_eq!(positions[3][1], 6.0);
    }

    #[test]
    fn repairs_non_overlap_east_edge_slope_for_mesh() {
        if NON_OVERLAP_EDGE_RAMP_SAMPLES < 2 {
            return;
        }
        let hf = Heightfield::from_samples(
            4,
            1.0,
            vec![
                0.0, 0.0, 0.0, 10.0, //
                0.0, 0.0, 0.0, 10.0, //
                0.0, 0.0, 0.0, 10.0, //
                0.0, 0.0, 0.0, 1.0,
            ],
        )
        .unwrap();
        let mesh = build_chunk_mesh_scaled(&hf, ChunkLod::Full, 1.0, &ChunkMeshSeamWeld::default());
        let bevy::mesh::VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!("expected Float32x3 positions");
        };
        // row 2 (still inside the east ramp): cols 1-2 ramp toward boundary col 3 (=10).
        assert!((positions[9][1] - (10.0 / 3.0)).abs() < 1e-5);
        assert!((positions[10][1] - (20.0 / 3.0)).abs() < 1e-5);
        assert!((positions[11][1] - 10.0).abs() < 1e-5);
    }

    #[test]
    fn repairs_sample_world_east_edge_cliff() {
        if NON_OVERLAP_EDGE_RAMP_SAMPLES < 2 {
            return;
        }
        use std::path::Path;

        use crate::terrain::decode::decode_chunk;

        let path = Path::new("assets/worlds/main/chunks/0_0.ron");
        if !path.exists() {
            return;
        }
        let text = std::fs::read_to_string(path).unwrap();
        let (_, data) = decode_chunk(&text).unwrap();
        let scale = 4_278_744.5;
        let mesh = build_chunk_mesh_scaled(
            &data.heightfield,
            ChunkLod::Full,
            scale,
            &ChunkMeshSeamWeld::default(),
        );
        let bevy::mesh::VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!("expected Float32x3 positions");
        };
        let spe = data.heightfield.samples_per_edge() as usize;
        let row = 128;
        let y_penultimate = positions[row * spe + (spe - 2)][1];
        let y_boundary = positions[row * spe + (spe - 1)][1];
        let step = (y_boundary - y_penultimate).abs();
        assert!(
            step < 1.0,
            "expected ramped east-edge step under 1 unit, got {step}"
        );
    }

    #[test]
    fn vertical_scale_exaggerates_positions() {
        let hf = Heightfield::from_samples(2, 1.0, vec![0.0, 1.0, 2.0, 3.0]).unwrap();
        let mesh = build_chunk_mesh_scaled(&hf, ChunkLod::Full, 100.0, &ChunkMeshSeamWeld::default());
        let bevy::mesh::VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!("expected Float32x3 positions");
        };
        assert_eq!(positions[1][1], 100.0);
        assert_eq!(positions[3][1], 300.0);
    }
}
