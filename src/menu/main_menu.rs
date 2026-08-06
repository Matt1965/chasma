//! Main Menu UI root (spawned on enter MainMenu, despawned on exit).

use bevy::asset::LoadState;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::PrimaryWindow;

use super::font::{
    MENU_BODY_FONT_SIZE, MENU_BUTTON_FONT_SIZE, MENU_HEADING_FONT_SIZE, MENU_TITLE_FONT_SIZE,
    menu_text_font,
};
use super::navigation::{MenuNavigation, MenuPage};
use super::settings::{SettingsHostKind, SettingsMenuState, spawn_settings_panel};
use super::transition::{SessionTransitionKind, SessionTransitionRequest};
use crate::camera::CameraSettings;

/// Asset-relative path for the Main Menu background (under `assets/`).
pub const MAIN_MENU_BACKGROUND_PATH: &str = "images/chasma_background.png";

/// Intrinsic pixel size of [`MAIN_MENU_BACKGROUND_PATH`] (cover math fallback).
const MAIN_MENU_BACKGROUND_INTRINSIC: Vec2 = Vec2::new(1672.0, 941.0);

#[derive(Component, Debug)]
pub struct MainMenuRoot;

/// Full-screen clipped host for the background image (Main Menu only).
#[derive(Component, Debug)]
pub struct MainMenuBackgroundHost;

/// Cover-cropped background image (Main Menu only).
#[derive(Component, Debug)]
pub struct MainMenuBackgroundImage;

/// Dark readability scrim between background and controls.
#[derive(Component, Debug)]
pub struct MainMenuBackgroundOverlay;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuAction {
    NewGame,
    #[cfg(feature = "dev")]
    EditDefaultWorld,
    Settings,
    Credits,
    Quit,
    Back,
}

#[derive(Component, Debug)]
pub struct MainMenuPageHost;

pub fn spawn_main_menu(mut commands: Commands, asset_server: Res<AssetServer>) {
    let background = asset_server.load(MAIN_MENU_BACKGROUND_PATH);

    commands
        .spawn((
            MainMenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            // Solid fallback if the image fails to load / while loading.
            BackgroundColor(Color::srgba(0.04, 0.05, 0.07, 0.96)),
            // Above Dev root ZIndex band (900-980).
            ZIndex(10_000),
        ))
        .with_children(|root| {
            // Layer 1: clipped cover image (does not receive pointer hits).
            root.spawn((
                MainMenuBackgroundHost,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                FocusPolicy::Pass,
            ))
            .with_children(|host| {
                host.spawn((
                    MainMenuBackgroundImage,
                    ImageNode::new(background),
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(MAIN_MENU_BACKGROUND_INTRINSIC.x),
                        height: Val::Px(MAIN_MENU_BACKGROUND_INTRINSIC.y),
                        ..default()
                    },
                    FocusPolicy::Pass,
                ));
            });

            // Layer 2: dark scrim for readable controls (pass hits through).
            root.spawn((
                MainMenuBackgroundOverlay,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.55)),
                FocusPolicy::Pass,
            ));

            // Layer 3: title + controls (spawned after so they stack above).
            root.spawn((
                Text::new("CHASMA"),
                menu_text_font(MENU_TITLE_FONT_SIZE),
                TextColor(Color::srgb(0.92, 0.94, 0.96)),
            ));
            root.spawn((
                Text::new("Main Menu"),
                menu_text_font(MENU_HEADING_FONT_SIZE),
                TextColor(Color::srgb(0.7, 0.74, 0.78)),
            ));
            spawn_button(root, "New Game", MainMenuAction::NewGame);
            #[cfg(feature = "dev")]
            spawn_button(root, "Edit Default World", MainMenuAction::EditDefaultWorld);
            spawn_button(root, "Settings", MainMenuAction::Settings);
            spawn_button(root, "Credits", MainMenuAction::Credits);
            spawn_button(root, "Quit", MainMenuAction::Quit);

            root.spawn((
                MainMenuPageHost,
                Node {
                    margin: UiRect::top(Val::Px(24.0)),
                    width: Val::Percent(92.0),
                    max_width: Val::Px(920.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
            ));
        });
}

/// Cover-crop the Main Menu background to the primary window (aspect preserved).
pub fn sync_main_menu_background_cover(
    windows: Query<&Window, With<PrimaryWindow>>,
    images: Res<Assets<Image>>,
    mut backgrounds: Query<(&ImageNode, &mut Node), With<MainMenuBackgroundImage>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let win = Vec2::new(window.width(), window.height());
    if win.x <= 1.0 || win.y <= 1.0 {
        return;
    }

    for (image_node, mut node) in &mut backgrounds {
        let intrinsic = images
            .get(&image_node.image)
            .map(|image| Vec2::new(image.width() as f32, image.height() as f32))
            .filter(|size| size.x > 0.0 && size.y > 0.0)
            .unwrap_or(MAIN_MENU_BACKGROUND_INTRINSIC);
        let scale = (win.x / intrinsic.x).max(win.y / intrinsic.y);
        let size = intrinsic * scale;
        node.width = Val::Px(size.x);
        node.height = Val::Px(size.y);
        node.left = Val::Px((win.x - size.x) * 0.5);
        node.top = Val::Px((win.y - size.y) * 0.5);
        node.position_type = PositionType::Absolute;
    }
}

/// One-shot warning if the background asset fails; keep solid fallback.
pub fn warn_main_menu_background_load_failure(
    asset_server: Res<AssetServer>,
    backgrounds: Query<&ImageNode, With<MainMenuBackgroundImage>>,
    mut warned: Local<bool>,
) {
    if *warned {
        return;
    }
    for image_node in &backgrounds {
        if let Some(LoadState::Failed(_)) = asset_server.get_load_state(&image_node.image) {
            warn!(
                "Failed to load Main Menu background at `{MAIN_MENU_BACKGROUND_PATH}`; solid fallback remains"
            );
            *warned = true;
            return;
        }
    }
}

fn spawn_button(parent: &mut ChildSpawnerCommands, label: &str, action: MainMenuAction) {
    parent
        .spawn((
            Button,
            action,
            Node {
                min_width: Val::Px(280.0),
                padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                justify_content: JustifyContent::Center,
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

pub fn despawn_main_menu(mut commands: Commands, roots: Query<Entity, With<MainMenuRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

pub fn handle_main_menu_buttons(
    mut interaction: Query<
        (&Interaction, &MainMenuAction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut nav: ResMut<MenuNavigation>,
    mut settings: ResMut<SettingsMenuState>,
    mut transitions: ResMut<SessionTransitionRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, action, mut bg) in &mut interaction {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(Color::srgb(0.28, 0.36, 0.46));
                match *action {
                    MainMenuAction::NewGame => {
                        transitions.request(SessionTransitionKind::StartNewGame);
                    }
                    #[cfg(feature = "dev")]
                    MainMenuAction::EditDefaultWorld => {
                        transitions.request(SessionTransitionKind::StartDefaultWorldAuthoring);
                    }
                    MainMenuAction::Settings => {
                        settings.reset_for_open();
                        nav.go_settings();
                    }
                    MainMenuAction::Credits => nav.go_credits(),
                    MainMenuAction::Quit => {
                        exit.write(AppExit::Success);
                    }
                    MainMenuAction::Back => nav.back_to_root(),
                }
            }
            Interaction::Hovered => *bg = BackgroundColor(Color::srgb(0.22, 0.28, 0.36)),
            Interaction::None => *bg = BackgroundColor(Color::srgb(0.16, 0.2, 0.26)),
        }
    }
}

pub fn sync_main_menu_page(
    nav: Res<MenuNavigation>,
    settings: Res<SettingsMenuState>,
    camera: Res<CameraSettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    host: Query<Entity, With<MainMenuPageHost>>,
    mut root_buttons: Query<(&MainMenuAction, &mut Visibility), With<Button>>,
    mut commands: Commands,
) {
    if !nav.is_changed() && !nav.is_added() && !settings.is_changed() && !settings.is_added() {
        return;
    }
    let show_root = matches!(nav.page, MenuPage::Root);
    for (action, mut vis) in &mut root_buttons {
        *vis = match *action {
            MainMenuAction::Back => {
                if show_root {
                    Visibility::Hidden
                } else {
                    Visibility::Visible
                }
            }
            _ => {
                if show_root {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                }
            }
        };
    }
    let Ok(host_entity) = host.single() else {
        return;
    };
    commands.entity(host_entity).despawn_related::<Children>();
    match nav.page {
        MenuPage::Root => {}
        MenuPage::Settings => {
            let Ok(window) = windows.single() else {
                return;
            };
            commands.entity(host_entity).with_children(|p| {
                spawn_settings_panel(p, SettingsHostKind::Main, &settings, &camera, window);
            });
        }
        MenuPage::Credits => {
            commands.entity(host_entity).with_children(|p| {
                spawn_placeholder_page(p, "Credits", "Chasma - development build.");
            });
        }
        _ => {}
    }
}

fn spawn_placeholder_page(parent: &mut ChildSpawnerCommands, title: &str, body: &str) {
    parent.spawn((
        Text::new(title.to_string()),
        menu_text_font(MENU_HEADING_FONT_SIZE),
        TextColor(Color::srgb(0.92, 0.94, 0.96)),
    ));
    parent.spawn((
        Text::new(body.to_string()),
        menu_text_font(MENU_BODY_FONT_SIZE),
        TextColor(Color::srgb(0.72, 0.76, 0.8)),
    ));
    spawn_button(parent, "Back", MainMenuAction::Back);
}

/// Cover size that fills `window` while preserving `intrinsic` aspect (crop excess).
pub fn cover_size_for_window(window: Vec2, intrinsic: Vec2) -> Vec2 {
    let scale = (window.x / intrinsic.x).max(window.y / intrinsic.y);
    intrinsic * scale
}
