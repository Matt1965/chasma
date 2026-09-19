//! Preview camera controls for the continuous origin/squad stage (CG8).

use bevy::prelude::*;

use crate::menu::{OriginSquadViewMode, StartingSquadSession};
use crate::units::presentation::{UnitEditorPreviewFraming, UnitEditorPreviewRosterMember};
use crate::ui::unit_editor::UnitEditorPreviewCamera;

use super::presentation::FOCUS_STAGE_POSITION;
use super::screen::OriginSelectPreviewPane;

const PREVIEW_DISTANCE_BASE: f32 = 2.8;
const PREVIEW_FOCUS_Y: f32 = 0.9;
const FOCUS_DISTANCE_SCALE: f32 = 0.72;

pub fn update_origin_select_preview_camera(
    mut session: ResMut<StartingSquadSession>,
    editor_session: Option<Res<crate::ui::unit_editor::UnitEditorSession>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    mouse_scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    mut cameras: Query<&mut Transform, With<UnitEditorPreviewCamera>>,
    preview_pane: Query<&Interaction, With<OriginSelectPreviewPane>>,
    roster: Query<
        (&GlobalTransform, &UnitEditorPreviewRosterMember, Option<&UnitEditorPreviewFraming>),
    >,
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

    let yaw = session.preview_yaw_radians;
    let zoom = session.preview_zoom;
    let (focus, distance_scale) = match session.view_mode {
        OriginSquadViewMode::FullSquad => (Vec3::new(0.0, PREVIEW_FOCUS_Y, 0.0), 1.0),
        OriginSquadViewMode::FocusedMember { slot_index } => roster
            .iter()
            .find(|(_, member, _)| member.slot_index == slot_index)
            .map(|(_, _, framing)| {
                let center = framing
                    .map(|value| value.body_center)
                    .unwrap_or(FOCUS_STAGE_POSITION + Vec3::Y * PREVIEW_FOCUS_Y);
                (center, FOCUS_DISTANCE_SCALE)
            })
            .unwrap_or((FOCUS_STAGE_POSITION + Vec3::Y * PREVIEW_FOCUS_Y, FOCUS_DISTANCE_SCALE)),
    };
    let distance = PREVIEW_DISTANCE_BASE * zoom * distance_scale;
    for mut transform in &mut cameras {
        let offset = Vec3::new(yaw.sin() * distance, distance * 0.15, yaw.cos() * distance);
        *transform = Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y);
    }
}

pub fn rotate_origin_select_preview_roster(
    session: Res<StartingSquadSession>,
    editor_session: Option<Res<crate::ui::unit_editor::UnitEditorSession>>,
    mut roster: Query<(&UnitEditorPreviewRosterMember, &mut Transform)>,
) {
    let focused = session.focused_slot_index();
    let yaw = match focused {
        Some(_) => editor_session
            .as_ref()
            .map(|value| value.preview_yaw_radians)
            .unwrap_or(session.preview_yaw_radians),
        None => session.preview_yaw_radians,
    };
    for (member, mut transform) in &mut roster {
        match focused {
            Some(slot_index) if member.slot_index == slot_index => {
                transform.rotation = Quat::from_rotation_y(yaw);
            }
            None => {
                transform.rotation = Quat::from_rotation_y(yaw);
            }
            _ => {}
        }
    }
}
