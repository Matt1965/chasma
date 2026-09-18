//! Preview camera controls for origin selection (CG7).

use bevy::prelude::*;

use crate::units::presentation::UnitEditorPreviewRosterMember;
use crate::ui::unit_editor::UnitEditorPreviewCamera;

use super::screen::OriginSelectPreviewPane;
use super::session::OriginSelectSession;

const PREVIEW_DISTANCE_BASE: f32 = 2.8;
const PREVIEW_FOCUS_Y: f32 = 0.9;

pub fn update_origin_select_preview_camera(
    mut session: ResMut<OriginSelectSession>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    mouse_scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    mut cameras: Query<&mut Transform, With<UnitEditorPreviewCamera>>,
    preview_pane: Query<&Interaction, With<OriginSelectPreviewPane>>,
) {
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
    let focus = Vec3::new(0.0, PREVIEW_FOCUS_Y, 0.0);
    let distance = PREVIEW_DISTANCE_BASE * session.preview_zoom;
    let yaw = session.preview_yaw_radians;
    for mut transform in &mut cameras {
        let offset = Vec3::new(yaw.sin() * distance, distance * 0.15, yaw.cos() * distance);
        *transform = Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y);
    }
}

pub fn rotate_origin_select_preview_roster(
    session: Res<OriginSelectSession>,
    mut roster: Query<&mut Transform, With<UnitEditorPreviewRosterMember>>,
) {
    let rotation = Quat::from_rotation_y(session.preview_yaw_radians);
    for mut transform in &mut roster {
        transform.rotation = rotation;
    }
}
