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

/// Build a renderable mesh from a chunk's authoritative heightfield.
///
/// Generates positions, analytic normals, UVs, and triangle indices. Normals
/// are computed from the height gradient (central differences) so lighting does
/// not depend on triangle winding; winding is chosen so front faces point up
/// (+Y), matching `StandardMaterial`'s default back-face culling.
pub fn build_chunk_mesh(heightfield: &Heightfield, lod: ChunkLod) -> Mesh {
    build_chunk_mesh_scaled(heightfield, lod, 1.0)
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
) -> Mesh {
    let ChunkLod::Full = lod;

    let spe = heightfield.samples_per_edge() as usize;
    let spacing = heightfield.spacing_meters();
    let samples = heightfield.samples();
    let last = (spe - 1) as f32;

    let vertex_count = spe * spe;
    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut uvs = Vec::with_capacity(vertex_count);

    let height = |row: usize, col: usize| samples[row * spe + col];

    for row in 0..spe {
        for col in 0..spe {
            let h = height(row, col) * vertical_scale;
            positions.push([col as f32 * spacing, h, row as f32 * spacing]);
            uvs.push([col as f32 / last, row as f32 / last]);

            // Central differences with one-sided fallback at the borders.
            let (hx0, hx1, dx) = if col == 0 {
                (h, height(row, col + 1) * vertical_scale, spacing)
            } else if col == spe - 1 {
                (height(row, col - 1) * vertical_scale, h, spacing)
            } else {
                (
                    height(row, col - 1) * vertical_scale,
                    height(row, col + 1) * vertical_scale,
                    2.0 * spacing,
                )
            };
            let (hz0, hz1, dz) = if row == 0 {
                (h, height(row + 1, col) * vertical_scale, spacing)
            } else if row == spe - 1 {
                (height(row - 1, col) * vertical_scale, h, spacing)
            } else {
                (
                    height(row - 1, col) * vertical_scale,
                    height(row + 1, col) * vertical_scale,
                    2.0 * spacing,
                )
            };

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
    fn vertical_scale_exaggerates_positions() {
        let hf = Heightfield::from_samples(2, 1.0, vec![0.0, 1.0, 2.0, 3.0]).unwrap();
        let mesh = build_chunk_mesh_scaled(&hf, ChunkLod::Full, 100.0);
        let bevy::mesh::VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!("expected Float32x3 positions");
        };
        assert_eq!(positions[1][1], 100.0);
        assert_eq!(positions[3][1], 300.0);
    }
}
