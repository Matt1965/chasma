//! Ordered portrait pipeline diagnostics (dev). Set [`PORTRAIT_DIAGNOSTIC_STEP`] to 1-7.

use bevy::camera::{Camera, RenderTarget};
use bevy::prelude::*;

use crate::camera::render_layers::PORTRAIT_RENDER_LAYER;

use super::components::{UnitPortraitCamera, UnitPortraitStageRoot};
use super::pipeline::UnitPortraitCaptureState;
use super::studio::PORTRAIT_STAGE_OFFSET;
use super::studio_images::UnitPortraitStudioImages;

/// `0` = production. `1`-`6` = ordered checks. `7` = production cache verification.
#[cfg(feature = "dev")]
pub const PORTRAIT_DIAGNOSTIC_STEP: u8 = 0;

#[cfg(not(feature = "dev"))]
pub const PORTRAIT_DIAGNOSTIC_STEP: u8 = 0;

#[derive(Component, Debug)]
pub(crate) struct UnitPortraitDiagnosticCube;

#[derive(Resource, Debug, Default)]
pub struct PortraitDiagnosticTrace {
    pub step: u8,
    pub ui_image_id: Option<AssetId<Image>>,
    pub camera_target_id: Option<AssetId<Image>>,
    pub camera_active: bool,
    pub renderable_mesh_count: u32,
    pub logged_step: u8,
}

pub fn portrait_diagnostic_step() -> u8 {
    PORTRAIT_DIAGNOSTIC_STEP
}

pub fn portrait_diagnostics_block_cache(step: u8) -> bool {
    step >= 1 && step <= 6
}

pub fn portrait_diagnostics_continuous_camera(step: u8) -> bool {
    step >= 2 && step <= 6
}

pub fn portrait_diagnostics_show_without_selection(step: u8) -> bool {
    step >= 1 && step <= 6
}

pub fn portrait_diagnostic_clear_color(step: u8) -> Color {
    match step {
        2 => Color::srgb(0.92, 0.18, 0.12),
        3 | 4 => Color::srgb(0.14, 0.16, 0.22),
        _ => Color::srgb(0.12, 0.13, 0.15),
    }
}

pub fn sync_portrait_diagnostic_stage(
    mut commands: Commands,
    studio: Res<UnitPortraitStudioImages>,
    mut cameras: Query<(&mut Camera, &mut RenderTarget), With<UnitPortraitCamera>>,
    stage: Query<Entity, With<UnitPortraitStageRoot>>,
    cubes: Query<Entity, With<UnitPortraitDiagnosticCube>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    children: Query<&Children>,
    mesh3d: Query<&Mesh3d>,
    mut trace: ResMut<PortraitDiagnosticTrace>,
    capture: Res<UnitPortraitCaptureState>,
) {
    let step = portrait_diagnostic_step();
    trace.step = step;
    if step < 2 || step > 4 {
        for entity in &cubes {
            commands.entity(entity).despawn();
        }
    }

    for (mut camera, mut target) in &mut cameras {
        if step == 1 {
            camera.is_active = false;
        } else if portrait_diagnostics_continuous_camera(step) {
            camera.is_active = true;
            camera.clear_color = ClearColorConfig::Custom(portrait_diagnostic_clear_color(step));
            *target = RenderTarget::Image(studio.live_target.clone().into());
        }
        trace.camera_active = camera.is_active;
        if portrait_diagnostics_continuous_camera(step) {
            trace.camera_target_id = Some(studio.live_target.id());
        }
    }

    if step == 3 || step == 4 {
        if cubes.is_empty() {
            let mesh = meshes.add(Cuboid::new(0.55, 0.55, 0.55));
            let material = materials.add(if step == 3 {
                StandardMaterial {
                    base_color: Color::srgb(0.95, 0.85, 0.15),
                    unlit: true,
                    ..default()
                }
            } else {
                StandardMaterial {
                    base_color: Color::srgb(0.72, 0.42, 0.28),
                    perceptual_roughness: 0.55,
                    ..default()
                }
            });
            let Some(stage_root) = stage.iter().next() else {
                return;
            };
            commands.entity(stage_root).with_children(|parent| {
                parent.spawn((
                    UnitPortraitDiagnosticCube,
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.0, 1.05, 0.0),
                    Visibility::default(),
                    PORTRAIT_RENDER_LAYER,
                ));
            });
        }
        if let Some(stage_root) = stage.iter().next() {
            let mut count = 0u32;
            count_meshes(stage_root, &children, &mesh3d, &mut count);
            trace.renderable_mesh_count = count;
        }
    } else if let Some(actor) = capture.actor_entity {
        let mut count = 0u32;
        if children.get(actor).is_ok() {
            count_meshes(actor, &children, &mesh3d, &mut count);
        }
        trace.renderable_mesh_count = count;
    } else {
        trace.renderable_mesh_count = 0;
    }

    log_step_once(&mut trace);
}

fn count_meshes(
    root: Entity,
    children: &Query<&Children>,
    mesh3d: &Query<&Mesh3d>,
    count: &mut u32,
) {
    if mesh3d.get(root).is_ok() {
        *count += 1;
    }
    if let Ok(kids) = children.get(root) {
        for child in kids.iter() {
            count_meshes(child, children, mesh3d, count);
        }
    }
}

fn log_step_once(trace: &mut PortraitDiagnosticTrace) {
    if trace.logged_step == trace.step {
        return;
    }
    trace.logged_step = trace.step;
    if trace.step == 0 {
        return;
    }
    info!(
        "portrait diagnostic step {} | ui_image={:?} camera_target={:?} camera_active={} mesh_count={}",
        trace.step,
        trace.ui_image_id,
        trace.camera_target_id,
        trace.camera_active,
        trace.renderable_mesh_count,
    );
    match trace.step {
        1 => info!("portrait check 1: expect magenta/cyan checkerboard in HUD (no camera)."),
        2 => info!("portrait check 2: expect solid orange-red clear color from camera."),
        3 => info!("portrait check 3: expect yellow unlit cube on dark background."),
        4 => info!("portrait check 4: expect lit brown cube with stage lights."),
        5 => info!("portrait check 5: select a unit — expect full-body model in frame."),
        6 => info!("portrait check 6: head/shoulders + equipment on selected unit."),
        7 => info!("portrait check 7: cached portrait; camera off when idle."),
        _ => {}
    }
}

/// Fixed diagnostic camera for cube steps (stage-local offsets).
pub fn sync_portrait_diagnostic_camera(
    mut cameras: Query<&mut Transform, With<UnitPortraitCamera>>,
) {
    let step = portrait_diagnostic_step();
    if step != 3 && step != 4 {
        return;
    }
    let eye = PORTRAIT_STAGE_OFFSET + Vec3::new(0.0, 1.05, 1.65);
    let focus = PORTRAIT_STAGE_OFFSET + Vec3::new(0.0, 1.05, 0.0);
    for mut transform in &mut cameras {
        *transform = Transform::from_translation(eye).looking_at(focus, Vec3::Y);
    }
}
