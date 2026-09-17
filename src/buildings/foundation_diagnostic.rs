//! One-shot runtime diagnostics for foundation skirt visibility bisection.

use bevy::prelude::*;

use crate::terrain::terrain_surface_render_y_at;
use crate::world::{BuildingId, WorldConfig, WorldData};

use super::components::BuildingFoundationSkirt;
use super::foundation::{foundation_mesh_attribute_report, foundation_mesh_diagnostics};
use super::foundation_assets::FoundationPresentationAssets;

/// Log [`log_foundation_runtime_once`] after this skirt entity spawns.
#[derive(Component, Debug, Clone, Copy)]
pub struct FoundationRuntimeTrace {
    pub building_id: BuildingId,
}

pub fn log_foundation_runtime_once(
    mut commands: Commands,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    foundation_assets: Res<FoundationPresentationAssets>,
    asset_server: Res<AssetServer>,
    meshes: Res<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    images: Res<Assets<Image>>,
    render_terrain: Option<Res<crate::terrain::TerrainRenderAssets>>,
    pending: Query<
        (
            Entity,
            &FoundationRuntimeTrace,
            &GlobalTransform,
            &Transform,
            &Visibility,
            Option<&InheritedVisibility>,
            Option<&ViewVisibility>,
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
        ),
        With<BuildingFoundationSkirt>,
    >,
) {
    let vertical_scale = render_terrain
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let layout = config.chunk_layout();

    for (
        entity,
        trace,
        global,
        local,
        visibility,
        inherited,
        view,
        mesh3d,
        mesh_material,
    ) in &pending
    {
        let mesh = meshes.get(&mesh3d.0);
        let mesh_diag = mesh
            .map(foundation_mesh_diagnostics)
            .unwrap_or_else(|| "mesh asset missing".to_string());
        let attr = mesh.map(foundation_mesh_attribute_report);

        let material = materials.get(&mesh_material.0);
        let material_diag = material
            .map(|material| {
                format!(
                    "base_color={:?} base_color.alpha={:.3} alpha_mode={:?} unlit={} double_sided={} cull_mode={:?} texture_handle={}",
                    material.base_color,
                    material.base_color.alpha(),
                    material.alpha_mode,
                    material.unlit,
                    material.double_sided,
                    material.cull_mode,
                    material
                        .base_color_texture
                        .as_ref()
                        .map(|h| format!("{:?}", h.id()))
                        .unwrap_or_else(|| "none".to_string())
                )
            })
            .unwrap_or_else(|| "material asset missing".to_string());

        let shared_material_match = material.is_some_and(|m| {
            mesh_material.0 == foundation_assets.material
                && m.base_color_texture == foundation_assets.texture
        });

        let (texture_load, image_diag) = if let Some(texture) = foundation_assets.texture.as_ref() {
            let load = asset_server
                .get_load_state(texture)
                .map(|state| format!("{state:?}"))
                .unwrap_or_else(|| "unknown".to_string());
            let image = images
                .get(texture)
                .map(|img| {
                    format!(
                        "{}x{} format={:?}",
                        img.width(),
                        img.height(),
                        img.texture_descriptor.format
                    )
                })
                .unwrap_or_else(|| "image asset not resident".to_string());
            (load, image)
        } else {
            ("n/a (solid stage)".to_string(), "n/a".to_string())
        };

        let world_pos = global.translation();
        let terrain_render_y = terrain_surface_render_y_at(
            world_pos.x,
            world_pos.z,
            &world,
            layout,
            vertical_scale,
        )
        .map(|y| format!("{y:.3}"))
        .unwrap_or_else(|| "unavailable".to_string());

        let (top_y, bottom_y) = mesh_world_y_extents(global, mesh);

        let stage_b_mesh = if let Some(report) = attr {
            format!(
                "FOUNDATION STAGE B MESH:\n\
  mesh handle: {:?}\n\
  vertex count: {}\n\
  index count: {}\n\
  ATTRIBUTE_POSITION present: {}\n\
  ATTRIBUTE_NORMAL present: {}\n\
  ATTRIBUTE_UV_0 present: {}\n\
  UV count: {}\n\
  vertex count == UV count: {}\n\
  all UV finite: {}\n\
  UV min: {:?}\n\
  UV max: {:?}",
                mesh3d.0.id(),
                report.vertex_count,
                report.index_count,
                report.has_position,
                report.has_normal,
                report.has_uv_0,
                report.uv_count,
                report.uv_matches_vertices,
                report.all_uv_finite,
                report.uv_min,
                report.uv_max,
            )
        } else {
            "FOUNDATION STAGE B MESH: unavailable (mesh asset missing)".to_string()
        };

        info!(
            "\nFOUNDATION RUNTIME:\n\
  building_id: {}\n\
  foundation_entity: {:?}\n\
  entity exists: yes\n\
  Visibility: {:?}\n\
  InheritedVisibility: {:?}\n\
  ViewVisibility: {:?}\n\
  Transform: {:?}\n\
  GlobalTransform: {:?}\n\
  Mesh3d handle: {:?}\n\
  material handle: {:?}\n\
  shared foundation material match: {}\n\
  {}\n\
  {}\n\
  FoundationSkirt marker: yes\n\
  material stage: {:?}\n\
  texture kind: {:?}\n\
  texture path: {}\n\
  texture load state: {}\n\
  image: {}\n\
  material: {}\n\
  foundation top world Y: {:.3}\n\
  foundation bottom world Y: {:.3}\n\
  terrain render Y beneath: {}",
            trace.building_id.raw(),
            entity,
            visibility,
            inherited.map(|v| format!("{:?}", v.get())),
            view.map(|v| v.get()),
            local,
            global,
            mesh3d.0.id(),
            mesh_material.0.id(),
            shared_material_match,
            stage_b_mesh,
            mesh_diag,
            foundation_assets.stage,
            foundation_assets.texture_kind,
            foundation_assets.texture_path,
            texture_load,
            image_diag,
            material_diag,
            top_y,
            bottom_y,
            terrain_render_y,
        );

        commands.entity(entity).remove::<FoundationRuntimeTrace>();
    }
}

fn mesh_world_y_extents(global: &GlobalTransform, mesh: Option<&Mesh>) -> (f32, f32) {
    let Some(mesh) = mesh else {
        return (global.translation().y, global.translation().y);
    };
    let Some(positions) = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .and_then(|attr| attr.as_float3())
    else {
        return (global.translation().y, global.translation().y);
    };
    let mut top = f32::NEG_INFINITY;
    let mut bottom = f32::INFINITY;
    for pos in positions {
        let world = global.transform_point(Vec3::from(*pos));
        top = top.max(world.y);
        bottom = bottom.min(world.y);
    }
    if top.is_finite() && bottom.is_finite() {
        (top, bottom)
    } else {
        (global.translation().y, global.translation().y)
    }
}
