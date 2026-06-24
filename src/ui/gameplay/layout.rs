//! Persistent bottom HUD layout (P-UI1).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use super::command_panel::spawn_command_panel;
use super::selected_unit_panel::spawn_selected_unit_panel;
use super::squad_panel::spawn_squad_panel;
use super::styles::{BAR_BG, BOTTOM_BAR_HEIGHT_PX, SECTION_GAP_PX};

/// Root gameplay HUD (full-width bottom bar).
#[derive(Component, Debug)]
pub struct GameplayHudRoot;

/// Marker on all player HUD widgets for pointer-capture detection.
#[derive(Component, Debug)]
pub struct PlayerHudUi;

/// Bottom bar container.
#[derive(Component, Debug)]
pub struct BottomBar;

/// Spawn the gameplay HUD tree once at startup.
pub fn setup_player_hud_layout(mut commands: Commands) {
    commands
        .spawn((
            GameplayHudRoot,
            PlayerHudUi,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(BOTTOM_BAR_HEIGHT_PX),
                ..default()
            },
            BackgroundColor(BAR_BG),
            FocusPolicy::Block,
            ZIndex(300),
        ))
        .with_children(|root| {
            root.spawn((
                BottomBar,
                PlayerHudUi,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(SECTION_GAP_PX),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
            ))
            .with_children(|bar| {
                spawn_selected_unit_panel(bar);
                spawn_squad_panel(bar);
                spawn_command_panel(bar);
            });
        });
}
