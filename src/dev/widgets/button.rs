//! Shared dev action and stepper buttons (Slice 9).

use bevy::prelude::*;

use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::DevTooltipTarget;
use crate::dev::window::DevWindowUi;

use super::interaction::DevButtonChrome;
use super::theme::{TEXT_PRIMARY, label_text_font, standard_button_node};

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

/// Sync action button backgrounds (respects disabled flag).
pub fn sync_action_button_styles(
    mut buttons: Query<(&DevWidgetActionButton, &mut DevButtonChrome), With<Button>>,
) {
    for (widget, mut chrome) in &mut buttons {
        chrome.disabled = widget.disabled;
    }
}

