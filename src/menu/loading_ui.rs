//! Loading screen UI root.

use bevy::prelude::*;

use super::font::{MENU_HEADING_FONT_SIZE, MENU_TITLE_FONT_SIZE, menu_text_font};
use super::loading::LoadingSession;

#[derive(Component, Debug)]
pub struct LoadingRoot;

#[derive(Component, Debug)]
pub struct LoadingStatusText;

pub fn spawn_loading_ui(mut commands: Commands) {
    commands
        .spawn((
            LoadingRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.94)),
            ZIndex(10_100),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Loading"),
                menu_text_font(MENU_TITLE_FONT_SIZE),
                TextColor(Color::srgb(0.92, 0.94, 0.96)),
            ));
            root.spawn((
                LoadingStatusText,
                Text::new("Preparing..."),
                menu_text_font(MENU_HEADING_FONT_SIZE),
                TextColor(Color::srgb(0.75, 0.8, 0.85)),
            ));
        });
}

pub fn despawn_loading_ui(mut commands: Commands, roots: Query<Entity, With<LoadingRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

pub fn sync_loading_status_text(
    loading: Res<LoadingSession>,
    mut texts: Query<&mut Text, With<LoadingStatusText>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    let label = loading.phase.label();
    if text.as_str() != label {
        **text = label.to_string();
    }
}
