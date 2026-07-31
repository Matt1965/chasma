//! Bounded slider + numeric entry row (Slice 11).

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::DevTooltipTarget;
use crate::dev::window::DevWindowUi;

use super::theme::{TEXT_LABEL, TEXT_PRIMARY, small_text_font};

/// Marker on the draggable slider track.
#[derive(Component, Debug, Clone, Copy)]
pub struct DevWidgetSliderTrack {
    pub id: u32,
}

/// Marker on the numeric value button / field.
#[derive(Component, Debug, Clone, Copy)]
pub struct DevWidgetSliderValue {
    pub id: u32,
}

/// Active slider drag (blocks camera input).
#[derive(Resource, Debug, Default)]
pub struct DevSliderDragState {
    pub field_id: Option<u32>,
}

/// Spawn label + slider track + numeric value button.
pub fn spawn_bounded_slider_row(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &str,
    field_id: u32,
    label_width_px: f32,
    tooltip: &str,
) {
    parent
        .spawn((
            DevPanelUi,
            DevTooltipTarget::new(tooltip),
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                align_items: AlignItems::Center,
                min_height: Val::Px(20.0),
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                DevPanelUi,
                Text::new(label),
                small_text_font(),
                TextColor(TEXT_LABEL),
                Node {
                    width: Val::Px(label_width_px),
                    ..default()
                },
            ));
            row.spawn((
                DevWidgetSliderTrack { id: field_id },
                DevPanelUi,
                DevWindowUi,
                Button,
                RelativeCursorPosition::default(),
                Node {
                    flex_grow: 1.0,
                    height: Val::Px(14.0),
                    min_width: Val::Px(80.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.12, 0.16, 1.0)),
            ))
            .with_children(|track| {
                track.spawn((
                    DevPanelUi,
                    Node {
                        height: Val::Percent(100.0),
                        width: Val::Percent(0.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.25, 0.55, 0.78, 0.95)),
                ));
            });
            row.spawn((
                DevWidgetSliderValue { id: field_id },
                DevPanelUi,
                DevWindowUi,
                Button,
                Node {
                    min_width: Val::Px(52.0),
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.1, 0.16, 0.22, 0.95)),
                Text::new("0"),
                small_text_font(),
                TextColor(TEXT_PRIMARY),
            ));
        });
}

/// Update fill width and value label from normalized 0..1 position.
pub fn sync_slider_fill(
    field_id: u32,
    normalized: f32,
    display: &str,
    tracks: &mut Query<(&DevWidgetSliderTrack, &Children)>,
    fills: &mut Query<&mut Node, Without<DevWidgetSliderTrack>>,
    values: &mut Query<(&DevWidgetSliderValue, &mut Text)>,
) {
    let t = normalized.clamp(0.0, 1.0);
    for (track, children) in tracks.iter() {
        if track.id != field_id {
            continue;
        }
        if let Some(&child) = children.first() {
            if let Ok(mut node) = fills.get_mut(child) {
                node.width = Val::Percent(t * 100.0);
            }
        }
    }
    for (value, mut text) in values.iter_mut() {
        if value.id == field_id {
            **text = display.to_string();
        }
    }
}

/// Map normalized relative cursor (-0.5..0.5) to slider position 0..1.
pub fn slider_normalized_x(relative: &RelativeCursorPosition) -> Option<f32> {
    relative
        .normalized
        .map(|position| (position.x + 0.5).clamp(0.0, 1.0))
}

/// Map value to normalized slider position.
pub fn value_to_normalized(value: f32, min: f32, max: f32) -> f32 {
    if (max - min).abs() < f32::EPSILON {
        return 0.0;
    }
    ((value - min) / (max - min)).clamp(0.0, 1.0)
}

/// Map normalized slider position to value.
pub fn normalized_to_value(normalized: f32, min: f32, max: f32) -> f32 {
    min + normalized.clamp(0.0, 1.0) * (max - min)
}
