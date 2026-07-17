//! Offline GLB source-bounds measurement (`data-import` feature only).

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use crate::world::asset_sizing::{AssetSizingError, SourceBoundsOrigin, SourceDimensions};

pub const DEFAULT_SIZE_REFERENCE_NODE: &str = "size_reference";

pub fn asset_path_for_render_key(asset_root: &str, render_key: &str) -> PathBuf {
    PathBuf::from("assets")
        .join(asset_root)
        .join(format!("{render_key}.glb"))
}

pub fn measure_glb_source_bounds(
    asset_path: &Path,
    explicit_source: Option<SourceDimensions>,
    source_bounds_node: Option<&str>,
) -> Result<(SourceDimensions, SourceBoundsOrigin, Vec<String>), AssetSizingError> {
    if let Some(source) = explicit_source {
        if source.is_valid() {
            return Ok((source, SourceBoundsOrigin::ExplicitCatalog, Vec::new()));
        }
        return Err(AssetSizingError::SourceBoundsInvalid {
            message: "explicit catalog source dimensions invalid".into(),
        });
    }

    if !asset_path.is_file() {
        return Err(AssetSizingError::AssetNotFound {
            path: asset_path.display().to_string(),
        });
    }

    let (document, buffers, _) =
        gltf::import(asset_path).map_err(|err| AssetSizingError::SourceBoundsInvalid {
            message: err.to_string(),
        })?;

    if document.scenes().len() > 1 {
        return Err(AssetSizingError::SceneSelectionMissing);
    }

    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or(AssetSizingError::SourceBoundsUnavailable)?;

    if let Some(node_name) = source_bounds_node {
        let bounds = bounds_for_named_node(&document, &buffers, node_name)?;
        return Ok((bounds, SourceBoundsOrigin::NamedNode, Vec::new()));
    }

    if let Ok(bounds) = bounds_for_named_node(&document, &buffers, DEFAULT_SIZE_REFERENCE_NODE) {
        return Ok((bounds, SourceBoundsOrigin::NamedNode, Vec::new()));
    }

    let mut warnings = Vec::new();
    let bounds = combined_visible_mesh_bounds(scene, &document, &buffers, &mut warnings)?;
    Ok((bounds, SourceBoundsOrigin::CombinedVisibleMeshes, warnings))
}

fn bounds_for_named_node(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    node_name: &str,
) -> Result<SourceDimensions, AssetSizingError> {
    let node = document
        .nodes()
        .find(|node| node.name() == Some(node_name))
        .ok_or_else(|| AssetSizingError::SourceBoundsNodeMissing {
            node: node_name.to_string(),
        })?;
    let bounds = subtree_bounds(node, buffers, false)?;
    dimensions_from_aabb(bounds).ok_or(AssetSizingError::SourceBoundsUnavailable)
}

fn combined_visible_mesh_bounds(
    scene: gltf::Scene,
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    warnings: &mut Vec<String>,
) -> Result<SourceDimensions, AssetSizingError> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;

    for node in scene.nodes() {
        if node_visible(&node) {
            if let Some((node_min, node_max)) = subtree_bounds(node, buffers, true).ok().flatten() {
                min = min.min(node_min);
                max = max.max(node_max);
                found = true;
            }
        }
    }

    if !found {
        let _ = document;
        return Err(AssetSizingError::SourceBoundsUnavailable);
    }

    if let Some(w) = check_root_scale_warning(scene) {
        warnings.push(w);
    }

    dimensions_from_aabb(Some((min, max))).ok_or(AssetSizingError::SourceBoundsInvalid {
        message: "empty visible mesh bounds".into(),
    })
}

fn check_root_scale_warning(scene: gltf::Scene) -> Option<String> {
    for node in scene.nodes() {
        let (_, _, scale) = node.transform().decomposed();
        let s = Vec3::from(scale);
        if (s - Vec3::ONE).length() > 0.001 {
            return Some(format!(
                "root node scale ({:.3}, {:.3}, {:.3}) is not identity",
                s.x, s.y, s.z
            ));
        }
    }
    None
}

fn node_visible(node: &gltf::Node) -> bool {
    let name = node.name().unwrap_or("");
    !is_excluded_node_name(name)
}

fn is_excluded_node_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("collision")
        || lower.contains("occupancy")
        || lower.starts_with("portal__")
        || lower.contains("gizmo")
        || lower.contains("particle")
        || lower.contains("helper")
        || lower == "camera"
        || lower == "light"
        || lower.ends_with("_helper")
        || lower.ends_with("_helpers")
}

fn subtree_bounds(
    root: gltf::Node,
    buffers: &[gltf::buffer::Data],
    skip_excluded: bool,
) -> Result<Option<(Vec3, Vec3)>, AssetSizingError> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;
    visit_node(root, Mat4::IDENTITY, buffers, skip_excluded, &mut |world| {
        min = min.min(world);
        max = max.max(world);
        found = true;
    });
    if found {
        Ok(Some((min, max)))
    } else {
        Ok(None)
    }
}

fn visit_node(
    node: gltf::Node,
    parent: Mat4,
    buffers: &[gltf::buffer::Data],
    skip_excluded: bool,
    visit_vertex: &mut dyn FnMut(Vec3),
) {
    if skip_excluded && !node_visible(&node) {
        return;
    }
    let local = node_local_matrix(&node);
    let world_matrix = parent * local;
    if let Some(mesh) = node.mesh() {
        if let Some((mesh_min, mesh_max)) = mesh_bounds(mesh, world_matrix, buffers) {
            visit_vertex(mesh_min);
            visit_vertex(mesh_max);
        }
    }
    let children: Vec<_> = node.children().collect();
    for child in children {
        visit_node(child, world_matrix, buffers, skip_excluded, visit_vertex);
    }
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
    if found { Some((min, max)) } else { None }
}

fn dimensions_from_aabb(bounds: Option<(Vec3, Vec3)>) -> Option<SourceDimensions> {
    let (min, max) = bounds?;
    let size = max - min;
    if size.x <= 0.0 || size.y <= 0.0 || size.z <= 0.0 {
        return None;
    }
    if !size.is_finite() {
        return None;
    }
    Some(SourceDimensions {
        width_meters: size.x,
        height_meters: size.y,
        depth_meters: size.z,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robot_bounds_are_finite_when_asset_present() {
        let path = asset_path_for_render_key("units", "robot");
        if !path.is_file() {
            return;
        }
        let (dims, origin, _) = measure_glb_source_bounds(&path, None, None).expect("robot bounds");
        assert!(dims.is_valid());
        assert!(matches!(
            origin,
            SourceBoundsOrigin::NamedNode | SourceBoundsOrigin::CombinedVisibleMeshes
        ));
        assert!(dims.height_meters < 2.0);
    }

    #[test]
    fn chest_bounds_when_asset_present() {
        let path = asset_path_for_render_key("buildings", "chest");
        if !path.is_file() {
            return;
        }
        let (dims, _, _) = measure_glb_source_bounds(&path, None, None).expect("chest bounds");
        assert!(dims.is_valid());
    }
}
