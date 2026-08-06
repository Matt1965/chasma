//! Shared dev action and stepper buttons (Slice 9).

use bevy::prelude::*;

use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::DevTooltipTarget;
use crate::dev::window::DevWindowUi;

use super::interaction::DevButtonChrome;
use super::theme::{
    TEXT_PRIMARY, action_button_bg, label_text_font, standard_button_node, stepper_button_bg,
};

/// Marker for a standard dev action button.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct DevWidgetActionButton {
    pub disabled: bool,
}

/// Spawn a labeled action button with optional tooltip and domain marker bundle.
pub fn spawn_action_button<M: Component>(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &str,
    tooltip: Option<&str>,
    marker: M,
) {
    let mut entity = parent.spawn((
        DevWidgetActionButton { disabled: false },
        DevButtonChrome::default(),
        marker,
        DevPanelUi,
        DevWindowUi,
        Button,
        standard_button_node(8.0, 4.0),
        BorderColor::all(super::theme::BTN_BORDER_IDLE),
        BackgroundColor(super::theme::BTN_BG_IDLE),
        Text::new(label),
        label_text_font(),
        TextColor(TEXT_PRIMARY),
    ));
    if let Some(tip) = tooltip {
        entity.insert(DevTooltipTarget::new(tip));
    }
}

/// Spawn a compact stepper (+/-) button.
pub fn spawn_stepper_button<M: Component>(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &str,
    tooltip: Option<&str>,
    marker: M,
) {
    let mut entity = parent.spawn((
        marker,
        DevButtonChrome::default(),
        DevPanelUi,
        DevWindowUi,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
            min_width: Val::Px(22.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(super::theme::BTN_BG_IDLE),
        BorderColor::all(super::theme::BTN_BORDER_IDLE),
        Text::new(label),
        super::theme::small_text_font(),
        TextColor(TEXT_PRIMARY),
    ));
    if let Some(tip) = tooltip {
        entity.insert(DevTooltipTarget::new(tip));
    }
}

/// Spawn label + decrement + increment stepper row.
pub fn spawn_labeled_stepper_row<MDec: Component, MInc: Component>(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &str,
    label_width_px: f32,
    dec_marker: MDec,
    inc_marker: MInc,
    tooltip: Option<&str>,
) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|row| {
            let label_bundle = (
                DevPanelUi,
                Text::new(label),
                super::theme::small_text_font(),
                TextColor(super::theme::TEXT_LABEL),
                Node {
                    width: Val::Px(label_width_px),
                    ..default()
                },
            );
            if let Some(tip) = tooltip {
                row.spawn((DevTooltipTarget::new(tip), label_bundle));
            } else {
                row.spawn(label_bundle);
            }
            spawn_stepper_button(row, "-", None, dec_marker);
            spawn_stepper_button(row, "+", None, inc_marker);
        });
}

/// Sync action button backgrounds (respects disabled flag).
pub fn sync_action_button_styles(
    mut buttons: Query<(&DevWidgetActionButton, &mut DevButtonChrome), With<Button>>,
) {
    for (widget, mut chrome) in &mut buttons {
        chrome.disabled = widget.disabled;
    }
}

/// Sync stepper button backgrounds where marker type encodes active state.
pub fn sync_stepper_active_styles<M, F>(
    active: F,
    mut buttons: Query<(&Interaction, &M, &mut BackgroundColor)>,
) where
    M: Component,
    F: Fn(&M) -> bool,
{
    for (interaction, marker, mut bg) in &mut buttons {
        *bg = stepper_button_bg(interaction, active(marker));
    }
}
