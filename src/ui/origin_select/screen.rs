//! Continuous origin/squad stage UI (CG8).

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use bevy::ui::RelativeCursorPosition;
use bevy::window::PrimaryWindow;

use crate::menu::{
    MENU_BUTTON_FONT_SIZE, MENU_HEADING_FONT_SIZE, StartingSquadSession, menu_text_font,
};
use crate::ui::unit_editor::UnitEditorPreviewImage;
use crate::world::OriginCatalog;

/// Persistent preview viewport for the origin/squad stage (survives focus transitions).
#[derive(Component, Debug)]
pub struct OriginSelectPreviewUiRoot;

/// Squad controls panel (hidden while a member is in focused edit mode).
#[derive(Component, Debug)]
pub struct OriginSelectSquadPanelRoot;

#[derive(Component, Debug, Clone, Copy)]
pub struct OriginSelectPreviewPane;

#[derive(Component, Debug, Clone, Copy)]
pub struct OriginSelectPreviewImage;

fn preview_viewport_intrinsic() -> Vec2 {
    UnitEditorPreviewImage::intrinsic_size()
}

#[derive(Component, Debug, Clone, Copy)]
pub enum OriginSquadAction {
    Back,
    BeginGame,
    PrevOrigin,
    NextOrigin,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct OriginSquadMemberButton {
    pub slot_index: usize,
}

#[derive(Component, Debug)]
pub struct OriginSquadOriginNameText;

#[derive(Component, Debug)]
pub struct OriginSquadOriginDescriptionText;

pub fn spawn_origin_select_preview_ui(
    mut commands: Commands,
    preview_image: Res<UnitEditorPreviewImage>,
) {
    commands
        .spawn((
            OriginSelectPreviewUiRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            ZIndex(190),
        ))
        .with_children(|root| {
            root.spawn((
                OriginSelectPreviewPane,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                RelativeCursorPosition::default(),
            ))
            .with_children(|pane| {
                pane.spawn((
                    OriginSelectPreviewImage,
                    ImageNode::new(preview_image.handle.clone()),
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(preview_viewport_intrinsic().x),
                        height: Val::Px(preview_viewport_intrinsic().y),
                        ..default()
                    },
                ));
            });
        });
}

/// Fit the preview render target inside the window with pillar/letterboxing (no stretch).
pub fn sync_origin_select_preview_viewport(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut preview_images: Query<&mut Node, With<OriginSelectPreviewImage>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let viewport = Vec2::new(window.width(), window.height());
    if viewport.x <= 1.0 || viewport.y <= 1.0 {
        return;
    }

    let intrinsic = preview_viewport_intrinsic();
    let scale = (viewport.x / intrinsic.x).min(viewport.y / intrinsic.y);
    let size = intrinsic * scale;
    let offset = (viewport - size) * 0.5;

    for mut node in &mut preview_images {
        node.width = Val::Px(size.x);
        node.height = Val::Px(size.y);
        node.left = Val::Px(offset.x);
        node.top = Val::Px(offset.y);
        node.position_type = PositionType::Absolute;
    }
}

pub fn spawn_origin_select_squad_panel(
    mut commands: Commands,
    origins: Res<OriginCatalog>,
    session: Res<StartingSquadSession>,
) {
    let origin = origins.get_index(session.selected_origin_index);
    let (name, description) = origin
        .map(|value| (value.display_name.clone(), value.description.clone()))
        .unwrap_or_else(|| ("Origin".to_string(), String::new()));

    commands
        .spawn((
            OriginSelectSquadPanelRoot,
            Node {
                width: Val::Px(360.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(20.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.07, 0.09, 1.0)),
            ZIndex(200),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Starting Squad"),
                menu_text_font(MENU_HEADING_FONT_SIZE),
                TextColor(Color::srgb(0.92, 0.94, 0.96)),
            ));
            spawn_origin_nav_row(panel);
            panel.spawn((
                OriginSquadOriginNameText,
                Text::new(name),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.92, 0.96)),
            ));
            panel.spawn((
                OriginSquadOriginDescriptionText,
                Text::new(description),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.68, 0.72, 0.76)),
            ));
            panel.spawn((
                Text::new("Select a squad member to customize appearance."),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.62, 0.66, 0.7)),
            ));
            if let Some(draft) = session.active_draft(&origins) {
                for (slot_index, member) in draft.members.iter().enumerate() {
                    let label = if member.edited {
                        format!("{} (edited)", member.role_label)
                    } else {
                        member.role_label.clone()
                    };
                    spawn_member_button(panel, slot_index, &label);
                }
            }
            panel.spawn((Node {
                flex_grow: 1.0,
                ..default()
            },));
            spawn_action_button(panel, "Back", OriginSquadAction::Back);
            spawn_action_button(panel, "Begin Game", OriginSquadAction::BeginGame);
        });
}

fn spawn_origin_nav_row(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|row| {
            spawn_action_button(row, "<", OriginSquadAction::PrevOrigin);
            row.spawn((
                Text::new("Origin"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.74, 0.78)),
                Node {
                    flex_grow: 1.0,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ));
            spawn_action_button(row, ">", OriginSquadAction::NextOrigin);
        });
}

fn spawn_member_button(parent: &mut ChildSpawnerCommands, slot_index: usize, label: &str) {
    parent
        .spawn((
            Button,
            OriginSquadMemberButton { slot_index },
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

fn spawn_action_button(parent: &mut ChildSpawnerCommands, label: &str, action: OriginSquadAction) {
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

pub fn despawn_origin_select_ui(
    mut commands: Commands,
    preview_roots: Query<Entity, With<OriginSelectPreviewUiRoot>>,
    squad_panels: Query<Entity, With<OriginSelectSquadPanelRoot>>,
) {
    for entity in preview_roots.iter().chain(squad_panels.iter()) {
        commands.entity(entity).despawn();
    }
}

pub fn sync_origin_squad_origin_text(
    session: Res<StartingSquadSession>,
    origins: Res<OriginCatalog>,
    mut names: Query<&mut Text, With<OriginSquadOriginNameText>>,
    mut descriptions: Query<&mut Text, (With<OriginSquadOriginDescriptionText>, Without<OriginSquadOriginNameText>)>,
) {
    let Some(origin) = origins.get_index(session.selected_origin_index) else {
        return;
    };
    for mut text in &mut names {
        **text = origin.display_name.clone();
    }
    for mut text in &mut descriptions {
        **text = origin.description.clone();
    }
}
