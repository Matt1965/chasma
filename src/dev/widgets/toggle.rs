//! Shared boolean toggle rows (Slice 9).

use bevy::prelude::*;

use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::{DevTooltipContent, DevTooltipTarget};
use crate::dev::window::DevWindowUi;

use super::theme::{TEXT_MUTED, TEXT_PRIMARY, label_text_font, toggle_button_bg};

/// Domain panels attach this marker with their own id/flag type.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct DevWidgetToggle {
    pub disabled: bool,
}

/// Inner fill shown when a toggle is on (font-independent).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct DevWidgetToggleMark;

#[derive(Component, Debug)]
pub(crate) struct DevWidgetToggleRow;

/// Spawn a checkbox-style toggle: bordered box + inner mark + label.
pub fn spawn_toggle_row<M: Component>(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &str,
    tooltip: DevTooltipContent,
    marker: M,
) {
    parent
        .spawn((
            DevWidgetToggleRow,
            DevTooltipTarget::from_content(tooltip),
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                DevWidgetToggle { disabled: false },
                marker,
                DevPanelUi,
                DevWindowUi,
                Button,
                Node {
                    width: Val::Px(18.0),
                    height: Val::Px(18.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(super::theme::BTN_BG_IDLE),
                BorderColor::all(super::theme::FIELD_BORDER_IDLE),
            ))
            .with_children(|btn| {
                btn.spawn((
                    DevWidgetToggleMark,
                    DevPanelUi,
                    Node {
                        width: Val::Px(10.0),
                        height: Val::Px(10.0),
                        ..default()
                    },
                    BackgroundColor(TEXT_PRIMARY),
                    Visibility::Hidden,
                ));
            });
            row.spawn((
                DevPanelUi,
                Text::new(label),
                label_text_font(),
                TextColor(TEXT_MUTED),
            ));
        });
}

/// Sync toggle checkbox backgrounds with domain marker.
pub fn sync_toggle_styles_with_marker<M, F, T: bevy::ecs::query::QueryFilter>(
    on: F,
    mut query: Query<
        (
            &Interaction,
            &DevWidgetToggle,
            &M,
            &mut BackgroundColor,
            &Children,
        ),
        T,
    >,
    mut marks: Query<&mut Visibility, With<DevWidgetToggleMark>>,
) where
    M: Component,
    F: Fn(&M) -> bool,
{
    for (interaction, toggle, marker, mut bg, children) in &mut query {
        *bg = toggle_button_bg(interaction, on(marker), toggle.disabled);
        let show_mark = on(marker) && !toggle.disabled;
        for child in children.iter() {
            if let Ok(mut visibility) = marks.get_mut(child) {
                *visibility = if show_mark {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::widgets::glyph_safety::contains_forbidden_dev_ui_glyph;

    #[test]
    fn toggle_row_does_not_use_unicode_checkbox_glyphs() {
        assert!(!contains_forbidden_dev_ui_glyph("+"));
        assert!(!contains_forbidden_dev_ui_glyph("Cycle enabled"));
    }
}
