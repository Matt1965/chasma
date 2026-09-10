//! HUD stat bar widgets (HP, nutrition).

use bevy::prelude::*;

use super::super::styles::{HUD_HP_FILL, HUD_NUTRITION_FILL, TEXT_MUTED, hud_body_font};
use super::depth::hud_stat_track_style;

#[derive(Component, Debug, Clone, Copy)]
pub struct HudStatBarFill {
    pub bar_id: HudStatBarId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HudStatBarId {
    Hp,
    Nutrition,
}

#[derive(Component, Debug)]
pub struct HudStatBarValueText {
    pub bar_id: HudStatBarId,
}

/// Height of a stat bar row, including its share of vertical breathing room.
pub const HUD_STAT_ROW_HEIGHT_PX: f32 = 30.0;
/// Height of the bar track itself.
pub const HUD_STAT_BAR_HEIGHT_PX: f32 = 14.0;

pub fn spawn_stat_bar_row(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &str,
    bar_id: HudStatBarId,
    fill_color: Color,
) {
    parent
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            height: Val::Px(HUD_STAT_ROW_HEIGHT_PX),
            flex_shrink: 0.0,
            ..default()
        },))
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                hud_body_font(),
                TextColor(TEXT_MUTED),
                Node {
                    min_width: Val::Px(42.0),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
            row.spawn((
                Node {
                    flex_grow: 1.0,
                    height: Val::Px(HUD_STAT_BAR_HEIGHT_PX),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(HUD_STAT_BAR_HEIGHT_PX * 0.5)),
                    ..default()
                },
                hud_stat_track_style(),
            ))
            .with_children(|track| {
                track.spawn((
                    HudStatBarFill { bar_id },
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        border_radius: BorderRadius::all(Val::Px(HUD_STAT_BAR_HEIGHT_PX * 0.5)),
                        ..default()
                    },
                    BackgroundColor(fill_color),
                ));
            });
            row.spawn((
                HudStatBarValueText { bar_id },
                Text::new("--"),
                hud_body_font(),
                TextColor(TEXT_MUTED),
                Node {
                    min_width: Val::Px(70.0),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
        });
}

pub fn sync_stat_bar(
    bar_id: HudStatBarId,
    current: f32,
    max: f32,
    fills: &mut Query<(&HudStatBarFill, &mut Node)>,
    texts: &mut Query<(&HudStatBarValueText, &mut Text)>,
) {
    let percent = if max > 0.0 {
        ((current / max) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    for (fill, mut node) in fills.iter_mut() {
        if fill.bar_id == bar_id {
            node.width = Val::Percent(percent);
        }
    }
    for (text, mut label) in texts.iter_mut() {
        if text.bar_id == bar_id {
            **label = format!("{:.0} / {:.0}", current.max(0.0), max.max(0.0));
        }
    }
}

pub fn hp_bar_color() -> Color {
    HUD_HP_FILL
}

pub fn nutrition_bar_color() -> Color {
    HUD_NUTRITION_FILL
}
