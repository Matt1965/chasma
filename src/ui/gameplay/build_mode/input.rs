//! Build mode keyboard/mouse intent collection (ADR-081 B4).

use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::client::{ClientIntent, ClientIntentQueue};
use crate::terrain::TerrainRenderAssets;
use crate::units::input::{cursor_world_ray, terrain_click_to_world_position};
use crate::world::{BuildingDefinitionId, WorldConfig, WorldData, rotation_from_quadrants};

use super::state::{BuildModePhase, BuildModeState};
use crate::ui::gameplay::{PlayerHudHoverState, gameplay_input_blocked_by_hud};

/// Collect build-mode keyboard and placement clicks into client intents.
pub fn collect_build_mode_intents(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut build_mode: ResMut<BuildModeState>,
    mut queue: ResMut<ClientIntentQueue>,
    hud_hover: Res<PlayerHudHoverState>,
    #[cfg(feature = "dev")] dev_state: Option<Res<crate::dev::DevModeState>>,
) {
    if build_shortcuts_blocked(&build_mode) {
        return;
    }
    #[cfg(feature = "dev")]
    if dev_state.is_some_and(|state| state.has_text_focus()) {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyB) {
        if build_mode.is_active() {
            queue.push(ClientIntent::ExitBuildMode);
        } else {
            queue.push(ClientIntent::EnterBuildMode);
        }
    }

    if !build_mode.is_active() {
        return;
    }

    if keyboard.just_pressed(KeyCode::Escape) {
        queue.push(ClientIntent::CancelBuildPlacement);
        return;
    }

    if build_mode.is_ghost_placing() && keyboard.just_pressed(KeyCode::KeyR) {
        queue.push(ClientIntent::RotateBuildGhost);
    }

    if gameplay_input_blocked_by_hud(&hud_hover) {
        return;
    }

    if mouse_buttons.just_pressed(MouseButton::Right) {
        queue.push(ClientIntent::CancelBuildPlacement);
        return;
    }

    if !build_mode.is_ghost_placing() {
        return;
    }

    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(ray) = cursor_world_ray(&windows, &camera) else {
        return;
    };
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let Some(click) = terrain_click_to_world_position(&ray, &world, layout, vertical_scale) else {
        return;
    };

    let BuildModePhase::GhostPlacing {
        definition_id,
        rotation_quadrants,
    } = build_mode.phase.clone()
    else {
        return;
    };

    queue.push(ClientIntent::PlaceBuilding {
        definition_id,
        anchor: click.world_position,
        rotation: rotation_from_quadrants(rotation_quadrants),
    });
}

fn build_shortcuts_blocked(build_mode: &BuildModeState) -> bool {
    build_mode.search_focused
}
