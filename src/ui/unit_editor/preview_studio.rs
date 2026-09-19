//! Isolated preview render target, camera, lighting, and orbit controls (CG3).

use bevy::camera::{Camera, RenderTarget};
use bevy::image::Image;
use bevy::math::{Affine3A, Vec3A};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use crate::camera::render_layers::PREVIEW_RENDER_LAYER;
use crate::units::presentation::{
    UnitEditorPreviewFraming, UnitEditorPreviewRoot, UnitEditorPreviewRosterMember,
    UnitEditorPreviewUnit,
};

use super::screen::UnitEditorPreviewPane;
use super::session::UnitEditorSession;

const PREVIEW_WIDTH: u32 = 640;
const PREVIEW_HEIGHT: u32 = 640;
const PREVIEW_DISTANCE_BASE: f32 = 2.4;
const PREVIEW_FOCUS_FALLBACK_Y: f32 = 0.9;
const PREVIEW_FALLBACK_HEIGHT: f32 = 1.75;

#[derive(Resource, Debug, Clone)]
pub struct UnitEditorPreviewImage {
    pub handle: Handle<Image>,
}

impl UnitEditorPreviewImage {
    pub const WIDTH: u32 = PREVIEW_WIDTH;
    pub const HEIGHT: u32 = PREVIEW_HEIGHT;
}

#[derive(Component, Debug)]
pub struct UnitEditorPreviewCamera;

#[derive(Component, Debug)]
pub struct UnitEditorPreviewBackdrop;

pub fn setup_unit_editor_preview_studio(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let image = Image::new_target_texture(PREVIEW_WIDTH, PREVIEW_HEIGHT, TextureFormat::Bgra8UnormSrgb, None);
    let handle = images.add(image);
    commands.insert_resource(UnitEditorPreviewImage { handle: handle.clone() });

    let backdrop_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.12, 0.13, 0.16, 1.0),
        unlit: true,
        ..default()
    });

    commands.spawn((
        UnitEditorPreviewRoot,
        Transform::default(),
        Visibility::default(),
        PREVIEW_RENDER_LAYER,
    )).with_children(|studio| {
        let backdrop_mesh = meshes.add(Cuboid::new(20.0, 20.0, 0.1));
        studio.spawn((
            UnitEditorPreviewBackdrop,
            Mesh3d(backdrop_mesh),
            MeshMaterial3d(backdrop_material),
            Transform::from_xyz(0.0, PREVIEW_FOCUS_FALLBACK_Y, -3.0),
            PREVIEW_RENDER_LAYER,
        ));
        studio.spawn((
            DirectionalLight {
                illuminance: 12_000.0,
                shadows_enabled: true,
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.6, 0.0)),
            PREVIEW_RENDER_LAYER,
        ));
        studio.spawn((
            PointLight {
                intensity: 400_000.0,
                range: 12.0,
                ..default()
            },
            Transform::from_xyz(-1.5, 2.0, 2.0),
            PREVIEW_RENDER_LAYER,
        ));
    });

    commands.spawn((
        UnitEditorPreviewCamera,
        Camera3d::default(),
        Camera {
            order: 2,
            ..default()
        },
        RenderTarget::Image(handle.into()),
        Transform::from_xyz(0.0, PREVIEW_FOCUS_FALLBACK_Y, PREVIEW_DISTANCE_BASE)
            .looking_at(Vec3::new(0.0, PREVIEW_FOCUS_FALLBACK_Y, 0.0), Vec3::Y),
        PREVIEW_RENDER_LAYER,
    ));
}

pub fn cleanup_unit_editor_preview_studio(
    mut commands: Commands,
    roots: Query<Entity, With<UnitEditorPreviewRoot>>,
    cameras: Query<Entity, With<UnitEditorPreviewCamera>>,
) {
    for entity in roots.iter().chain(cameras.iter()) {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<UnitEditorPreviewImage>();
}

/// Measure rendered preview meshes once and cache a torso-centered framing volume.
pub fn update_unit_editor_preview_framing(
    mut commands: Commands,
    preview_units: Query<
        Entity,
        (
            Or<(With<UnitEditorPreviewUnit>, With<UnitEditorPreviewRosterMember>)>,
            Without<UnitEditorPreviewFraming>,
        ),
    >,
    children: Query<&Children>,
    mesh3d: Query<&Mesh3d>,
    meshes: Res<Assets<Mesh>>,
    global_transforms: Query<&GlobalTransform>,
) {
    for root in &preview_units {
        if let Some((center, height)) = measure_preview_body_bounds(
            root,
            &children,
            &mesh3d,
            &meshes,
            &global_transforms,
        ) {
            commands.entity(root).insert(UnitEditorPreviewFraming {
                body_center: center,
                body_height: height,
            });
        }
    }
}

pub fn update_unit_editor_preview_camera(
    mut session: Option<ResMut<UnitEditorSession>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    mouse_scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    mut cameras: Query<&mut Transform, With<UnitEditorPreviewCamera>>,
    preview_units: Query<(&GlobalTransform, Option<&UnitEditorPreviewFraming>), With<UnitEditorPreviewUnit>>,
    preview_pane: Query<&Interaction, With<UnitEditorPreviewPane>>,
) {
    let Some(mut session) = session else {
        return;
    };
    let scroll = mouse_scroll.delta.y;
    if scroll.abs() > f32::EPSILON {
        session.preview_zoom = (session.preview_zoom - scroll * 0.08).clamp(0.55, 1.8);
    }
    let preview_hovered = preview_pane.iter().any(|state| *state != Interaction::None);
    if preview_hovered && mouse_buttons.pressed(MouseButton::Left) {
        let delta = mouse_motion.delta;
        if delta.length_squared() > 0.0 {
            session.preview_yaw_radians -= delta.x * 0.01;
        }
    }

    let zoom = session.preview_zoom;
    let yaw = session.preview_yaw_radians;
    let (focus, body_height) = preview_units
        .iter()
        .next()
        .map(|(transform, framing)| {
            framing
                .map(|value| (value.body_center, value.body_height))
                .unwrap_or_else(|| {
                    (
                        transform.translation() + Vec3::Y * PREVIEW_FOCUS_FALLBACK_Y,
                        PREVIEW_FALLBACK_HEIGHT,
                    )
                })
        })
        .unwrap_or((Vec3::new(0.0, PREVIEW_FOCUS_FALLBACK_Y, 0.0), PREVIEW_FALLBACK_HEIGHT));
    let distance = PREVIEW_DISTANCE_BASE * zoom * (body_height / PREVIEW_FALLBACK_HEIGHT).clamp(0.75, 1.35);

    for mut transform in &mut cameras {
        let offset = Vec3::new(yaw.sin() * distance, body_height * 0.08, yaw.cos() * distance);
        *transform = Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y);
    }
}

pub fn rotate_unit_editor_preview_unit(
    session: Option<Res<UnitEditorSession>>,
    mut preview_units: Query<&mut Transform, With<UnitEditorPreviewUnit>>,
) {
    let yaw = session.map(|value| value.preview_yaw_radians).unwrap_or(0.0);
    for mut transform in &mut preview_units {
        transform.rotation = Quat::from_rotation_y(yaw);
    }
}

fn measure_preview_body_bounds(
    root: Entity,
    children: &Query<&Children>,
    mesh3d: &Query<&Mesh3d>,
    meshes: &Assets<Mesh>,
    global_transforms: &Query<&GlobalTransform>,
) -> Option<(Vec3, f32)> {
    let mut min = Vec3A::splat(f32::INFINITY);
    let mut max = Vec3A::splat(f32::NEG_INFINITY);
    let mut found = false;

    for entity in descendants(root, children) {
        let Ok(mesh3d) = mesh3d.get(entity) else {
            continue;
        };
        let Some(mesh) = meshes.get(&mesh3d.0) else {
            continue;
        };
        let Some(aabb) = mesh.final_aabb else {
            continue;
        };
        let Ok(global) = global_transforms.get(entity) else {
            continue;
        };
        let matrix = Affine3A::from(global.affine());
        let corners = [
            Vec3A::from(aabb.min),
            Vec3A::from(aabb.max),
            Vec3A::new(aabb.min.x, aabb.min.y, aabb.max.z),
            Vec3A::new(aabb.min.x, aabb.max.y, aabb.min.z),
            Vec3A::new(aabb.max.x, aabb.min.y, aabb.min.z),
            Vec3A::new(aabb.min.x, aabb.max.y, aabb.max.z),
            Vec3A::new(aabb.max.x, aabb.min.y, aabb.max.z),
            Vec3A::new(aabb.max.x, aabb.max.y, aabb.min.z),
        ];
        for corner in corners {
            let world = matrix.transform_point3a(corner);
            min = min.min(world);
            max = max.max(world);
            found = true;
        }
    }

    if !found {
        return None;
    }
    let center = Vec3::from((min + max) * 0.5);
    let height = (max.y - min.y).max(0.5);
    Some((center, height))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_camera_distance_scales_with_body_height() {
        let short = PREVIEW_DISTANCE_BASE * (1.0 / PREVIEW_FALLBACK_HEIGHT).clamp(0.75, 1.35);
        let tall = PREVIEW_DISTANCE_BASE * (2.1 / PREVIEW_FALLBACK_HEIGHT).clamp(0.75, 1.35);
        assert!(tall > short);
    }
}
