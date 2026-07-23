//! GLB mesh extraction for navigation blueprint generation (NV1.2).

use std::path::Path;

use bevy::prelude::*;

use crate::world::occupancy::bake::OCCUPANCY_COLLISION_NODE;
use crate::world::OccupancyError;

/// Triangle in building-local 3D space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalTriangle3d {
    pub a: Vec3,
    pub b: Vec3,
    pub c: Vec3,
}

impl LocalTriangle3d {
    pub fn normal(&self) -> Vec3 {
        (self.b - self.a).cross(self.c - self.a).normalize_or_zero()
    }

    pub fn centroid(&self) -> Vec3 {
        (self.a + self.b + self.c) / 3.0
    }
}

/// Authored portal hint from a `portal__*` glTF node.
#[derive(Debug, Clone, PartialEq)]
pub struct PortalMarker3d {
    pub name: String,
    pub position: Vec3,
}

/// Mesh data extracted from a building GLB.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BuildingMeshAnalysisInput {
    pub triangles: Vec<LocalTriangle3d>,
    pub portal_markers: Vec<PortalMarker3d>,
    pub source_path: String,
    pub used_collision_node: bool,
}

/// Load mesh triangles for navigation analysis.
///
/// Prefers the `occupancy_collision` node, then falls back to visible meshes.
pub fn load_building_mesh_for_navigation(
    asset_path: &Path,
) -> Result<BuildingMeshAnalysisInput, OccupancyError> {
    if let Ok(mut input) = load_from_named_node(asset_path, OCCUPANCY_COLLISION_NODE) {
        input.used_collision_node = true;
        return Ok(input);
    }
    load_from_visible_meshes(asset_path)
}

fn load_from_named_node(
    path: &Path,
    node_name: &str,
) -> Result<BuildingMeshAnalysisInput, OccupancyError> {
    let (document, buffers, _) = gltf::import(path)
        .map_err(|error| OccupancyError::BakeFailed(format!("glb import failed: {error}")))?;

    let mut triangles = Vec::new();
    let mut portal_markers = Vec::new();
    let mut found_node = false;
    for scene in document.scenes() {
        for node in scene.nodes() {
            walk_scene_node(
                node,
                Mat4::IDENTITY,
                node_name,
                &buffers,
                &mut found_node,
                &mut triangles,
                &mut portal_markers,
            )?;
        }
    }
    if !found_node || triangles.is_empty() {
        return Err(OccupancyError::CollisionNodeMissing {
            asset: path.display().to_string(),
        });
    }
    Ok(BuildingMeshAnalysisInput {
        triangles,
        portal_markers,
        source_path: path.display().to_string(),
        used_collision_node: false,
    })
}

fn load_from_visible_meshes(path: &Path) -> Result<BuildingMeshAnalysisInput, OccupancyError> {
    let (document, buffers, _) = gltf::import(path)
        .map_err(|error| OccupancyError::BakeFailed(format!("glb import failed: {error}")))?;

    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or_else(|| OccupancyError::BakeFailed("glb has no scenes".into()))?;

    let mut triangles = Vec::new();
    let mut portal_markers = Vec::new();
    for node in scene.nodes() {
        walk_scene_node(
            node,
            Mat4::IDENTITY,
            "",
            &buffers,
            &mut false,
            &mut triangles,
            &mut portal_markers,
        )?;
    }
    if triangles.is_empty() {
        return Err(OccupancyError::BakeFailed(
            "no visible mesh geometry for navigation analysis".into(),
        ));
    }
    Ok(BuildingMeshAnalysisInput {
        triangles,
        portal_markers,
        source_path: path.display().to_string(),
        used_collision_node: false,
    })
}

fn walk_scene_node(
    node: gltf::Node,
    parent: Mat4,
    collision_node_name: &str,
    buffers: &[gltf::buffer::Data],
    found_collision: &mut bool,
    triangles: &mut Vec<LocalTriangle3d>,
    portal_markers: &mut Vec<PortalMarker3d>,
) -> Result<(), OccupancyError> {
    let world = parent * node_local_matrix(&node);

    if let Some(name) = node.name() {
        if name.to_ascii_lowercase().starts_with("portal__") {
            let mut position = world.transform_point3(Vec3::ZERO);
            if let Some(mesh) = node.mesh() {
                if let Some((min, max)) = mesh_bounds(mesh, world, buffers) {
                    position = (min + max) * 0.5;
                }
            }
            portal_markers.push(PortalMarker3d {
                name: name.to_string(),
                position,
            });
        }
    }

    let extract_collision = !collision_node_name.is_empty()
        && node.name().is_some_and(|name| name == collision_node_name);
    if extract_collision {
        *found_collision = true;
        let mesh = node
            .mesh()
            .ok_or_else(|| OccupancyError::BakeFailed("collision node has no mesh".into()))?;
        push_mesh_triangles(mesh, world, buffers, triangles)?;
    } else if collision_node_name.is_empty() && node_visible(node.name()) {
        if let Some(mesh) = node.mesh() {
            let _ = push_mesh_triangles(mesh, world, buffers, triangles);
        }
    }

    for child in node.children().collect::<Vec<_>>() {
        walk_scene_node(
            child,
            world,
            collision_node_name,
            buffers,
            found_collision,
            triangles,
            portal_markers,
        )?;
    }
    Ok(())
}

fn push_mesh_triangles(
    mesh: gltf::Mesh,
    world: Mat4,
    buffers: &[gltf::buffer::Data],
    out: &mut Vec<LocalTriangle3d>,
) -> Result<(), OccupancyError> {
    for primitive in mesh.primitives() {
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
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
        let verts: Vec<Vec3> = positions
            .map(|p| {
                let v = world.transform_point3(Vec3::from_array(p));
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
            out.push(LocalTriangle3d {
                a: verts[chunk[0] as usize],
                b: verts[chunk[1] as usize],
                c: verts[chunk[2] as usize],
            });
        }
    }
    Ok(())
}

fn node_visible(name: Option<&str>) -> bool {
    let name = name.unwrap_or("");
    let lower = name.to_ascii_lowercase();
    !(lower.contains("collision")
        || lower.contains("occupancy")
        || lower.starts_with("portal__")
        || lower.contains("gizmo")
        || lower.contains("particle")
        || lower.contains("helper")
        || lower == "camera"
        || lower == "light"
        || lower.ends_with("_helper")
        || lower.ends_with("_helpers"))
}

fn node_local_matrix(node: &gltf::Node) -> Mat4 {
    let (translation, rotation, scale) = node.transform().decomposed();
    Mat4::from_scale_rotation_translation(
        Vec3::from(scale),
        Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]),
        Vec3::from(translation),
    )
}

fn mesh_bounds(
    mesh: gltf::Mesh,
    world: Mat4,
    buffers: &[gltf::buffer::Data],
) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;
    for primitive in mesh.primitives() {
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
        let Some(iter) = reader.read_positions() else {
            continue;
        };
        for position in iter {
            let p = world.transform_point3(Vec3::from(position));
            if !p.is_finite() {
                continue;
            }
            min = min.min(p);
            max = max.max(p);
            found = true;
        }
    }
    if found {
        Some((min, max))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn hut_mesh_loads_when_asset_present() {
        let path = PathBuf::from("assets/buildings/hut.glb");
        if !path.is_file() {
            return;
        }
        let input = load_building_mesh_for_navigation(&path).expect("hut mesh");
        assert!(!input.triangles.is_empty());
    }
}
