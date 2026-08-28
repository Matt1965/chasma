//! Dev-window shell spawn (Slice 3).

use bevy::prelude::*;

use bevy::ui::FocusPolicy;

use super::components::{
    DevLauncherGroup, DevWindowBody, DevWindowCloseButton, DevWindowCollapseButton,
    DevWindowCollapseButtonLabel, DevWindowRoot, DevWindowTitleBarDragRegion, DevWindowUi,
    DevWorkspaceLauncher, DevWorkspaceLauncherButton, DevWorkspaceLauncherButtons,
    DevWorkspaceLauncherToggle,
};

use super::id::DevWindowId;

use super::math::{
    DEFAULT_PANEL_BODY_PADDING_PX, DEFAULT_PANEL_WIDTH_PX, LAUNCHER_LEFT_PX, LAUNCHER_TOP_PX,
    TITLE_BAR_HEIGHT_PX, default_catalog_position, default_debug_position, default_fields_position,
    default_navigation_editor_position, default_save_position, default_selected_object_position,
    default_world_position, navigation_editor_body_max_height, navigation_editor_panel_width,
};

use super::state::DevWindowRegistry;
use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::DevTooltipTarget;
use crate::dev::widgets::DevButtonChrome;
use crate::dev::widgets::theme::{
    LAUNCHER_BG, LAUNCHER_LABEL_TEXT, WINDOW_BG, WINDOW_TITLE_TEXT, window_title_font,
};

const TITLE_BTN: Color = crate::dev::widgets::theme::TITLE_BTN_IDLE;

const TITLE_BTN_HOVER: Color = crate::dev::widgets::theme::TITLE_BTN_HOVER;

/// Spawn workspace launcher + dev window shells; bodies filled by panel setup systems.

pub fn setup_dev_workspace(mut commands: Commands, registry: Res<DevWindowRegistry>) {
    spawn_workspace_launcher(&mut commands);

    spawn_save_window(&mut commands, registry.session(DevWindowId::Save));

    spawn_catalog_window(&mut commands, registry.session(DevWindowId::Catalog));

    spawn_selected_object_window(&mut commands, registry.session(DevWindowId::SelectedObject));

    spawn_navigation_editor_window(
        &mut commands,
        registry.session(DevWindowId::NavigationEditor),
    );

    spawn_debug_window(&mut commands, registry.session(DevWindowId::Debug));

    spawn_world_window(&mut commands, registry.session(DevWindowId::World));

    spawn_fields_window(&mut commands, registry.session(DevWindowId::Fields));
}

fn spawn_workspace_launcher(commands: &mut Commands) {
    commands
        .spawn((
            DevWorkspaceLauncher,
            DevWindowUi,
            Node {
                position_type: PositionType::Absolute,

                left: Val::Px(LAUNCHER_LEFT_PX),

                top: Val::Px(LAUNCHER_TOP_PX),

                flex_direction: FlexDirection::Column,

                row_gap: Val::Px(4.0),

                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),

                ..default()
            },
            BackgroundColor(LAUNCHER_BG),
            FocusPolicy::Block,
            ZIndex(980),
            Visibility::Hidden,
        ))
        .with_children(|column| {
            spawn_launcher_row(
                column,
                DevLauncherGroup::Windows,
                "Windows",
                "Show or hide primary dev windows: Save, Catalog, and Selected Object.",
                DevWindowId::WINDOWS_LAUNCHER,
            );

            spawn_launcher_row(
                column,
                DevLauncherGroup::Advanced,
                "Advanced",
                "Show or hide advanced authoring windows: Debug, World, Fields, and Navigation Editor.",
                DevWindowId::ADVANCED_LAUNCHER,
            );
        });
}

fn spawn_launcher_row(
    parent: &mut ChildSpawnerCommands<'_>,

    group: DevLauncherGroup,

    title: &str,

    tooltip: &str,

    windows: &[DevWindowId],
) {
    parent
        .spawn((
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
                DevWorkspaceLauncherToggle { group },
                DevTooltipTarget::new(tooltip),
                DevWindowUi,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),

                    ..default()
                },
                BackgroundColor(Color::NONE),
                Text::new(title),
                window_title_font(),
                TextColor(LAUNCHER_LABEL_TEXT),
            ));

            row.spawn((
                DevWorkspaceLauncherButtons { group },
                DevWindowUi,
                Node {
                    flex_direction: FlexDirection::Row,

                    column_gap: Val::Px(6.0),

                    align_items: AlignItems::Center,

                    flex_wrap: FlexWrap::Wrap,

                    ..default()
                },
            ))
            .with_children(|buttons| {
                for &window in windows {
                    spawn_launcher_button(buttons, window);
                }
            });
        });
}

fn spawn_launcher_button(parent: &mut ChildSpawnerCommands, window: DevWindowId) {
    let tooltip = match window {
        DevWindowId::Save => {
            "Toggle the Save window (scene snapshots). Hidden windows reopen at the same position."
        }
        DevWindowId::Catalog => "Toggle the Catalog window (asset discovery and placement).",
        DevWindowId::SelectedObject => {
            "Toggle the Selected Object window (inspection and transform controls)."
        }
        DevWindowId::NavigationEditor => {
            "Toggle the Navigation Editor (building blueprint authoring)."
        }
        DevWindowId::Debug => "Toggle the Debug window (runtime diagnostic overlays).",
        DevWindowId::World => "Toggle the World window (time, cycle, and environment).",
        DevWindowId::Fields => "Toggle the Fields window (terrain field tools).",
    };
    parent.spawn((
        DevWorkspaceLauncherButton { window },
        DevTooltipTarget::new(tooltip),
        DevWindowUi,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),

            ..default()
        },
        BackgroundColor(TITLE_BTN),
        Text::new(window.launcher_label()),
        TextFont {
            font_size: 12.0,

            ..default()
        },
        TextColor(Color::srgba(0.85, 0.92, 0.98, 1.0)),
    ));
}

/// Spawn the Save window shell.

pub fn spawn_save_window(
    commands: &mut Commands,

    session: Option<&super::state::DevWindowSessionState>,
) {
    let position = session
        .map(|s| s.position)
        .unwrap_or_else(|| default_save_position(Vec2::new(1280.0, 720.0), DEFAULT_PANEL_WIDTH_PX));

    commands
        .spawn((
            DevWindowRoot {
                id: DevWindowId::Save,
            },
            DevWindowUi,
            Node {
                position_type: PositionType::Absolute,

                left: Val::Px(position.x),

                top: Val::Px(position.y),

                width: Val::Px(DEFAULT_PANEL_WIDTH_PX),

                flex_direction: FlexDirection::Column,

                row_gap: Val::Px(0.0),

                ..default()
            },
            BackgroundColor(WINDOW_BG),
            FocusPolicy::Block,
            ZIndex(899),
            Visibility::Hidden,
        ))
        .with_children(|window| {
            spawn_title_bar(window, DevWindowId::Save);

            window.spawn((
                DevWindowBody {
                    id: DevWindowId::Save,
                },
                DevWindowUi,
                Node {
                    flex_direction: FlexDirection::Column,

                    row_gap: Val::Px(6.0),

                    padding: UiRect::all(Val::Px(DEFAULT_PANEL_BODY_PADDING_PX)),

                    width: Val::Percent(100.0),

                    max_height: Val::Px(520.0),

                    overflow: Overflow::scroll_y(),

                    ..default()
                },
                Visibility::Visible,
            ));
        });
}

/// Spawn the catalog window shell; returns the body entity id for panel content.

pub fn spawn_catalog_window(
    commands: &mut Commands,

    session: Option<&super::state::DevWindowSessionState>,
) {
    let position = session
        .map(|s| s.position)
        .unwrap_or(Vec2::new(900.0, 46.0));

    commands
        .spawn((
            DevWindowRoot {
                id: DevWindowId::Catalog,
            },
            DevWindowUi,
            Node {
                position_type: PositionType::Absolute,

                left: Val::Px(position.x),

                top: Val::Px(position.y),

                width: Val::Px(DEFAULT_PANEL_WIDTH_PX),

                flex_direction: FlexDirection::Column,

                row_gap: Val::Px(0.0),

                ..default()
            },
            BackgroundColor(WINDOW_BG),
            FocusPolicy::Block,
            ZIndex(900),
            Visibility::Hidden,
        ))
        .with_children(|window| {
            spawn_title_bar(window, DevWindowId::Catalog);

            window.spawn((
                DevWindowBody {
                    id: DevWindowId::Catalog,
                },
                DevWindowUi,
                Node {
                    flex_direction: FlexDirection::Column,

                    row_gap: Val::Px(6.0),

                    padding: UiRect::all(Val::Px(DEFAULT_PANEL_BODY_PADDING_PX)),

                    width: Val::Percent(100.0),

                    ..default()
                },
                Visibility::Visible,
            ));
        });
}

/// Spawn the Selected Object window shell (Slice 5).

pub fn spawn_selected_object_window(
    commands: &mut Commands,

    session: Option<&super::state::DevWindowSessionState>,
) {
    let position = session.map(|s| s.position).unwrap_or_else(|| {
        default_selected_object_position(Vec2::new(1280.0, 720.0), DEFAULT_PANEL_WIDTH_PX)
    });

    commands
        .spawn((
            DevWindowRoot {
                id: DevWindowId::SelectedObject,
            },
            DevWindowUi,
            Node {
                position_type: PositionType::Absolute,

                left: Val::Px(position.x),

                top: Val::Px(position.y),

                width: Val::Px(DEFAULT_PANEL_WIDTH_PX),

                flex_direction: FlexDirection::Column,

                row_gap: Val::Px(0.0),

                ..default()
            },
            BackgroundColor(WINDOW_BG),
            FocusPolicy::Block,
            ZIndex(901),
            Visibility::Hidden,
        ))
        .with_children(|window| {
            spawn_title_bar(window, DevWindowId::SelectedObject);

            window.spawn((
                DevWindowBody {
                    id: DevWindowId::SelectedObject,
                },
                DevWindowUi,
                Node {
                    flex_direction: FlexDirection::Column,

                    row_gap: Val::Px(6.0),

                    padding: UiRect::all(Val::Px(DEFAULT_PANEL_BODY_PADDING_PX)),

                    width: Val::Percent(100.0),

                    ..default()
                },
                Visibility::Visible,
            ));
        });
}

/// Spawn the Navigation Editor window shell (Slice 7).

pub fn spawn_navigation_editor_window(
    commands: &mut Commands,

    session: Option<&super::state::DevWindowSessionState>,
) {
    let position = session.map(|s| s.position).unwrap_or_else(|| {
        default_navigation_editor_position(
            Vec2::new(1280.0, 720.0),
            navigation_editor_panel_width(Vec2::new(1280.0, 720.0)),
        )
    });
    let viewport = Vec2::new(1280.0, 720.0);
    let panel_width = navigation_editor_panel_width(viewport);
    let body_max_height = navigation_editor_body_max_height(viewport, position.y);

    commands
        .spawn((
            DevWindowRoot {
                id: DevWindowId::NavigationEditor,
            },
            DevWindowUi,
            Node {
                position_type: PositionType::Absolute,

                left: Val::Px(position.x),

                top: Val::Px(position.y),

                width: Val::Px(panel_width),

                flex_direction: FlexDirection::Column,

                row_gap: Val::Px(0.0),

                ..default()
            },
            BackgroundColor(WINDOW_BG),
            FocusPolicy::Block,
            ZIndex(902),
            Visibility::Hidden,
        ))
        .with_children(|window| {
            spawn_title_bar(window, DevWindowId::NavigationEditor);

            window.spawn((
                DevWindowBody {
                    id: DevWindowId::NavigationEditor,
                },
                DevWindowUi,
                Node {
                    flex_direction: FlexDirection::Column,

                    row_gap: Val::Px(6.0),

                    padding: UiRect::all(Val::Px(DEFAULT_PANEL_BODY_PADDING_PX)),

                    width: Val::Percent(100.0),

                    min_height: Val::Px(0.0),

                    max_height: Val::Px(body_max_height),

                    overflow: Overflow::scroll_y(),

                    ..default()
                },
                Visibility::Visible,
            ));
        });
}

fn spawn_advanced_window_shell(
    commands: &mut Commands,

    id: DevWindowId,

    session: Option<&super::state::DevWindowSessionState>,

    default_position: impl FnOnce(Vec2, f32) -> Vec2,

    width: f32,

    max_body_height: Option<f32>,

    min_body_height: Option<f32>,

    z_index: i32,
) {
    let viewport = Vec2::new(1280.0, 720.0);

    let position = session
        .map(|s| s.position)
        .unwrap_or_else(|| default_position(viewport, width));

    let mut body_node = Node {
        flex_direction: FlexDirection::Column,

        row_gap: Val::Px(6.0),

        padding: UiRect::all(Val::Px(DEFAULT_PANEL_BODY_PADDING_PX)),

        width: Val::Percent(100.0),

        min_height: Val::Px(0.0),

        overflow: Overflow::scroll_y(),

        ..default()
    };

    if let Some(height) = max_body_height {
        body_node.max_height = Val::Px(height);
    }

    if let Some(height) = min_body_height {
        body_node.min_height = Val::Px(height);
    }

    commands
        .spawn((
            DevWindowRoot { id },
            DevWindowUi,
            Node {
                position_type: PositionType::Absolute,

                left: Val::Px(position.x),

                top: Val::Px(position.y),

                width: Val::Px(width),

                flex_direction: FlexDirection::Column,

                row_gap: Val::Px(0.0),

                ..default()
            },
            BackgroundColor(WINDOW_BG),
            FocusPolicy::Block,
            ZIndex(z_index),
            Visibility::Hidden,
        ))
        .with_children(|window| {
            spawn_title_bar(window, id);

            window.spawn((
                DevWindowBody { id },
                DevWindowUi,
                body_node,
                Visibility::Visible,
            ));
        });
}

/// Spawn the Debug window shell (Slice 8).

pub fn spawn_debug_window(
    commands: &mut Commands,

    session: Option<&super::state::DevWindowSessionState>,
) {
    spawn_advanced_window_shell(
        commands,
        DevWindowId::Debug,
        session,
        default_debug_position,
        DEFAULT_PANEL_WIDTH_PX,
        Some(520.0),
        Some(420.0),
        903,
    );
}

/// Spawn the World window shell (Slice 8).

pub fn spawn_world_window(
    commands: &mut Commands,

    session: Option<&super::state::DevWindowSessionState>,
) {
    spawn_advanced_window_shell(
        commands,
        DevWindowId::World,
        session,
        default_world_position,
        DEFAULT_PANEL_WIDTH_PX,
        Some(620.0),
        None,
        904,
    );
}

/// Spawn the Fields window shell (Slice 8).

pub fn spawn_fields_window(
    commands: &mut Commands,

    session: Option<&super::state::DevWindowSessionState>,
) {
    spawn_advanced_window_shell(
        commands,
        DevWindowId::Fields,
        session,
        default_fields_position,
        DEFAULT_PANEL_WIDTH_PX + 40.0,
        Some(540.0),
        None,
        905,
    );
}

fn spawn_title_bar(parent: &mut ChildSpawnerCommands, id: DevWindowId) {
    parent
        .spawn((
            DevWindowUi,
            Node {
                width: Val::Percent(100.0),

                height: Val::Px(TITLE_BAR_HEIGHT_PX),

                flex_direction: FlexDirection::Row,

                align_items: AlignItems::Center,

                padding: UiRect::horizontal(Val::Px(6.0)),

                column_gap: Val::Px(4.0),

                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.10, 0.13, 0.98)),
        ))
        .with_children(|bar| {
            bar.spawn((
                DevWindowTitleBarDragRegion { id },
                DevWindowUi,
                Button,
                Node {
                    flex_grow: 1.0,

                    height: Val::Percent(100.0),

                    align_items: AlignItems::Center,

                    padding: UiRect::left(Val::Px(4.0)),

                    ..default()
                },
                BackgroundColor(Color::NONE),
                Text::new(id.title()),
                TextFont {
                    font_size: 14.0,

                    ..default()
                },
                TextColor(WINDOW_TITLE_TEXT),
            ));

            if id.supports_collapse() {
                spawn_title_chrome_button(bar, id, true, "-");
            }

            spawn_title_chrome_button(bar, id, false, "x");
        });
}

fn spawn_title_chrome_button(
    parent: &mut ChildSpawnerCommands<'_>,

    id: DevWindowId,

    collapse: bool,

    label: &str,
) {
    let mut button = parent.spawn((
        DevWindowUi,
        DevButtonChrome::default(),
        Button,
        Node {
            width: Val::Px(24.0),

            height: Val::Px(22.0),

            justify_content: JustifyContent::Center,

            align_items: AlignItems::Center,

            ..default()
        },
        BackgroundColor(TITLE_BTN),
        BorderColor::all(crate::dev::widgets::theme::BTN_BORDER_IDLE),
    ));

    if collapse {
        button.insert(DevWindowCollapseButton { id });
    } else {
        button.insert(DevWindowCloseButton { id });
    }

    button.with_children(|child| {
        if collapse {
            child.spawn((
                DevWindowCollapseButtonLabel { id },
                DevWindowUi,
                Text::new(label),
                TextFont {
                    font_size: 14.0,

                    ..default()
                },
                TextColor(Color::srgba(0.85, 0.92, 0.98, 1.0)),
            ));
        } else {
            child.spawn((
                DevWindowUi,
                Text::new(label),
                TextFont {
                    font_size: 14.0,

                    ..default()
                },
                TextColor(Color::srgba(0.85, 0.92, 0.98, 1.0)),
            ));
        }
    });
}
