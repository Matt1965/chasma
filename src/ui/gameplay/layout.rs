//! Persistent bottom HUD layout (P-UI1).
//!
//! The root owns all vertical geometry. Content lives in a single row inset by
//! the frame bevels, so section tops and bottoms cannot drift apart.
//!
//! Visual chrome is layered behind that row and owns no layout:
//!
//! - `ZIndex(-6..=-3)` one octagonal plate per section — rails, chamfers,
//!   interior, and transitions. Shorter plates draw last so a join is one step.
//! - `ZIndex(-1)` endcap ornaments and spire
//!
//! Only the plate frames draw rails, so each region has one visible contour.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use super::command_panel::spawn_command_panel;
use super::hud::{HudUiAssets, HudViewportGeometry, spawn_hud_ornaments, spawn_hud_plate_frames};
use super::selected_unit_panel::spawn_selected_unit_panel;
use super::squad_panel::spawn_squad_panel;
use super::styles::HUD_SECTION_GAP_PX;
use super::utility_panel::spawn_utility_panel;

/// Root gameplay HUD (full-width bottom bar).
#[derive(Component, Debug)]
pub struct GameplayHudRoot;

/// Marker on all player HUD widgets for pointer-capture detection.
#[derive(Component, Debug)]
pub struct PlayerHudUi;

/// Single row holding every HUD section; owns the shared content bounds.
#[derive(Component, Debug)]
pub struct BottomBar;

/// Spawn the gameplay HUD tree once at startup.
pub fn setup_player_hud_layout(mut commands: Commands, asset_server: Res<AssetServer>) {
    let assets = HudUiAssets::load(&asset_server);
    commands.insert_resource(assets.clone());
    let geom = HudViewportGeometry::default();

    commands
        .spawn((
            GameplayHudRoot,
            PlayerHudUi,
            Button,
            Interaction::None,
            FocusPolicy::Block,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(geom.hud_height),
                ..default()
            },
            ZIndex(300),
        ))
        .with_children(|root| {
            spawn_hud_plate_frames(root, &assets);
            spawn_hud_ornaments(root, &assets);
            root.spawn((
                BottomBar,
                PlayerHudUi,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(geom.endcap_left_width),
                    right: Val::Px(geom.endcap_right_width),
                    top: Val::Px(geom.frame_top),
                    bottom: Val::Px(geom.frame_bottom),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Stretch,
                    column_gap: Val::Px(HUD_SECTION_GAP_PX),
                    ..default()
                },
            ))
            .with_children(|bar| {
                spawn_selected_unit_panel(bar);
                spawn_squad_panel(bar);
                spawn_command_panel(bar);
                spawn_utility_panel(bar);
            });
        });
}

/// Whether a screen-space point lies inside the bottom HUD strip.
pub fn bottom_hud_rect_contains(cursor: Vec2, viewport: Vec2, hud_height: f32) -> bool {
    cursor.y >= viewport.y - hud_height && cursor.y <= viewport.y
}
