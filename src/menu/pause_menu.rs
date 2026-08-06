//! Pause Menu overlay UI (InGame only).

use bevy::prelude::*;

use bevy::window::PrimaryWindow;

use super::font::{
    MENU_BANNER_FONT_SIZE, MENU_BODY_FONT_SIZE, MENU_BUTTON_FONT_SIZE, MENU_HEADING_FONT_SIZE,
    MENU_TITLE_FONT_SIZE, PAUSE_CONTROL_FONT_SIZE, PauseMenuText, menu_text_font,
};
use super::navigation::{MenuNavigation, MenuPage, authoring_banner_label};
use super::screen::{GameSessionKind, GameSessionState};
use super::settings::{SettingsHostKind, SettingsMenuState, spawn_settings_panel};
use super::systems::close_pause_menu;
use super::transition::{SessionTransitionKind, SessionTransitionRequest};
use crate::camera::CameraSettings;
use crate::simulation::SimulationControlState;
use crate::ui::gameplay::InventoryUiState;

#[derive(Component, Debug)]
pub struct PauseMenuRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseMenuAction {
    Resume,
    Settings,
    ReturnToMainMenu,
    QuitToDesktop,
    Back,
    ConfirmReturn,
    ConfirmQuit,
    CancelConfirm,
}

pub fn spawn_pause_menu(commands: &mut Commands) {
    commands
        .spawn((
            PauseMenuRoot,
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
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.72)),
            ZIndex(10_200),
        ))
        .with_children(|root| {
            root.spawn((
                PauseAuthoringBanner,
                PauseMenuText,
                Text::new(""),
                menu_text_font(MENU_BANNER_FONT_SIZE),
                TextColor(Color::srgb(0.95, 0.75, 0.35)),
            ));
            root.spawn((
                PauseMenuText,
                Text::new("Paused"),
                menu_text_font(MENU_TITLE_FONT_SIZE),
                TextColor(Color::srgb(0.92, 0.94, 0.96)),
            ));
            root.spawn((
                Button,
                PauseMenuAction::Resume,
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
                    PauseResumeButtonLabel,
                    PauseMenuText,
                    Text::new("Resume"),
                    menu_text_font(PAUSE_CONTROL_FONT_SIZE),
                    TextColor(Color::srgb(0.92, 0.94, 0.96)),
                ));
            });
            spawn_pause_button(root, "Settings", PauseMenuAction::Settings);
            spawn_pause_button(
                root,
                "Return to Main Menu",
                PauseMenuAction::ReturnToMainMenu,
            );
            spawn_pause_button(root, "Quit to Desktop", PauseMenuAction::QuitToDesktop);
            root.spawn((
                PauseMenuPageHost,
                Node {
                    margin: UiRect::top(Val::Px(20.0)),
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

#[derive(Component, Debug)]
pub struct PauseMenuPageHost;

#[derive(Component, Debug)]
pub struct PauseAuthoringBanner;

#[derive(Component, Debug)]
pub struct PauseResumeButtonLabel;

fn spawn_pause_button(parent: &mut ChildSpawnerCommands, label: &str, action: PauseMenuAction) {
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
                PauseMenuText,
                Text::new(label.to_string()),
                menu_text_font(MENU_BUTTON_FONT_SIZE),
                TextColor(Color::srgb(0.92, 0.94, 0.96)),
            ));
        });
}

pub fn handle_pause_menu_buttons(
    mut interaction: Query<
        (&Interaction, &PauseMenuAction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut nav: ResMut<MenuNavigation>,
    mut settings: ResMut<SettingsMenuState>,
    mut pause_ctx: ResMut<super::navigation::PauseMenuContext>,
    mut control: ResMut<SimulationControlState>,
    mut inventory: ResMut<InventoryUiState>,
    mut transitions: ResMut<SessionTransitionRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, action, mut bg) in &mut interaction {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(Color::srgb(0.28, 0.36, 0.46));
                match *action {
                    PauseMenuAction::Resume => {
                        close_pause_menu(&mut nav, &mut pause_ctx, &mut control);
                    }
                    PauseMenuAction::Settings => {
                        settings.reset_for_open();
                        nav.go_settings();
                    }
                    PauseMenuAction::ReturnToMainMenu => nav.go_confirm_return(),
                    PauseMenuAction::QuitToDesktop => nav.go_confirm_quit(),
                    PauseMenuAction::Back | PauseMenuAction::CancelConfirm => nav.back_to_root(),
                    PauseMenuAction::ConfirmReturn => {
                        inventory.cancel_drag(None);
                        pause_ctx.active = false;
                        pause_ctx.was_simulation_paused = false;
                        nav.close_pause();
                        control.pause();
                        transitions.request(SessionTransitionKind::ReturnToMainMenu);
                    }
                    PauseMenuAction::ConfirmQuit => {
                        exit.write(AppExit::Success);
                    }
                }
            }
            Interaction::Hovered => *bg = BackgroundColor(Color::srgb(0.22, 0.28, 0.36)),
            Interaction::None => *bg = BackgroundColor(Color::srgb(0.16, 0.2, 0.26)),
        }
    }
}

pub fn sync_pause_menu_page(
    nav: Res<MenuNavigation>,
    settings: Res<SettingsMenuState>,
    camera: Res<CameraSettings>,
    session: Res<GameSessionState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    host: Query<Entity, With<PauseMenuPageHost>>,
    mut banner: Query<&mut Text, With<PauseAuthoringBanner>>,
    mut resume_label: Query<
        &mut Text,
        (With<PauseResumeButtonLabel>, Without<PauseAuthoringBanner>),
    >,
    mut root_buttons: Query<(&PauseMenuAction, &mut Visibility), With<Button>>,
    mut commands: Commands,
) {
    if let Ok(mut text) = banner.single_mut() {
        let label = authoring_banner_label(session.kind).unwrap_or("");
        if text.as_str() != label {
            **text = label.to_string();
        }
    }
    if let Ok(mut text) = resume_label.single_mut() {
        let label = if session.kind == GameSessionKind::DefaultWorldAuthoring {
            "Resume Editing"
        } else {
            "Resume"
        };
        if text.as_str() != label {
            **text = label.to_string();
        }
    }

    if !nav.is_changed() && !nav.is_added() && !settings.is_changed() && !settings.is_added() {
        return;
    }

    let hide_roots_for_settings = matches!(nav.page, MenuPage::Settings);
    for (action, mut vis) in &mut root_buttons {
        *vis = match *action {
            PauseMenuAction::Back
            | PauseMenuAction::CancelConfirm
            | PauseMenuAction::ConfirmReturn
            | PauseMenuAction::ConfirmQuit => Visibility::Inherited,
            _ => {
                if hide_roots_for_settings {
                    Visibility::Hidden
                } else {
                    Visibility::Visible
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
                spawn_settings_panel(p, SettingsHostKind::Pause, &settings, &camera, window);
            });
        }
        MenuPage::ConfirmReturnToMainMenu => {
            commands.entity(host_entity).with_children(|p| {
                spawn_confirm(
                    p,
                    "Return to Main Menu?",
                    "The current world stays in memory. Persistence is not connected.",
                    PauseMenuAction::ConfirmReturn,
                    PauseMenuAction::CancelConfirm,
                );
            });
        }
        MenuPage::ConfirmQuitToDesktop => {
            commands.entity(host_entity).with_children(|p| {
                spawn_confirm(
                    p,
                    "Quit to Desktop?",
                    "Unsaved work is not protected in this pass.",
                    PauseMenuAction::ConfirmQuit,
                    PauseMenuAction::CancelConfirm,
                );
            });
        }
        MenuPage::Credits => {}
    }
}

fn spawn_confirm(
    parent: &mut ChildSpawnerCommands,
    title: &str,
    body: &str,
    confirm: PauseMenuAction,
    cancel: PauseMenuAction,
) {
    parent.spawn((
        PauseMenuText,
        Text::new(title.to_string()),
        menu_text_font(MENU_HEADING_FONT_SIZE),
        TextColor(Color::srgb(0.92, 0.94, 0.96)),
    ));
    parent.spawn((
        PauseMenuText,
        Text::new(body.to_string()),
        menu_text_font(MENU_BODY_FONT_SIZE),
        TextColor(Color::srgb(0.72, 0.76, 0.8)),
    ));
    spawn_pause_button(parent, "Confirm", confirm);
    spawn_pause_button(parent, "Cancel", cancel);
}

/// Absolute font sizes used by Pause Menu text (for regression tests).
pub fn pause_menu_font_sizes() -> [f32; 4] {
    [
        MENU_BANNER_FONT_SIZE,
        MENU_TITLE_FONT_SIZE,
        MENU_BUTTON_FONT_SIZE,
        MENU_BODY_FONT_SIZE,
    ]
}
