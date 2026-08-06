//! Shared Settings menu (Main Menu + Pause Menu).
//!
//! Client-local only: live Display / Camera controls and read-only Controls.
//! No persistence, rebinding, audio, or resolution enumeration.

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode};

use super::font::{
    MENU_BODY_FONT_SIZE, MENU_BUTTON_FONT_SIZE, MENU_HEADING_FONT_SIZE, MENU_TITLE_FONT_SIZE,
    PauseMenuText, menu_text_font,
};
use super::main_menu::MainMenuAction;
use super::pause_menu::PauseMenuAction;
use crate::camera::CameraSettings;

/// Active Settings category (menu-layer only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsCategory {
    #[default]
    Display,
    Camera,
    Controls,
}

impl SettingsCategory {
    pub const ALL: [Self; 3] = [Self::Display, Self::Camera, Self::Controls];

    pub fn label(self) -> &'static str {
        match self {
            Self::Display => "Display",
            Self::Camera => "Camera",
            Self::Controls => "Controls",
        }
    }
}

/// Settings category + rebuild token for the shared Settings panel.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SettingsMenuState {
    pub category: SettingsCategory,
    /// Bumped when live values change so hosts refresh labels.
    pub revision: u32,
}

impl SettingsMenuState {
    pub fn reset_for_open(&mut self) {
        self.category = SettingsCategory::Display;
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn select(&mut self, category: SettingsCategory) {
        if self.category != category {
            self.category = category;
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub fn refresh(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}

/// Which shell owns the Settings Back button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsHostKind {
    Main,
    Pause,
}

/// Shared Settings control actions (category + live toggles / adjustments).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum SettingsAction {
    SelectCategory(SettingsCategory),
    ToggleBorderless,
    ToggleVsync,
    AdjustPanSpeed(i8),
    AdjustFastPan(i8),
    AdjustRotate(i8),
    AdjustZoom(i8),
}

const PANEL_BG: Color = Color::srgba(0.05, 0.07, 0.1, 0.94);
const CATEGORY_IDLE: Color = Color::srgb(0.14, 0.17, 0.22);
const CATEGORY_SELECTED: Color = Color::srgb(0.28, 0.36, 0.46);
const ROW_BG: Color = Color::srgb(0.1, 0.13, 0.17);
const CONTROL_BG: Color = Color::srgb(0.16, 0.2, 0.26);
const TEXT_PRIMARY: Color = Color::srgb(0.92, 0.94, 0.96);
const TEXT_MUTED: Color = Color::srgb(0.72, 0.76, 0.8);

/// Spawn the two-column Settings panel into a Main/Pause page host.
pub fn spawn_settings_panel(
    parent: &mut ChildSpawnerCommands,
    host: SettingsHostKind,
    state: &SettingsMenuState,
    camera: &CameraSettings,
    window: &Window,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                max_width: Val::Px(920.0),
                min_width: Val::Px(420.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(12.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            BorderColor::all(Color::srgb(0.22, 0.28, 0.34)),
        ))
        .with_children(|panel| {
            spawn_text(panel, host, "Settings", MENU_TITLE_FONT_SIZE, TEXT_PRIMARY);

            panel
                .spawn((Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(14.0),
                    min_height: Val::Px(220.0),
                    align_items: AlignItems::Stretch,
                    ..default()
                },))
                .with_children(|row| {
                    spawn_category_column(row, host, state.category);
                    spawn_content_column(row, host, state.category, camera, window);
                });

            spawn_back_button(panel, host);
        });
}

fn spawn_category_column(
    parent: &mut ChildSpawnerCommands,
    host: SettingsHostKind,
    selected: SettingsCategory,
) {
    parent
        .spawn((Node {
            width: Val::Percent(28.0),
            min_width: Val::Px(140.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        },))
        .with_children(|col| {
            spawn_text(col, host, "Categories", MENU_HEADING_FONT_SIZE, TEXT_MUTED);
            for category in SettingsCategory::ALL {
                let selected_here = category == selected;
                col.spawn((
                    Button,
                    SettingsAction::SelectCategory(category),
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(36.0),
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                        justify_content: JustifyContent::FlexStart,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(if selected_here {
                        CATEGORY_SELECTED
                    } else {
                        CATEGORY_IDLE
                    }),
                ))
                .with_children(|btn| {
                    spawn_text(
                        btn,
                        host,
                        category.label(),
                        MENU_BUTTON_FONT_SIZE,
                        TEXT_PRIMARY,
                    );
                });
            }
        });
}

fn spawn_content_column(
    parent: &mut ChildSpawnerCommands,
    host: SettingsHostKind,
    category: SettingsCategory,
    camera: &CameraSettings,
    window: &Window,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(72.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.1, 0.13, 0.85)),
        ))
        .with_children(|col| {
            spawn_text(
                col,
                host,
                category.label(),
                MENU_HEADING_FONT_SIZE,
                TEXT_PRIMARY,
            );
            match category {
                SettingsCategory::Display => spawn_display_rows(col, host, window),
                SettingsCategory::Camera => spawn_camera_rows(col, host, camera),
                SettingsCategory::Controls => spawn_controls_rows(col, host),
            }
        });
}

fn spawn_display_rows(parent: &mut ChildSpawnerCommands, host: SettingsHostKind, window: &Window) {
    let borderless = matches!(window.mode, WindowMode::BorderlessFullscreen(_));
    let vsync = matches!(
        window.present_mode,
        PresentMode::AutoVsync | PresentMode::Fifo | PresentMode::FifoRelaxed
    );

    spawn_toggle_row(
        parent,
        host,
        "Borderless Fullscreen",
        if borderless { "On" } else { "Off" },
        SettingsAction::ToggleBorderless,
    );
    spawn_toggle_row(
        parent,
        host,
        "VSync",
        if vsync { "On" } else { "Off" },
        SettingsAction::ToggleVsync,
    );
}

fn spawn_camera_rows(
    parent: &mut ChildSpawnerCommands,
    host: SettingsHostKind,
    camera: &CameraSettings,
) {
    spawn_adjust_row(
        parent,
        host,
        "Pan Speed",
        &format!("{:.0}", camera.pan_speed),
        SettingsAction::AdjustPanSpeed(-1),
        SettingsAction::AdjustPanSpeed(1),
    );
    spawn_adjust_row(
        parent,
        host,
        "Fast Pan Multiplier",
        &format!("{:.2}", camera.fast_pan_multiplier),
        SettingsAction::AdjustFastPan(-1),
        SettingsAction::AdjustFastPan(1),
    );
    spawn_adjust_row(
        parent,
        host,
        "Rotate Sensitivity",
        &format!("{:.3}", camera.rotate_sensitivity),
        SettingsAction::AdjustRotate(-1),
        SettingsAction::AdjustRotate(1),
    );
    spawn_adjust_row(
        parent,
        host,
        "Zoom Speed",
        &format!("{:.2}", camera.zoom_speed),
        SettingsAction::AdjustZoom(-1),
        SettingsAction::AdjustZoom(1),
    );
}

fn spawn_controls_rows(parent: &mut ChildSpawnerCommands, host: SettingsHostKind) {
    const ROWS: [(&str, &str); 8] = [
        ("Camera Pan", "WASD"),
        ("Fast Pan", "Shift + WASD"),
        ("Rotate Camera", "Middle Mouse Drag"),
        ("Zoom", "Mouse Wheel"),
        ("Pause Menu", "Escape"),
        ("Simulation Pause", "Space"),
        ("Inventory", "I"),
        ("Build Mode", "B"),
    ];
    for (label, binding) in ROWS {
        spawn_readonly_row(parent, host, label, binding);
    }
    #[cfg(feature = "dev")]
    spawn_readonly_row(parent, host, "Dev Mode", "F12");
}

fn spawn_toggle_row(
    parent: &mut ChildSpawnerCommands,
    host: SettingsHostKind,
    label: &str,
    value: &str,
    action: SettingsAction,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                min_height: Val::Px(36.0),
                ..default()
            },
            BackgroundColor(ROW_BG),
        ))
        .with_children(|row| {
            spawn_text(row, host, label, MENU_BODY_FONT_SIZE, TEXT_PRIMARY);
            row.spawn((
                Button,
                action,
                Node {
                    min_width: Val::Px(88.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(CONTROL_BG),
            ))
            .with_children(|btn| {
                spawn_text(btn, host, value, MENU_BODY_FONT_SIZE, TEXT_PRIMARY);
            });
        });
}

fn spawn_adjust_row(
    parent: &mut ChildSpawnerCommands,
    host: SettingsHostKind,
    label: &str,
    value: &str,
    dec: SettingsAction,
    inc: SettingsAction,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                min_height: Val::Px(36.0),
                ..default()
            },
            BackgroundColor(ROW_BG),
        ))
        .with_children(|row| {
            spawn_text(row, host, label, MENU_BODY_FONT_SIZE, TEXT_PRIMARY);
            row.spawn((Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },))
                .with_children(|controls| {
                    spawn_small_button(controls, host, "-", dec);
                    spawn_text(controls, host, value, MENU_BODY_FONT_SIZE, TEXT_MUTED);
                    spawn_small_button(controls, host, "+", inc);
                });
        });
}

fn spawn_readonly_row(
    parent: &mut ChildSpawnerCommands,
    host: SettingsHostKind,
    label: &str,
    value: &str,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                min_height: Val::Px(32.0),
                ..default()
            },
            BackgroundColor(ROW_BG),
        ))
        .with_children(|row| {
            spawn_text(row, host, label, MENU_BODY_FONT_SIZE, TEXT_PRIMARY);
            spawn_text(row, host, value, MENU_BODY_FONT_SIZE, TEXT_MUTED);
        });
}

fn spawn_small_button(
    parent: &mut ChildSpawnerCommands,
    host: SettingsHostKind,
    label: &str,
    action: SettingsAction,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                min_width: Val::Px(32.0),
                min_height: Val::Px(28.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(CONTROL_BG),
        ))
        .with_children(|btn| {
            spawn_text(btn, host, label, MENU_BODY_FONT_SIZE, TEXT_PRIMARY);
        });
}

fn spawn_back_button(parent: &mut ChildSpawnerCommands, host: SettingsHostKind) {
    match host {
        SettingsHostKind::Main => {
            parent
                .spawn((
                    Button,
                    MainMenuAction::Back,
                    Node {
                        min_width: Val::Px(160.0),
                        padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                        justify_content: JustifyContent::Center,
                        align_self: AlignSelf::FlexStart,
                        ..default()
                    },
                    BackgroundColor(CONTROL_BG),
                ))
                .with_children(|btn| {
                    spawn_text(btn, host, "Back", MENU_BUTTON_FONT_SIZE, TEXT_PRIMARY);
                });
        }
        SettingsHostKind::Pause => {
            parent
                .spawn((
                    Button,
                    PauseMenuAction::Back,
                    Node {
                        min_width: Val::Px(160.0),
                        padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                        justify_content: JustifyContent::Center,
                        align_self: AlignSelf::FlexStart,
                        ..default()
                    },
                    BackgroundColor(CONTROL_BG),
                ))
                .with_children(|btn| {
                    spawn_text(btn, host, "Back", MENU_BUTTON_FONT_SIZE, TEXT_PRIMARY);
                });
        }
    }
}

fn spawn_text(
    parent: &mut ChildSpawnerCommands,
    host: SettingsHostKind,
    label: &str,
    size: f32,
    color: Color,
) {
    let mut entity = parent.spawn((
        Text::new(label.to_string()),
        menu_text_font(size),
        TextColor(color),
    ));
    if matches!(host, SettingsHostKind::Pause) {
        entity.insert(PauseMenuText);
    }
}

/// Handle shared Settings controls (live Display + Camera).
pub fn handle_settings_actions(
    mut interaction: Query<
        (&Interaction, &SettingsAction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut settings: ResMut<SettingsMenuState>,
    mut camera: ResMut<CameraSettings>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    for (interaction, action, mut bg) in &mut interaction {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(CATEGORY_SELECTED);
                match *action {
                    SettingsAction::SelectCategory(category) => {
                        settings.select(category);
                    }
                    SettingsAction::ToggleBorderless => {
                        if let Ok(mut window) = windows.single_mut() {
                            window.mode = match window.mode {
                                WindowMode::BorderlessFullscreen(_) => WindowMode::Windowed,
                                _ => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                            };
                            settings.refresh();
                        }
                    }
                    SettingsAction::ToggleVsync => {
                        if let Ok(mut window) = windows.single_mut() {
                            window.present_mode = match window.present_mode {
                                PresentMode::AutoVsync
                                | PresentMode::Fifo
                                | PresentMode::FifoRelaxed => PresentMode::AutoNoVsync,
                                _ => PresentMode::AutoVsync,
                            };
                            settings.refresh();
                        }
                    }
                    SettingsAction::AdjustPanSpeed(dir) => {
                        let step = 32.0 * f32::from(dir);
                        camera.pan_speed = (camera.pan_speed + step).clamp(32.0, 1024.0);
                        settings.refresh();
                    }
                    SettingsAction::AdjustFastPan(dir) => {
                        let step = 0.25 * f32::from(dir);
                        camera.fast_pan_multiplier =
                            (camera.fast_pan_multiplier + step).clamp(1.0, 5.0);
                        settings.refresh();
                    }
                    SettingsAction::AdjustRotate(dir) => {
                        let step = 0.001 * f32::from(dir);
                        camera.rotate_sensitivity =
                            (camera.rotate_sensitivity + step).clamp(0.001, 0.02);
                        settings.refresh();
                    }
                    SettingsAction::AdjustZoom(dir) => {
                        let step = 0.02 * f32::from(dir);
                        camera.zoom_speed = (camera.zoom_speed + step).clamp(0.02, 0.4);
                        settings.refresh();
                    }
                }
            }
            Interaction::Hovered => {
                if !matches!(*action, SettingsAction::SelectCategory(_)) {
                    *bg = BackgroundColor(Color::srgb(0.22, 0.28, 0.36));
                }
            }
            Interaction::None => {
                if let SettingsAction::SelectCategory(category) = *action {
                    *bg = BackgroundColor(if category == settings.category {
                        CATEGORY_SELECTED
                    } else {
                        CATEGORY_IDLE
                    });
                } else {
                    *bg = BackgroundColor(CONTROL_BG);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_category_is_display() {
        assert_eq!(
            SettingsMenuState::default().category,
            SettingsCategory::Display
        );
    }

    #[test]
    fn reset_for_open_selects_display() {
        let mut state = SettingsMenuState {
            category: SettingsCategory::Controls,
            revision: 3,
        };
        state.reset_for_open();
        assert_eq!(state.category, SettingsCategory::Display);
        assert_eq!(state.revision, 4);
    }

    #[test]
    fn select_category_bumps_revision() {
        let mut state = SettingsMenuState::default();
        state.select(SettingsCategory::Camera);
        assert_eq!(state.category, SettingsCategory::Camera);
        assert_eq!(state.revision, 1);
        state.select(SettingsCategory::Camera);
        assert_eq!(state.revision, 1);
    }

    #[test]
    fn category_labels_are_stable() {
        assert_eq!(SettingsCategory::Display.label(), "Display");
        assert_eq!(SettingsCategory::Camera.label(), "Camera");
        assert_eq!(SettingsCategory::Controls.label(), "Controls");
    }
}
