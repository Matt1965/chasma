//! Origin selection UI (CG7).

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use bevy::ui::RelativeCursorPosition;

use crate::menu::{MENU_BUTTON_FONT_SIZE, MENU_HEADING_FONT_SIZE, menu_text_font};
use crate::ui::unit_editor::UnitEditorPreviewImage;
use crate::world::OriginCatalog;

use super::session::OriginSelectSession;

#[derive(Component, Debug)]
pub struct OriginSelectUiRoot;

#[derive(Component, Debug, Clone, Copy)]
pub struct OriginSelectPreviewPane;

#[derive(Component, Debug, Clone, Copy)]
pub enum OriginSelectAction {
    Back,
    Continue,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct OriginSelectListButton {
    pub index: usize,
}

pub fn spawn_origin_select_ui(
    mut commands: Commands,
    origins: Res<OriginCatalog>,
    preview_image: Res<UnitEditorPreviewImage>,
) {
    commands
        .spawn((
            OriginSelectUiRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Row,
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.07, 0.98)),
            ZIndex(200),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(360.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(20.0)),
                    row_gap: Val::Px(12.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.07, 0.09, 1.0)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Choose Origin"),
                    menu_text_font(MENU_HEADING_FONT_SIZE),
                    TextColor(Color::srgb(0.92, 0.94, 0.96)),
                ));
                panel.spawn((
                    Text::new("Preview your starting squad. Customization comes later."),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.74, 0.78)),
                ));
                for (index, origin) in origins.definitions().iter().enumerate() {
                    spawn_origin_button(panel, index, &origin.display_name);
                }
                panel.spawn((Node {
                    flex_grow: 1.0,
                    ..default()
                },));
                spawn_action_button(panel, "Back", OriginSelectAction::Back);
                spawn_action_button(panel, "Continue", OriginSelectAction::Continue);
            });

            root.spawn((
                OriginSelectPreviewPane,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                RelativeCursorPosition::default(),
            ))
            .with_children(|pane| {
                pane.spawn((
                    ImageNode::new(preview_image.handle.clone()),
                    Node {
                        width: Val::Px(UnitEditorPreviewImage::WIDTH as f32),
                        height: Val::Px(UnitEditorPreviewImage::HEIGHT as f32),
                        ..default()
                    },
                ));
            });
        });
}

fn spawn_origin_button(parent: &mut ChildSpawnerCommands, index: usize, label: &str) {
    parent
        .spawn((
            Button,
            OriginSelectListButton { index },
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.16, 0.2, 0.26)),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label.to_string()),
                menu_text_font(MENU_BUTTON_FONT_SIZE),
                TextColor(Color::srgb(0.92, 0.94, 0.96)),
            ));
        });
}

fn spawn_action_button(parent: &mut ChildSpawnerCommands, label: &str, action: OriginSelectAction) {
    parent
        .spawn((
            Button,
            action,
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.2, 0.28, 0.36)),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label.to_string()),
                menu_text_font(MENU_BUTTON_FONT_SIZE),
                TextColor(Color::srgb(0.92, 0.94, 0.96)),
            ));
        });
}

pub fn despawn_origin_select_ui(mut commands: Commands, roots: Query<Entity, With<OriginSelectUiRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

pub fn sync_origin_select_list_highlight(
    session: Res<OriginSelectSession>,
    mut buttons: Query<(&OriginSelectListButton, &mut BackgroundColor)>,
) {
    for (button, mut color) in &mut buttons {
        *color = if button.index == session.selected_index {
            BackgroundColor(Color::srgb(0.28, 0.36, 0.46))
        } else {
            BackgroundColor(Color::srgb(0.16, 0.2, 0.26))
        };
    }
}
