//! Full-screen Unit Editor UI (CG3).

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use bevy::ui::RelativeCursorPosition;

use crate::world::{AppearanceProfileCatalog, AppearanceParamId};

use super::controls::{appearance_control_specs, height_field_id};
use super::preview_studio::UnitEditorPreviewImage;
use super::session::{UnitEditorMode, UnitEditorSession};

#[derive(Component, Debug)]
pub struct UnitEditorUiRoot;

#[derive(Component, Debug)]
pub struct UnitEditorPreviewPane;

#[derive(Component, Debug)]
pub struct UnitEditorControlsPane;

/// Host for CG3 slider systems (full editor or origin-squad focus overlay).
#[derive(Component, Debug)]
pub struct UnitEditorControlsHost;

#[derive(Component, Debug, Clone, Copy)]
pub struct UnitEditorActionButton {
    pub action: UnitEditorAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitEditorAction {
    Done,
    Cancel,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct UnitEditorSliderTrack {
    pub field_id: u32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct UnitEditorSliderValue {
    pub field_id: u32,
}

#[derive(Component, Debug, Clone)]
pub struct UnitEditorSliderBinding {
    pub field_id: u32,
    pub is_height: bool,
    pub param_id: Option<AppearanceParamId>,
}

#[derive(Component, Debug)]
pub struct UnitEditorErrorText;

pub fn spawn_unit_editor_controls_panel(
    parent: &mut ChildSpawnerCommands<'_>,
    session: &UnitEditorSession,
    profiles: &AppearanceProfileCatalog,
) {
    let profile_id = session.draft.appearance.profile_id.clone();
    let profile = profiles.get(&profile_id);
    let title = session
        .draft
        .display_name
        .clone()
        .unwrap_or_else(|| "Unit Editor".to_string());

    parent
        .spawn((
            UnitEditorControlsPane,
            UnitEditorControlsHost,
            Node {
                width: Val::Px(320.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.07, 1.0)),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new(title),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgba(0.92, 0.95, 0.98, 1.0)),
            ));
            panel.spawn((
                UnitEditorErrorText,
                Text::new(""),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgba(0.95, 0.45, 0.45, 1.0)),
            ));
            if let Some(profile) = profile {
                spawn_slider_row(
                    panel,
                    "Height",
                    height_field_id(),
                    UnitEditorSliderBinding {
                        field_id: height_field_id(),
                        is_height: true,
                        param_id: None,
                    },
                );
                for spec in appearance_control_specs(profile) {
                    spawn_slider_row(
                        panel,
                        &spec.display_name,
                        spec.field_id,
                        UnitEditorSliderBinding {
                            field_id: spec.field_id,
                            is_height: false,
                            param_id: Some(spec.param_id.clone()),
                        },
                    );
                }
            }
            panel.spawn(Node {
                flex_grow: 1.0,
                ..default()
            });
            let actions: &[(&str, UnitEditorAction)] = match session.mode {
                UnitEditorMode::NewGameDraft { .. } => &[("Done", UnitEditorAction::Done)],
                _ => &[("Done", UnitEditorAction::Done), ("Cancel", UnitEditorAction::Cancel)],
            };
            for (label, action) in actions {
                panel.spawn((
                    UnitEditorActionButton { action: *action },
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                        margin: UiRect::bottom(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.16, 0.22, 0.30, 0.95)),
                    Text::new(*label),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.9, 0.94, 0.98, 1.0)),
                ));
            }
        });
}

pub fn spawn_unit_editor_ui(
    mut commands: Commands,
    session: Res<UnitEditorSession>,
    profiles: Res<AppearanceProfileCatalog>,
    preview_image: Res<UnitEditorPreviewImage>,
) {
    commands
        .spawn((
            UnitEditorUiRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            ZIndex(200),
        ))
        .with_children(|root| {
            spawn_unit_editor_controls_panel(root, &session, &profiles);

            root.spawn((
                UnitEditorPreviewPane,
                Node {
                    flex_grow: 1.0,
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(24.0)),
                    ..default()
                },
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

fn spawn_slider_row(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &str,
    field_id: u32,
    binding: UnitEditorSliderBinding,
) {
    parent
        .spawn((
            binding,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                min_height: Val::Px(24.0),
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgba(0.75, 0.82, 0.9, 1.0)),
                Node {
                    width: Val::Px(110.0),
                    ..default()
                },
            ));
            row.spawn((
                UnitEditorSliderTrack { field_id },
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
                    Node {
                        height: Val::Percent(100.0),
                        width: Val::Percent(0.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.25, 0.55, 0.78, 0.95)),
                ));
            });
            row.spawn((
                UnitEditorSliderValue { field_id },
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
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(0.9, 0.94, 0.98, 1.0)),
            ));
        });
}

pub fn despawn_unit_editor_ui(
    mut commands: Commands,
    roots: Query<Entity, With<UnitEditorUiRoot>>,
) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

pub fn sync_unit_editor_error_text(
    session: Option<Res<UnitEditorSession>>,
    mut texts: Query<&mut Text, With<UnitEditorErrorText>>,
) {
    let message = session
        .as_ref()
        .and_then(|value| value.error_message.clone())
        .unwrap_or_default();
    for mut text in &mut texts {
        **text = message.clone();
    }
}
