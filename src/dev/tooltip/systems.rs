//! Tooltip hover detection and popup presentation.

use bevy::ecs::system::ParamSet;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::PrimaryWindow;

use super::components::{DevTooltipHoverZone, DevTooltipTarget};
use super::state::{DevTooltipState, TOOLTIP_HIDE_GRACE_SECS, TOOLTIP_HOVER_DELAY_SECS};

use crate::dev::widgets::theme::{
    TOOLTIP_BG, TOOLTIP_MAX_WIDTH_PX, TOOLTIP_PADDING_X, TOOLTIP_PADDING_Y, TOOLTIP_TEXT,
    label_text_font,
};
const TOOLTIP_Z: i32 = 2000;
const TOOLTIP_OFFSET: Vec2 = Vec2::new(12.0, 16.0);
const TOOLTIP_MARGIN_PX: f32 = 8.0;

#[derive(Component)]
pub(crate) struct DevTooltipPopup;

#[derive(Component)]
pub(crate) struct DevTooltipPopupText;

/// Spawn the floating tooltip root once at startup.
pub fn setup_dev_tooltip(mut commands: Commands) {
    commands
        .spawn((
            DevTooltipPopup,
            Node {
                position_type: PositionType::Absolute,
                max_width: Val::Px(TOOLTIP_MAX_WIDTH_PX),
                padding: UiRect::axes(Val::Px(TOOLTIP_PADDING_X), Val::Px(TOOLTIP_PADDING_Y)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(TOOLTIP_BG),
            FocusPolicy::Pass,
            ZIndex(TOOLTIP_Z),
        ))
        .with_children(|popup| {
            popup.spawn((
                DevTooltipPopupText,
                Text::new(""),
                label_text_font(),
                TextColor(TOOLTIP_TEXT),
            ));
        });
}

pub fn dismiss_dev_tooltip(mut tooltip: ResMut<DevTooltipState>) {
    tooltip.hide();
}

fn clamp_tooltip_position(position: Vec2, viewport: Vec2, estimated_height: f32) -> Vec2 {
    let width = TOOLTIP_MAX_WIDTH_PX;
    let height = estimated_height.max(40.0);
    let x = position.x.clamp(
        TOOLTIP_MARGIN_PX,
        (viewport.x - width - TOOLTIP_MARGIN_PX).max(TOOLTIP_MARGIN_PX),
    );
    let y = position.y.clamp(
        TOOLTIP_MARGIN_PX,
        (viewport.y - height - TOOLTIP_MARGIN_PX).max(TOOLTIP_MARGIN_PX),
    );
    Vec2::new(x, y)
}

pub fn sync_dev_tooltip_presentation(
    time: Res<Time>,
    dev_state: Res<crate::dev::dev_mode::DevModeState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut interactions: ParamSet<(
        Query<(&Interaction, &DevTooltipTarget), Without<DevTooltipHoverZone>>,
        Query<(&Interaction, &DevTooltipHoverZone), Without<DevTooltipTarget>>,
    )>,
    mut tooltip: ResMut<DevTooltipState>,
    mut popup: Query<(&mut Node, &mut Visibility), With<DevTooltipPopup>>,
    mut text: Query<&mut Text, With<DevTooltipPopupText>>,
) {
    if !dev_state.enabled {
        tooltip.hide();
    } else {
        let hovered_text = interactions
            .p0()
            .iter()
            .find(|(interaction, _)| **interaction == Interaction::Hovered)
            .map(|(_, target)| target.text())
            .or_else(|| {
                interactions
                    .p1()
                    .iter()
                    .find(|(interaction, _)| **interaction == Interaction::Hovered)
                    .map(|(_, zone)| zone.content.format())
            });

        if let Some(content) = hovered_text {
            if let Ok(window) = windows.single() {
                let pos = window.cursor_position().unwrap_or(Vec2::ZERO) + TOOLTIP_OFFSET;
                tooltip.queue_hover(content, pos, time.delta_secs());
            }
        } else {
            tooltip.pending_text = None;
            tooltip.hover_timer = 0.0;
            if tooltip.visible {
                if tooltip.tick_hide_grace(time.delta_secs()) {
                    tooltip.hide();
                }
            } else {
                tooltip.hide_grace_timer = 0.0;
            }
        }
    }

    let Ok((mut node, mut visibility)) = popup.single_mut() else {
        return;
    };
    if tooltip.visible {
        if let Ok(window) = windows.single() {
            let viewport = Vec2::new(window.width(), window.height());
            let line_count = tooltip.text.lines().count().max(1) as f32;
            let estimated_height = 12.0 + line_count * 14.0;
            let clamped = clamp_tooltip_position(tooltip.position, viewport, estimated_height);
            tooltip.position = clamped;
        }
        *visibility = Visibility::Visible;
        node.left = Val::Px(tooltip.position.x);
        node.top = Val::Px(tooltip.position.y);
        node.display = Display::Flex;
        if let Ok(mut label) = text.single_mut() {
            **label = tooltip.text.clone();
        }
    } else {
        *visibility = Visibility::Hidden;
        node.display = Display::None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hide_grace_prevents_immediate_dismiss() {
        let mut state = DevTooltipState::default();
        state.show("tip".into(), Vec2::ZERO);
        assert!(!state.tick_hide_grace(0.05));
        assert!(state.tick_hide_grace(TOOLTIP_HIDE_GRACE_SECS));
    }

    #[test]
    fn hover_delay_blocks_immediate_show() {
        let mut state = DevTooltipState::default();
        state.queue_hover("tip".into(), Vec2::ZERO, 0.1);
        assert!(!state.visible);
        state.queue_hover("tip".into(), Vec2::ZERO, TOOLTIP_HOVER_DELAY_SECS);
        assert!(state.visible);
    }

    #[test]
    fn clamp_keeps_tooltip_inside_viewport() {
        let pos = clamp_tooltip_position(Vec2::new(5000.0, 5000.0), Vec2::new(1280.0, 720.0), 80.0);
        assert!(pos.x < 1280.0);
        assert!(pos.y < 720.0);
    }
}
