//! Shared portrait render stage (camera, lights, backdrop).

use bevy::camera::{Camera, RenderTarget};
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::camera::render_layers::{PORTRAIT_RENDER_LAYER, portrait_camera_view_layers};

use super::components::{UnitPortraitCamera, UnitPortraitStageRoot};
use super::diagnostics::{
    portrait_diagnostic_step, portrait_diagnostics_continuous_camera,
    portrait_diagnostic_clear_color,
};
use super::pipeline::UnitPortraitCaptureState;
use super::studio_images::{UnitPortraitStudioImages, new_portrait_render_target};

pub const PORTRAIT_TEXTURE_SIZE: u32 = 256;
pub(crate) const PORTRAIT_STAGE_OFFSET: Vec3 = Vec3::new(0.0, -8_000.0, 0.0);

/// Baby-step: aim at the known studio origin until dynamic head framing is reliable.
pub const PORTRAIT_USE_FIXED_STUDIO_CAMERA: bool = true;

/// Validated full upper-body framing (`4985d386`); restore via [`portrait_studio_camera_transform_foundation`].
pub mod foundation_camera {
    pub const FOCUS_Y: f32 = 1.05;
    pub const EYE_Y: f32 = 1.05;
    pub const EYE_Z: f32 = 1.75;
}

/// Head-and-shoulders on the same studio rig (small delta from foundation).
pub mod head_camera {
    pub const FOCUS_Y: f32 = 1.48;
    pub const EYE_Y: f32 = 1.50;
    pub const EYE_Z: f32 = 1.08;
}

pub fn portrait_studio_camera_transform() -> Transform {
    portrait_studio_camera_transform_head_at(Vec2::ZERO)
}

pub fn portrait_studio_camera_transform_foundation() -> Transform {
    portrait_studio_camera_transform_foundation_at(Vec2::ZERO)
}

pub fn portrait_studio_camera_transform_head() -> Transform {
    portrait_studio_camera_transform_head_at(Vec2::ZERO)
}

/// Pan the fixed studio rig so measured mesh bounds center under the head framing.
pub fn portrait_studio_camera_transform_head_at(center_xz: Vec2) -> Transform {
    studio_camera_from_offsets(
        head_camera::FOCUS_Y,
        head_camera::EYE_Y,
        head_camera::EYE_Z,
        center_xz,
    )
}

pub fn portrait_studio_camera_transform_foundation_at(center_xz: Vec2) -> Transform {
    studio_camera_from_offsets(
        foundation_camera::FOCUS_Y,
        foundation_camera::EYE_Y,
        foundation_camera::EYE_Z,
        center_xz,
    )
}

fn studio_camera_from_offsets(focus_y: f32, eye_y: f32, eye_z: f32, center_xz: Vec2) -> Transform {
    let horizontal = Vec3::new(center_xz.x, 0.0, center_xz.y);
    let focus = PORTRAIT_STAGE_OFFSET + horizontal + Vec3::new(0.0, focus_y, 0.0);
    let eye = PORTRAIT_STAGE_OFFSET + horizontal + Vec3::new(0.0, eye_y, eye_z);
    Transform::from_translation(eye).looking_at(focus, Vec3::Y)
}

pub fn setup_unit_portrait_studio(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let studio_images = UnitPortraitStudioImages::install(&mut images);
    commands.insert_resource(studio_images.clone());

    let backdrop_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.17, 0.19),
        unlit: true,
        ..default()
    });

    commands.spawn((
        UnitPortraitStageRoot,
        Transform::from_translation(PORTRAIT_STAGE_OFFSET),
        Visibility::default(),
        PORTRAIT_RENDER_LAYER,
    ))
    .with_children(|stage| {
        let backdrop_mesh = meshes.add(Cuboid::new(3.0, 3.0, 0.05));
        stage.spawn((
            Mesh3d(backdrop_mesh),
            MeshMaterial3d(backdrop_material),
            Transform::from_xyz(0.0, 1.35, -1.2),
            PORTRAIT_RENDER_LAYER,
        ));
        stage.spawn((
            DirectionalLight {
                illuminance: 22_000.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.85, 0.35, 0.0)),
            PORTRAIT_RENDER_LAYER,
        ));
        stage.spawn((
            DirectionalLight {
                illuminance: 6_500.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.25, -0.9, 0.0)),
            PORTRAIT_RENDER_LAYER,
        ));
        stage.spawn((
            PointLight {
                intensity: 120_000.0,
                range: 8.0,
                ..default()
            },
            Transform::from_xyz(0.8, 1.8, 1.4),
            PORTRAIT_RENDER_LAYER,
        ));
    });

    let step = portrait_diagnostic_step();
    commands.spawn((
        UnitPortraitCamera,
        Camera3d::default(),
        Camera {
            order: 4,
            is_active: portrait_diagnostics_continuous_camera(step),
            clear_color: ClearColorConfig::Custom(portrait_diagnostic_clear_color(step)),
            ..default()
        },
        // Bevy derives viewport + aspect from the square render target; no override needed.
        RenderTarget::Image(studio_images.live_target.clone().into()),
        portrait_studio_camera_transform(),
        portrait_camera_view_layers(),
    ));
}

/// Force portrait isolation on the stage and every spawned glTF descendant.
pub fn propagate_portrait_render_layers(
    mut commands: Commands,
    roots: Query<Entity, With<UnitPortraitStageRoot>>,
    actors: Query<Entity, With<super::components::UnitPortraitSceneRoot>>,
    children: Query<&Children>,
    render_layers: Query<&RenderLayers>,
) {
    for root in roots.iter().chain(actors.iter()) {
        for entity in descendants(root, &children) {
            stamp_portrait_layer(&mut commands, entity, &render_layers);
        }
        stamp_portrait_layer(&mut commands, root, &render_layers);
    }
}

fn stamp_portrait_layer(
    commands: &mut Commands,
    entity: Entity,
    render_layers: &Query<&RenderLayers>,
) {
    let layers = match render_layers.get(entity) {
        Ok(existing) => existing.clone(),
        Err(_) => RenderLayers::default(),
    };
    commands.entity(entity).insert(layers.with(2));
}

pub fn sync_portrait_camera_active(
    capture: Res<UnitPortraitCaptureState>,
    mut cameras: Query<&mut Camera, With<UnitPortraitCamera>>,
) {
    let step = portrait_diagnostic_step();
    let active = if portrait_diagnostics_continuous_camera(step) {
        true
    } else {
        capture.active_request.is_some()
    };
    for mut camera in &mut cameras {
        camera.is_active = active;
    }
}

pub fn new_portrait_render_target_image(images: &mut Assets<Image>) -> Handle<Image> {
    images.add(new_portrait_render_target())
}

fn descendants(root: Entity, children: &Query<&Children>) -> Vec<Entity> {
    let mut stack = vec![root];
    let mut out = Vec::new();
    while let Some(entity) = stack.pop() {
        out.push(entity);
        if let Ok(kids) = children.get(entity) {
            for child in kids.iter() {
                stack.push(child);
            }
        }
    }
    out
}
