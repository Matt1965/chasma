//! Shared portrait render stage (camera, lights, backdrop).

use bevy::camera::{Camera, RenderTarget};
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use crate::camera::render_layers::PORTRAIT_RENDER_LAYER;

use super::components::{UnitPortraitCamera, UnitPortraitStageRoot};
use super::pipeline::UnitPortraitCaptureState;

pub const PORTRAIT_TEXTURE_SIZE: u32 = 256;
const PORTRAIT_STAGE_OFFSET: Vec3 = Vec3::new(0.0, -8_000.0, 0.0);
const PORTRAIT_STAGE_CLEAR: Color = Color::srgb(0.12, 0.13, 0.15);

pub fn setup_unit_portrait_studio(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let idle_target = new_portrait_render_target(&mut images);
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

    commands.spawn((
        UnitPortraitCamera,
        Camera3d::default(),
        Camera {
            order: 4,
            is_active: false,
            clear_color: ClearColorConfig::Custom(PORTRAIT_STAGE_CLEAR),
            ..default()
        },
        RenderTarget::Image(idle_target.into()),
        Transform::from_xyz(0.0, PORTRAIT_STAGE_OFFSET.y + 1.45, PORTRAIT_STAGE_OFFSET.z + 1.0)
            .looking_at(
                Vec3::new(0.0, PORTRAIT_STAGE_OFFSET.y + 1.45, PORTRAIT_STAGE_OFFSET.z),
                Vec3::Y,
            ),
        PORTRAIT_RENDER_LAYER,
    ));
}

pub fn propagate_portrait_render_layers(
    mut commands: Commands,
    roots: Query<Entity, With<UnitPortraitStageRoot>>,
    actors: Query<Entity, With<super::components::UnitPortraitSceneRoot>>,
    children: Query<&Children>,
    layers: Query<&bevy::camera::visibility::RenderLayers>,
) {
    for root in roots.iter().chain(actors.iter()) {
        for entity in descendants(root, &children) {
            if layers.get(entity).is_ok() {
                continue;
            }
            commands.entity(entity).insert(PORTRAIT_RENDER_LAYER);
        }
    }
}

pub fn sync_portrait_camera_active(
    capture: Res<UnitPortraitCaptureState>,
    mut cameras: Query<&mut Camera, With<UnitPortraitCamera>>,
) {
    let active = capture.active_request.is_some() && capture.warmup_frames_remaining == 0;
    for mut camera in &mut cameras {
        camera.is_active = active;
    }
}

pub fn new_portrait_render_target(images: &mut Assets<Image>) -> Handle<Image> {
    let image = Image::new_target_texture(
        PORTRAIT_TEXTURE_SIZE,
        PORTRAIT_TEXTURE_SIZE,
        TextureFormat::Bgra8UnormSrgb,
        None,
    );
    images.add(image)
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
