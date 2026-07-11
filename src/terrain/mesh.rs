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

use super::albedo::{AlbedoFallback, ChunkAlbedoGrid, fallback_vertex_color};

#[cfg(test)]
use std::cell::RefCell;

#[cfg(test)]
thread_local! {
    static BUILD_MESH_CALLS: RefCell<usize> = const { RefCell::new(0) };
}

#[cfg(test)]
pub(crate) fn test_reset_build_mesh_calls() {
    BUILD_MESH_CALLS.with(|count| *count.borrow_mut() = 0);
}

#[cfg(test)]
pub(crate) fn test_build_mesh_call_count() -> usize {
    BUILD_MESH_CALLS.with(|count| *count.borrow())
}

/// Mesh level of detail for a chunk (ADR-013 Phase 2C).
///
/// Each level subsamples the authoritative full-resolution heightfield at a
/// power-of-two stride. Selection policy lives in [`super::lod`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
#[reflect(Debug, PartialEq)]
pub enum ChunkLod {
    /// Stride 1 — one mesh vertex per heightfield sample.
    Full,
    /// Stride 2.
    Half,
    /// Stride 4.
    Quarter,
    /// Stride 8.
    Eighth,
}

impl ChunkLod {
    /// Heightfield sample stride for this LOD level.
    pub const fn stride(self) -> usize {
        match self {
            Self::Full => 1,
            Self::Half => 2,
            Self::Quarter => 4,
            Self::Eighth => 8,
        }
    }

    /// Mesh grid width/height in samples after subsampling a full-resolution tile.
    pub fn lod_samples_per_edge(self, full_samples_per_edge: u32) -> u32 {
        lod_samples_per_edge(full_samples_per_edge as usize, self.stride()) as u32
    }

    /// Expected vertex and triangle counts for a built chunk mesh.
    pub fn expected_geometry(self, full_samples_per_edge: u32) -> ChunkMeshGeometry {
        let spe = self.lod_samples_per_edge(full_samples_per_edge) as usize;
        let cells = (spe - 1) * (spe - 1);
        ChunkMeshGeometry {
            vertices: spe * spe,
            indices: cells * 6,
            triangles: cells * 2,
        }
    }
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

/// LOD grid edge length after subsampling a full-resolution tile (integer math only).
#[inline]
pub(crate) fn lod_samples_per_edge(full_spe: usize, stride: usize) -> usize {
    debug_assert!(stride >= 1);
    debug_assert!(full_spe >= 2);
    (full_spe - 1) / stride + 1
}

/// Linear index of a vertex on the LOD grid.
#[inline]
pub(crate) fn lod_grid_index(lod_row: usize, lod_col: usize, lod_spe: usize) -> usize {
    lod_row * lod_spe + lod_col
}

/// One LOD vertex and its corresponding full-resolution grid coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LodVertexMapping {
    pub lod_row: usize,
    pub lod_col: usize,
    pub full_row: usize,
    pub full_col: usize,
    pub lod_idx: usize,
    pub full_idx: usize,
}

/// Iterate every LOD vertex in row-major order with synchronized full-grid indices.
pub(crate) fn iter_lod_vertex_mappings(
    full_spe: usize,
    stride: usize,
) -> impl Iterator<Item = LodVertexMapping> {
    let lod_spe = lod_samples_per_edge(full_spe, stride);
    (0..lod_spe).flat_map(move |lod_row| {
        (0..lod_spe).map(move |lod_col| {
            let full_row = lod_row * stride;
            let full_col = lod_col * stride;
            LodVertexMapping {
                lod_row,
                lod_col,
                full_row,
                full_col,
                lod_idx: lod_grid_index(lod_row, lod_col, lod_spe),
                full_idx: full_row * full_spe + full_col,
            }
        })
    })
}

#[cfg(feature = "dev")]
use std::sync::atomic::{AtomicBool, Ordering};

/// When enabled (dev builds only), mesh generation validates height/color LOD index alignment once.
#[cfg(feature = "dev")]
pub static DEBUG_VALIDATE_LOD_SAMPLE_ALIGNMENT: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "dev")]
static LOD_SAMPLE_ALIGNMENT_MISMATCH_LOGGED: AtomicBool = AtomicBool::new(false);

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

fn build_mesh_height_grid(samples: &[f32], spe: usize, seam_weld: &ChunkMeshSeamWeld) -> Vec<f32> {
    let mut heights = Vec::with_capacity(spe * spe);
    for row in 0..spe {
        for col in 0..spe {
            heights.push(sample_welded_height(row, col, spe, samples, seam_weld));
        }
    }

    repair_non_overlap_edge_slopes(&mut heights, spe);
    heights
}

/// Height at one full-resolution grid vertex after west/south seam weld (no edge ramp).
fn sample_welded_height(
    row: usize,
    col: usize,
    spe: usize,
    samples: &[f32],
    seam_weld: &ChunkMeshSeamWeld,
) -> f32 {
    let mut h = samples[row * spe + col];
    if col == 0 {
        if let Some(west) = seam_weld
            .west_edge
            .as_ref()
            .and_then(|strip| strip.get(row))
        {
            h = *west;
        }
    }
    if row == 0 {
        if let Some(south) = seam_weld
            .south_edge
            .as_ref()
            .and_then(|strip| strip.get(col))
        {
            h = *south;
        }
    }
    h
}

fn height_range_welded(samples: &[f32], spe: usize, seam_weld: &ChunkMeshSeamWeld) -> (f32, f32) {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for row in 0..spe {
        for col in 0..spe {
            let h = sample_welded_height(row, col, spe, samples, seam_weld);
            min = min.min(h);
            max = max.max(h);
        }
    }
    (min, max)
}

/// Build LOD height/color grids by direct stride sampling (no full-resolution buffers).
fn build_coarse_lod_grids(
    heightfield: &Heightfield,
    full_spe: usize,
    stride: usize,
    seam_weld: &ChunkMeshSeamWeld,
    albedo: Option<&ChunkAlbedoGrid>,
    fallback: AlbedoFallback,
) -> (Vec<f32>, Vec<[f32; 3]>, usize) {
    let samples = heightfield.samples();
    let (height_min, height_max) = if albedo.is_some() {
        (0.0, 0.0)
    } else {
        height_range_welded(samples, full_spe, seam_weld)
    };

    let lod_spe = lod_samples_per_edge(full_spe, stride);
    let mut lod_heights = Vec::with_capacity(lod_spe * lod_spe);
    let mut lod_colors = Vec::with_capacity(lod_spe * lod_spe);

    for mapping in iter_lod_vertex_mappings(full_spe, stride) {
        let h = sample_welded_height(
            mapping.full_row,
            mapping.full_col,
            full_spe,
            samples,
            seam_weld,
        );
        lod_heights.push(h);
        let color = if let Some(grid) = albedo {
            grid.data[mapping.full_idx]
        } else {
            fallback_vertex_color(h, height_min, height_max, fallback)
        };
        lod_colors.push(color);
    }

    debug_validate_coarse_lod_grids(
        &lod_heights,
        &lod_colors,
        samples,
        full_spe,
        stride,
        seam_weld,
        albedo,
        height_min,
        height_max,
        fallback,
    );

    (lod_heights, lod_colors, lod_spe)
}

#[cfg(feature = "dev")]
fn debug_validate_coarse_lod_grids(
    lod_heights: &[f32],
    lod_colors: &[[f32; 3]],
    samples: &[f32],
    full_spe: usize,
    stride: usize,
    seam_weld: &ChunkMeshSeamWeld,
    albedo: Option<&ChunkAlbedoGrid>,
    height_min: f32,
    height_max: f32,
    fallback: AlbedoFallback,
) {
    if !DEBUG_VALIDATE_LOD_SAMPLE_ALIGNMENT.load(Ordering::Relaxed) {
        return;
    }

    for mapping in iter_lod_vertex_mappings(full_spe, stride) {
        let expected_height = sample_welded_height(
            mapping.full_row,
            mapping.full_col,
            full_spe,
            samples,
            seam_weld,
        );
        let expected_color = if let Some(grid) = albedo {
            grid.data[mapping.full_idx]
        } else {
            fallback_vertex_color(expected_height, height_min, height_max, fallback)
        };

        let actual_height = lod_heights[mapping.lod_idx];
        let actual_color = lod_colors[mapping.lod_idx];

        let height_mismatch = actual_height.to_bits() != expected_height.to_bits();
        let color_mismatch = actual_color != expected_color;

        if height_mismatch || color_mismatch {
            debug_assert!(
                !height_mismatch && !color_mismatch,
                "coarse LOD sample mismatch at lod ({}, {}) full ({}, {})",
                mapping.lod_row,
                mapping.lod_col,
                mapping.full_row,
                mapping.full_col,
            );
            if !LOD_SAMPLE_ALIGNMENT_MISMATCH_LOGGED.swap(true, Ordering::Relaxed) {
                bevy::log::error!(
                    "coarse LOD height/color sample mismatch at lod ({}, {}) full ({}, {})",
                    mapping.lod_row,
                    mapping.lod_col,
                    mapping.full_row,
                    mapping.full_col,
                );
            }
            return;
        }
    }
}

#[cfg(not(feature = "dev"))]
fn debug_validate_coarse_lod_grids(
    _lod_heights: &[f32],
    _lod_colors: &[[f32; 3]],
    _samples: &[f32],
    _full_spe: usize,
    _stride: usize,
    _seam_weld: &ChunkMeshSeamWeld,
    _albedo: Option<&ChunkAlbedoGrid>,
    _height_min: f32,
    _height_max: f32,
    _fallback: AlbedoFallback,
) {
}

fn color_for_full_sample(
    full_idx: usize,
    full_heights: &[f32],
    height_min: f32,
    height_max: f32,
    albedo: Option<&ChunkAlbedoGrid>,
    fallback: AlbedoFallback,
) -> [f32; 3] {
    if let Some(grid) = albedo {
        grid.data[full_idx]
    } else {
        fallback_vertex_color(full_heights[full_idx], height_min, height_max, fallback)
    }
}

/// Build full-resolution vertex colors aligned to the welded height grid.
fn build_full_vertex_colors(
    full_heights: &[f32],
    full_spe: usize,
    height_min: f32,
    height_max: f32,
    albedo: Option<&ChunkAlbedoGrid>,
    fallback: AlbedoFallback,
) -> Vec<[f32; 3]> {
    debug_assert_eq!(full_heights.len(), full_spe * full_spe);
    (0..full_spe * full_spe)
        .map(|full_idx| {
            color_for_full_sample(
                full_idx,
                full_heights,
                height_min,
                height_max,
                albedo,
                fallback,
            )
        })
        .collect()
}

fn height_range(heights: &[f32]) -> (f32, f32) {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &h in heights {
        min = min.min(h);
        max = max.max(h);
    }
    (min, max)
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
            return (sample(row, col - 1) * scale, *east * scale, 2.0 * spacing);
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
            return (sample(row - 1, col) * scale, *north * scale, 2.0 * spacing);
        }
        return (sample(row - 1, col) * scale, h, spacing);
    }
    (
        sample(row - 1, col) * scale,
        sample(row + 1, col) * scale,
        2.0 * spacing,
    )
}

fn build_mesh_from_height_grid(
    heights: &[f32],
    spe: usize,
    spacing: f32,
    vertical_scale: f32,
    seam_weld: &ChunkMeshSeamWeld,
    colors: &[[f32; 3]],
) -> Mesh {
    let last = (spe - 1) as f32;

    let vertex_count = spe * spe;
    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut uvs = Vec::with_capacity(vertex_count);
    let mut vertex_colors = Vec::with_capacity(vertex_count);

    debug_assert_eq!(colors.len(), vertex_count);

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
                heights,
                h,
                vertical_scale,
                spacing,
                seam_weld,
            );
            let (hz0, hz1, dz) = normal_stencil_z(
                row,
                col,
                spe,
                heights,
                h,
                vertical_scale,
                spacing,
                seam_weld,
            );

            let dhdx = (hx1 - hx0) / dx;
            let dhdz = (hz1 - hz0) / dz;
            let normal = Vec3::new(-dhdx, 1.0, -dhdz).normalize();
            normals.push([normal.x, normal.y, normal.z]);
            vertex_colors.push([
                colors[row * spe + col][0],
                colors[row * spe + col][1],
                colors[row * spe + col][2],
                1.0,
            ]);
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vertex_colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Build a renderable mesh from a chunk's authoritative heightfield.
///
/// Generates positions, analytic normals, UVs, and triangle indices. Normals
/// are computed from the height gradient (central differences) so lighting does
/// not depend on triangle winding; winding is chosen so front faces point up
/// (+Y), matching `StandardMaterial`'s default back-face culling.
pub fn build_chunk_mesh(heightfield: &Heightfield, lod: ChunkLod) -> Mesh {
    build_chunk_mesh_scaled(
        heightfield,
        lod,
        1.0,
        &ChunkMeshSeamWeld::default(),
        None,
        AlbedoFallback::default(),
    )
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
    albedo: Option<&ChunkAlbedoGrid>,
    fallback: AlbedoFallback,
) -> Mesh {
    #[cfg(test)]
    BUILD_MESH_CALLS.with(|count| *count.borrow_mut() += 1);

    let full_spe = heightfield.samples_per_edge() as usize;
    let base_spacing = heightfield.spacing_meters();
    let stride = lod.stride();

    let (heights, colors, spe, spacing) = if stride == 1 {
        let full_heights = build_mesh_height_grid(heightfield.samples(), full_spe, seam_weld);
        let (height_min, height_max) = height_range(&full_heights);
        let full_colors = build_full_vertex_colors(
            &full_heights,
            full_spe,
            height_min,
            height_max,
            albedo,
            fallback,
        );
        (full_heights, full_colors, full_spe, base_spacing)
    } else {
        let (lod_heights, lod_colors, lod_spe) =
            build_coarse_lod_grids(heightfield, full_spe, stride, seam_weld, albedo, fallback);
        (
            lod_heights,
            lod_colors,
            lod_spe,
            base_spacing * stride as f32,
        )
    };

    debug_assert_eq!(heights.len(), spe * spe);
    debug_assert_eq!(colors.len(), spe * spe);
    debug_assert_eq!(spe, lod_samples_per_edge(full_spe, stride));

    // Seam-weld strips target full-resolution grids; subsampled meshes use welded
    // heights only (no per-LOD neighbor strips in Phase 2C-a).
    let lod_seam_weld = if stride == 1 {
        seam_weld
    } else {
        &ChunkMeshSeamWeld::default()
    };

    build_mesh_from_height_grid(
        &heights,
        spe,
        spacing,
        vertical_scale,
        lod_seam_weld,
        &colors,
    )
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
        let hf =
            Heightfield::from_samples(3, 1.0, vec![0.0, 0.0, 0.0, 9.0, 1.0, 2.0, 0.0, 0.0, 0.0])
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
            None,
            AlbedoFallback::default(),
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
        let mesh = build_chunk_mesh_scaled(
            &hf,
            ChunkLod::Full,
            1.0,
            &ChunkMeshSeamWeld::default(),
            None,
            AlbedoFallback::default(),
        );
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
            None,
            AlbedoFallback::default(),
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
        let mesh = build_chunk_mesh_scaled(
            &hf,
            ChunkLod::Full,
            100.0,
            &ChunkMeshSeamWeld::default(),
            None,
            AlbedoFallback::default(),
        );
        let bevy::mesh::VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!("expected Float32x3 positions");
        };
        assert_eq!(positions[1][1], 100.0);
        assert_eq!(positions[3][1], 300.0);
    }

    fn mesh_geometry(mesh: &Mesh) -> ChunkMeshGeometry {
        chunk_mesh_geometry(mesh)
    }

    #[test]
    fn full_lod_matches_phase_2a_geometry_for_257_tile() {
        let spe = 257u32;
        let hf = flat_tile(spe);
        let mesh = build_chunk_mesh(&hf, ChunkLod::Full);
        let geom = mesh_geometry(&mesh);
        assert_eq!(geom, ChunkLod::Full.expected_geometry(spe));
        assert_eq!(geom.vertices, 66_049);
        assert_eq!(geom.triangles, 131_072);
    }

    #[test]
    fn lod_geometry_counts_for_257_tile() {
        let spe = 257u32;
        let hf = flat_tile(spe);

        let half = mesh_geometry(&build_chunk_mesh(&hf, ChunkLod::Half));
        assert_eq!(half, ChunkLod::Half.expected_geometry(spe));
        assert_eq!(half.vertices, 16_641);
        assert_eq!(half.triangles, 32_768);

        let quarter = mesh_geometry(&build_chunk_mesh(&hf, ChunkLod::Quarter));
        assert_eq!(quarter, ChunkLod::Quarter.expected_geometry(spe));
        assert_eq!(quarter.vertices, 4_225);
        assert_eq!(quarter.triangles, 8_192);

        let eighth = mesh_geometry(&build_chunk_mesh(&hf, ChunkLod::Eighth));
        assert_eq!(eighth, ChunkLod::Eighth.expected_geometry(spe));
        assert_eq!(eighth.vertices, 1_089);
        assert_eq!(eighth.triangles, 2_048);
    }

    fn mesh_colors_rgb(mesh: &Mesh) -> Vec<[f32; 3]> {
        let bevy::mesh::VertexAttributeValues::Float32x4(colors) =
            mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap()
        else {
            panic!("expected vertex colors");
        };
        colors.iter().map(|c| [c[0], c[1], c[2]]).collect()
    }

    fn assert_lod_height_color_index_alignment(
        mesh: &Mesh,
        full_mesh: &Mesh,
        full_spe: usize,
        stride: usize,
    ) {
        let lod_spe = lod_samples_per_edge(full_spe, stride);
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        let full_positions = full_mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        let bevy::mesh::VertexAttributeValues::Float32x3(positions) = positions else {
            panic!("expected positions");
        };
        let bevy::mesh::VertexAttributeValues::Float32x3(full_positions) = full_positions else {
            panic!("expected positions");
        };
        let colors = mesh_colors_rgb(mesh);
        let full_colors = mesh_colors_rgb(full_mesh);

        assert_eq!(positions.len(), lod_spe * lod_spe);
        assert_eq!(colors.len(), positions.len());

        for mapping in iter_lod_vertex_mappings(full_spe, stride) {
            assert_eq!(
                positions[mapping.lod_idx], full_positions[mapping.full_idx],
                "position mismatch at lod ({}, {})",
                mapping.lod_row, mapping.lod_col,
            );
            assert_eq!(
                colors[mapping.lod_idx], full_colors[mapping.full_idx],
                "color mismatch at lod ({}, {})",
                mapping.lod_row, mapping.lod_col,
            );
            assert_eq!(
                mapping.lod_idx,
                lod_grid_index(mapping.lod_row, mapping.lod_col, lod_spe),
            );
        }
    }

    #[test]
    fn lod_vertex_mapping_matches_chunk_lod_geometry() {
        let full_spe = 257usize;
        for lod in [
            ChunkLod::Full,
            ChunkLod::Half,
            ChunkLod::Quarter,
            ChunkLod::Eighth,
        ] {
            let stride = lod.stride();
            let expected = lod.lod_samples_per_edge(full_spe as u32) as usize;
            assert_eq!(lod_samples_per_edge(full_spe, stride), expected);
            let count = iter_lod_vertex_mappings(full_spe, stride).count();
            assert_eq!(count, expected * expected, "{lod:?}");
        }
    }

    #[test]
    fn all_lods_keep_height_and_color_on_same_vertex_indices() {
        use super::super::albedo::ChunkAlbedoGrid;

        let spe = 257u32;
        let n = (spe * spe) as usize;
        let hf =
            Heightfield::from_samples(spe, 1.0, (0..n).map(|i| (i as f32 * 0.01).sin()).collect())
                .unwrap();
        let albedo = ChunkAlbedoGrid::from_samples(
            spe as usize,
            (0..n)
                .map(|i| {
                    let t = (i as f32 * 0.003).cos().abs();
                    [t, 0.25, 0.75 - t * 0.5]
                })
                .collect(),
        )
        .unwrap();
        let weld = ChunkMeshSeamWeld::default();

        let full = build_chunk_mesh_scaled(
            &hf,
            ChunkLod::Full,
            1.0,
            &weld,
            Some(&albedo),
            AlbedoFallback::default(),
        );

        for lod in [ChunkLod::Half, ChunkLod::Quarter, ChunkLod::Eighth] {
            let mesh = build_chunk_mesh_scaled(
                &hf,
                lod,
                1.0,
                &weld,
                Some(&albedo),
                AlbedoFallback::default(),
            );
            assert_lod_height_color_index_alignment(&mesh, &full, spe as usize, lod.stride());
        }
    }

    #[test]
    fn repeated_mesh_rebuild_is_deterministic_for_vertex_colors() {
        use super::super::albedo::ChunkAlbedoGrid;

        let hf =
            Heightfield::from_samples(9, 1.0, (0..81).map(|i| i as f32 * 0.1).collect()).unwrap();
        let albedo = ChunkAlbedoGrid::from_samples(
            9,
            (0..81).map(|i| [(i % 7) as f32 / 7.0, 0.2, 0.3]).collect(),
        )
        .unwrap();

        for lod in [
            ChunkLod::Full,
            ChunkLod::Half,
            ChunkLod::Quarter,
            ChunkLod::Eighth,
        ] {
            let first = build_chunk_mesh_scaled(
                &hf,
                lod,
                2.0,
                &ChunkMeshSeamWeld::default(),
                Some(&albedo),
                AlbedoFallback::default(),
            );
            let second = build_chunk_mesh_scaled(
                &hf,
                lod,
                2.0,
                &ChunkMeshSeamWeld::default(),
                Some(&albedo),
                AlbedoFallback::default(),
            );
            assert_eq!(mesh_colors_rgb(&first), mesh_colors_rgb(&second), "{lod:?}");
        }
    }

    #[test]
    fn fallback_colors_stable_across_lod_transitions() {
        let hf = Heightfield::from_samples(
            9,
            1.0,
            (0..81).map(|i| (i as f32 * 0.7).fract() * 10.0).collect(),
        )
        .unwrap();
        let weld = ChunkMeshSeamWeld::default();
        let full = build_chunk_mesh_scaled(
            &hf,
            ChunkLod::Full,
            1.0,
            &weld,
            None,
            AlbedoFallback::HeightGradient,
        );

        for lod in [ChunkLod::Half, ChunkLod::Quarter, ChunkLod::Eighth] {
            let coarse =
                build_chunk_mesh_scaled(&hf, lod, 1.0, &weld, None, AlbedoFallback::HeightGradient);
            assert_lod_height_color_index_alignment(&coarse, &full, 9, lod.stride());
        }
    }

    #[test]
    fn streaming_reload_simulation_preserves_vertex_colors() {
        use super::super::albedo::ChunkAlbedoGrid;

        let hf = Heightfield::from_samples(5, 2.0, (0..25).map(|i| i as f32).collect()).unwrap();
        let albedo = ChunkAlbedoGrid::from_samples(
            5,
            (0..25).map(|i| [i as f32 / 24.0, 0.4, 0.6]).collect(),
        )
        .unwrap();

        let initial = build_chunk_mesh_scaled(
            &hf,
            ChunkLod::Half,
            1.0,
            &ChunkMeshSeamWeld::default(),
            Some(&albedo),
            AlbedoFallback::default(),
        );
        let reload = build_chunk_mesh_scaled(
            &hf,
            ChunkLod::Half,
            1.0,
            &ChunkMeshSeamWeld::default(),
            Some(&albedo),
            AlbedoFallback::default(),
        );
        assert_eq!(mesh_colors_rgb(&initial), mesh_colors_rgb(&reload));
    }

    #[cfg(feature = "dev")]
    #[test]
    fn debug_validation_accepts_synchronized_lod_grids() {
        use std::sync::atomic::Ordering;

        DEBUG_VALIDATE_LOD_SAMPLE_ALIGNMENT.store(true, Ordering::Relaxed);
        LOD_SAMPLE_ALIGNMENT_MISMATCH_LOGGED.store(false, Ordering::Relaxed);

        let hf = flat_tile(9);
        let _ = build_chunk_mesh(&hf, ChunkLod::Quarter);

        DEBUG_VALIDATE_LOD_SAMPLE_ALIGNMENT.store(false, Ordering::Relaxed);
    }

    #[test]
    fn subsampled_positions_match_source_heightfield_at_stride() {
        let spe = 9u32;
        let n = (spe * spe) as usize;
        let samples: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let hf = Heightfield::from_samples(spe, 1.0, samples).unwrap();

        let full = build_chunk_mesh(&hf, ChunkLod::Full);
        let half = build_chunk_mesh(&hf, ChunkLod::Half);

        let full_positions = full.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        let half_positions = half.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        let bevy::mesh::VertexAttributeValues::Float32x3(full_positions) = full_positions else {
            panic!("expected positions");
        };
        let bevy::mesh::VertexAttributeValues::Float32x3(half_positions) = half_positions else {
            panic!("expected positions");
        };

        let full_spe = spe as usize;
        let stride = ChunkLod::Half.stride();
        let lod_spe = lod_samples_per_edge(full_spe, stride);
        for mapping in iter_lod_vertex_mappings(full_spe, stride) {
            assert_eq!(
                half_positions[mapping.lod_idx], full_positions[mapping.full_idx],
                "lod ({}, {}) vs full ({}, {})",
                mapping.lod_row, mapping.lod_col, mapping.full_row, mapping.full_col,
            );
        }
        assert_eq!(half_positions.len(), lod_spe * lod_spe);
    }

    #[test]
    fn flat_terrain_has_upward_normals_at_all_lods() {
        let hf = flat_tile(257);
        for lod in [
            ChunkLod::Full,
            ChunkLod::Half,
            ChunkLod::Quarter,
            ChunkLod::Eighth,
        ] {
            let mesh = build_chunk_mesh(&hf, lod);
            let bevy::mesh::VertexAttributeValues::Float32x3(normals) =
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap()
            else {
                panic!("expected normals");
            };
            for n in normals {
                assert!(
                    (n[1] - 1.0).abs() < 1e-5,
                    "{lod:?} expected +Y normal, got {n:?}"
                );
            }
        }
    }

    #[test]
    fn includes_vertex_color_attribute() {
        let hf = flat_tile(3);
        let mesh = build_chunk_mesh(&hf, ChunkLod::Full);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
    }

    #[test]
    fn vertex_colors_match_albedo_samples_at_full_lod() {
        use super::super::albedo::ChunkAlbedoGrid;

        let hf = Heightfield::from_samples(3, 1.0, vec![0.0; 9]).unwrap();
        let albedo = ChunkAlbedoGrid::from_samples(
            3,
            vec![
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [1.0, 1.0, 0.0],
                [1.0, 0.0, 1.0],
                [0.0, 1.0, 1.0],
                [0.5, 0.5, 0.5],
                [0.2, 0.2, 0.2],
                [0.8, 0.8, 0.8],
            ],
        )
        .unwrap();
        let mesh = build_chunk_mesh_scaled(
            &hf,
            ChunkLod::Full,
            1.0,
            &ChunkMeshSeamWeld::default(),
            Some(&albedo),
            AlbedoFallback::default(),
        );
        let bevy::mesh::VertexAttributeValues::Float32x4(colors) =
            mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap()
        else {
            panic!("expected vertex colors");
        };
        assert_eq!(colors.len(), 9);
        assert_eq!(colors[0][0..3], [1.0, 0.0, 0.0]);
        assert_eq!(colors[4][0..3], [1.0, 0.0, 1.0]);
    }

    #[test]
    fn missing_albedo_uses_height_gradient_fallback() {
        let hf = Heightfield::from_samples(2, 1.0, vec![0.0, 1.0, 0.0, 1.0]).unwrap();
        let mesh = build_chunk_mesh_scaled(
            &hf,
            ChunkLod::Full,
            1.0,
            &ChunkMeshSeamWeld::default(),
            None,
            AlbedoFallback::HeightGradient,
        );
        let bevy::mesh::VertexAttributeValues::Float32x4(colors) =
            mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap()
        else {
            panic!("expected vertex colors");
        };
        assert_ne!(colors[0][0..3], colors[3][0..3]);
    }

    #[test]
    fn lod_vertex_color_count_matches_vertex_count() {
        use super::super::albedo::ChunkAlbedoGrid;

        let spe = 9u32;
        let n = (spe * spe) as usize;
        let samples: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let hf = Heightfield::from_samples(spe, 1.0, samples).unwrap();
        let albedo = ChunkAlbedoGrid::from_samples(
            spe as usize,
            (0..n)
                .map(|i| {
                    let t = i as f32 / n as f32;
                    [t, 0.5, 1.0 - t]
                })
                .collect(),
        )
        .unwrap();

        for lod in [
            ChunkLod::Full,
            ChunkLod::Half,
            ChunkLod::Quarter,
            ChunkLod::Eighth,
        ] {
            let mesh = build_chunk_mesh_scaled(
                &hf,
                lod,
                1.0,
                &ChunkMeshSeamWeld::default(),
                Some(&albedo),
                AlbedoFallback::default(),
            );
            let geom = mesh_geometry(&mesh);
            let color_count = mesh
                .attribute(Mesh::ATTRIBUTE_COLOR)
                .map(|attr| match attr {
                    bevy::mesh::VertexAttributeValues::Float32x4(values) => values.len(),
                    _ => 0,
                })
                .unwrap_or(0);
            assert_eq!(color_count, geom.vertices, "{lod:?}");
        }
    }

    #[test]
    fn lod_colors_subsample_same_indices_as_height() {
        use super::super::albedo::ChunkAlbedoGrid;

        let spe = 9u32;
        let n = (spe * spe) as usize;
        let hf = Heightfield::from_samples(spe, 1.0, (0..n).map(|i| i as f32).collect()).unwrap();
        let albedo = ChunkAlbedoGrid::from_samples(
            spe as usize,
            (0..n)
                .map(|i| {
                    let t = (i as f32 * 0.1) % 1.0;
                    [t, 0.2, 0.3]
                })
                .collect(),
        )
        .unwrap();

        let full = build_chunk_mesh_scaled(
            &hf,
            ChunkLod::Full,
            1.0,
            &ChunkMeshSeamWeld::default(),
            Some(&albedo),
            AlbedoFallback::default(),
        );
        let half = build_chunk_mesh_scaled(
            &hf,
            ChunkLod::Half,
            1.0,
            &ChunkMeshSeamWeld::default(),
            Some(&albedo),
            AlbedoFallback::default(),
        );

        assert_lod_height_color_index_alignment(
            &half,
            &full,
            spe as usize,
            ChunkLod::Half.stride(),
        );
    }
}
