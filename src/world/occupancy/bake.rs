//! Offline occupancy mesh baking (ADR-080 B3).

use std::collections::BTreeSet;
use std::path::Path;

use bevy::prelude::*;

use crate::world::{
    BakedCellMask, FootprintDefinition, FootprintId, FootprintShape, OCCUPANCY_CELL_SIZE_METERS,
    OccupancyError,
};

/// Stable collision node name for occupancy rasterization.
pub const OCCUPANCY_COLLISION_NODE: &str = "occupancy_collision";

/// Bake configuration recorded in exported footprint data.
#[derive(Debug, Clone, PartialEq)]
pub struct BakeConfig {
    pub cell_size_meters: f32,
    pub collision_node: String,
}

impl Default for BakeConfig {
    fn default() -> Self {
        Self {
            cell_size_meters: OCCUPANCY_CELL_SIZE_METERS,
            collision_node: OCCUPANCY_COLLISION_NODE.to_string(),
        }
    }
}

/// Metadata for stale-bake detection.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BakeSourceMetadata {
    pub asset_path: String,
    pub source_hash: Option<String>,
}

/// Triangle in building-local XZ space for rasterization tests and offline bake.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalTriangle2d {
    pub a: Vec2,
    pub b: Vec2,
    pub c: Vec2,
}

/// Rasterize local triangles into a baked cell mask.
pub fn rasterize_triangles(
    triangles: &[LocalTriangle2d],
    config: &BakeConfig,
) -> Result<BakedCellMask, OccupancyError> {
    if triangles.is_empty() {
        return Err(OccupancyError::BakeFailed(
            "collision geometry is empty".to_string(),
        ));
    }

    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for tri in triangles {
        for point in [tri.a, tri.b, tri.c] {
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(OccupancyError::NonFiniteGeometry);
            }
            min = min.min(point);
            max = max.max(point);
        }
    }

    let cell = config.cell_size_meters;
    let width_cells = ((max.x - min.x) / cell).ceil().max(1.0) as u32;
    let depth_cells = ((max.y - min.y) / cell).ceil().max(1.0) as u32;
    if width_cells > crate::world::occupancy::cell::MAX_MASK_CELLS_PER_AXIS
        || depth_cells > crate::world::occupancy::cell::MAX_MASK_CELLS_PER_AXIS
    {
        return Err(OccupancyError::InvalidMaskDimensions {
            width_cells,
            depth_cells,
        });
    }

    let mut blocked = BTreeSet::new();
    for z in 0..depth_cells {
        for x in 0..width_cells {
            let center = min + Vec2::new((x as f32 + 0.5) * cell, (z as f32 + 0.5) * cell);
            if triangles.iter().any(|tri| point_in_triangle(center, tri)) {
                blocked.insert(z * width_cells + x);
            }
        }
    }

    if blocked.is_empty() {
        return Err(OccupancyError::BakeFailed(
            "rasterization produced no blocked cells".to_string(),
        ));
    }

    Ok(BakedCellMask {
        cell_size_meters: cell,
        width_cells,
        depth_cells,
        local_origin: min,
        blocked_cells: blocked,
        forced_open_cells: BTreeSet::new(),
        forced_blocked_cells: BTreeSet::new(),
        space_id: crate::world::SURFACE_SPACE_ID,
    })
}

fn point_in_triangle(point: Vec2, tri: &LocalTriangle2d) -> bool {
    fn sign(p1: Vec2, p2: Vec2, p3: Vec2) -> f32 {
        (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
    }
    let d1 = sign(point, tri.a, tri.b);
    let d2 = sign(point, tri.b, tri.c);
    let d3 = sign(point, tri.c, tri.a);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

/// Build a footprint definition from rasterized triangles.
pub fn bake_footprint_from_triangles(
    footprint_id: FootprintId,
    triangles: &[LocalTriangle2d],
    metadata: BakeSourceMetadata,
    config: &BakeConfig,
) -> Result<FootprintDefinition, OccupancyError> {
    let mask = rasterize_triangles(triangles, config)?;
    Ok(FootprintDefinition {
        id: footprint_id,
        shape: FootprintShape::BakedCellMask(mask),
        rotation_step_degrees: 90,
        enabled: true,
        source_asset: Some(metadata.asset_path),
        source_hash: metadata.source_hash,
        bake_cell_size_meters: Some(config.cell_size_meters),
    })
}

/// Dev-only GLB bake entry point. Fails clearly when collision node is absent.
#[cfg(feature = "data-import")]
pub fn bake_footprint_from_glb(
    footprint_id: FootprintId,
    asset_path: &Path,
    config: &BakeConfig,
) -> Result<FootprintDefinition, OccupancyError> {
    let triangles = load_collision_triangles_from_glb(asset_path, &config.collision_node)?;
    let metadata = BakeSourceMetadata {
        asset_path: asset_path.display().to_string(),
        source_hash: file_hash_hex(asset_path).ok(),
    };
    bake_footprint_from_triangles(footprint_id, &triangles, metadata, config)
}

#[cfg(feature = "data-import")]
fn load_collision_triangles_from_glb(
    path: &Path,
    node_name: &str,
) -> Result<Vec<LocalTriangle2d>, OccupancyError> {
    let (document, _buffers, _images) = gltf::import(path)
        .map_err(|error| OccupancyError::BakeFailed(format!("glb import failed: {error}")))?;

    let mut triangles = Vec::new();
    let mut found_node = false;
    for scene in document.scenes() {
        for node in scene.nodes() {
            collect_node_triangles(node, node_name, &mut found_node, &mut triangles)?;
        }
    }
    if !found_node {
        return Err(OccupancyError::CollisionNodeMissing {
            asset: path.display().to_string(),
        });
    }
    Ok(triangles)
}

#[cfg(feature = "data-import")]
fn collect_node_triangles(
    node: gltf::Node,
    target_name: &str,
    found: &mut bool,
    out: &mut Vec<LocalTriangle2d>,
) -> Result<(), OccupancyError> {
    if node.name().is_some_and(|name| name == target_name) {
        *found = true;
        let mesh = node
            .mesh()
            .ok_or_else(|| OccupancyError::BakeFailed("collision node has no mesh".into()))?;
        let world = node.transform().matrix();
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|_| None);
            let positions = reader
                .read_positions()
                .ok_or_else(|| OccupancyError::BakeFailed("mesh has no positions".into()))?;
            let indices: Vec<u32> = reader
                .read_indices()
                .map(|indices| indices.into_u32().collect())
                .unwrap_or_else(|| {
                    (0..positions.len() as u32 / 3)
                        .flat_map(|i| [i * 3, i * 3 + 1, i * 3 + 2])
                        .collect()
                });
            let world_matrix = Mat4::from_cols_array_2d(&world);
            let verts: Vec<Vec3> = positions
                .map(|p| {
                    let v = world_matrix.transform_point3(Vec3::from_array(p));
                    if !v.is_finite() {
                        Err(OccupancyError::NonFiniteGeometry)
                    } else {
                        Ok(v)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            for chunk in indices.chunks(3) {
                if chunk.len() < 3 {
                    continue;
                }
                let a = verts[chunk[0] as usize];
                let b = verts[chunk[1] as usize];
                let c = verts[chunk[2] as usize];
                out.push(LocalTriangle2d {
                    a: Vec2::new(a.x, a.z),
                    b: Vec2::new(b.x, b.z),
                    c: Vec2::new(c.x, c.z),
                });
            }
        }
    }
    for child in node.children() {
        collect_node_triangles(child, target_name, found, out)?;
    }
    Ok(())
}

#[cfg(feature = "data-import")]
pub fn source_file_hash_hex(path: &Path) -> Result<String, OccupancyError> {
    file_hash_hex(path)
}

#[cfg(feature = "data-import")]
fn file_hash_hex(path: &Path) -> Result<String, OccupancyError> {
    use std::hash::{Hash, Hasher};
    let metadata = std::fs::metadata(path)
        .map_err(|error| OccupancyError::BakeFailed(format!("metadata read failed: {error}")))?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    metadata.len().hash(&mut hasher);
    metadata.modified().ok().map(|time| time.hash(&mut hasher));
    Ok(format!("{:016x}", hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_triangle_produces_expected_mask() {
        let triangles = vec![LocalTriangle2d {
            a: Vec2::new(0.0, 0.0),
            b: Vec2::new(4.0, 0.0),
            c: Vec2::new(0.0, 4.0),
        }];
        let mask = rasterize_triangles(&triangles, &BakeConfig::default()).unwrap();
        assert!(mask.width_cells >= 1);
        assert!(!mask.blocked_cells.is_empty());
    }

    #[test]
    fn deterministic_repeated_bake() {
        let triangles = vec![LocalTriangle2d {
            a: Vec2::new(1.0, 1.0),
            b: Vec2::new(5.0, 1.0),
            c: Vec2::new(1.0, 5.0),
        }];
        let a = rasterize_triangles(&triangles, &BakeConfig::default()).unwrap();
        let b = rasterize_triangles(&triangles, &BakeConfig::default()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn empty_geometry_rejected() {
        assert!(rasterize_triangles(&[], &BakeConfig::default()).is_err());
    }
}
