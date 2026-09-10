//! Player HUD pointer capture — blocks world selection when cursor is over HUD (P-UI1).

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::ui::ScrollPosition;
use bevy::window::PrimaryWindow;

use super::build_mode::BuildModeState;
use super::building_panel::BuildingPanelState;
use super::floating_window::{
    FloatingGameplayWindowRegistry, open_gameplay_floating_windows, ui_rect_contains,
};
use super::hud::HudViewportGeometry;
use super::inventory::InventoryUiState;
use super::layout::{PlayerHudUi, bottom_hud_rect_contains};
use super::settlement_workforce::SettlementWorkforcePanelState;
use super::squad_panel::SquadRosterViewport;
use super::unit_skills::UnitSkillsPanelState;

const BUILD_CATALOG_LEFT_PX: f32 = 8.0;
const BUILD_CATALOG_BOTTOM_OFFSET_PX: f32 = 8.0;
const BUILD_CATALOG_WIDTH_PX: f32 = 280.0;
const BUILD_CATALOG_MAX_HEIGHT_PX: f32 = 420.0;

/// Whether the player HUD is under the cursor (blocks gameplay mouse intents).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerHudHoverState {
    pub hovered: bool,
    /// Set by dev mode when the F12 panel captures input (ADR-047).
    pub dev_panel_blocks: bool,
    /// When true, camera zoom wheel should be suppressed (e.g. workforce row viewport).
    pub blocks_camera_scroll: bool,
}

/// Track HUD hover from UI interaction states and open floating-window geometry.
pub fn update_player_hud_hover_state(
    geometry: Res<HudViewportGeometry>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut interactions: ParamSet<(
        Query<&Interaction, With<PlayerHudUi>>,
        Query<&Interaction, With<SquadRosterViewport>>,
    )>,
    mut roster_scroll: Query<&mut ScrollPosition, With<SquadRosterViewport>>,
    registry: Res<FloatingGameplayWindowRegistry>,
    building_panel: Res<BuildingPanelState>,
    inventory_ui: Res<InventoryUiState>,
    unit_skills: Res<UnitSkillsPanelState>,
    workforce_panel: Res<SettlementWorkforcePanelState>,
    build_mode: Res<BuildModeState>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut hover: ResMut<PlayerHudHoverState>,
) {
    let interaction_hover = interactions
        .p0()
        .iter()
        .any(|state| *state != Interaction::None)
        || registry.is_dragging();

    let cursor = windows
        .single()
        .ok()
        .and_then(|window| window.cursor_position());

    let open_windows = open_gameplay_floating_windows(
        building_panel.is_open(),
        inventory_ui.open,
        unit_skills.open,
        workforce_panel.open,
    );
    let floating_rect_hover =
        cursor.is_some_and(|point| registry.pointer_inside_any_open(point, &open_windows));

    let build_catalog_hover = cursor.is_some_and(|point| {
        build_mode.is_active()
            && build_catalog_rect_contains(point, registry.viewport, geometry.hud_height)
    });

    let bottom_hud_hover = cursor.is_some_and(|point| {
        bottom_hud_rect_contains(point, registry.viewport, geometry.hud_height)
    });

    hover.hovered =
        interaction_hover || floating_rect_hover || build_catalog_hover || bottom_hud_hover;
    hover.blocks_camera_scroll = floating_rect_hover || build_catalog_hover || bottom_hud_hover;

    let wheel_delta = mouse_scroll.delta.y;
    if wheel_delta.abs() <= f32::EPSILON {
        return;
    }
    let roster_hovered = interactions
        .p1()
        .iter()
        .any(|interaction| *interaction != Interaction::None);
    if !roster_hovered {
        return;
    }
    hover.blocks_camera_scroll = true;
    let scroll_amount = wheel_delta * 24.0;
    for mut scroll_position in &mut roster_scroll {
        scroll_position.x = (scroll_position.x - scroll_amount).max(0.0);
    }
}

fn build_catalog_rect_contains(cursor: Vec2, viewport: Vec2, hud_height: f32) -> bool {
    let origin = Vec2::new(
        BUILD_CATALOG_LEFT_PX,
        viewport.y - hud_height - BUILD_CATALOG_BOTTOM_OFFSET_PX - BUILD_CATALOG_MAX_HEIGHT_PX,
    );
    let size = Vec2::new(BUILD_CATALOG_WIDTH_PX, BUILD_CATALOG_MAX_HEIGHT_PX);
    ui_rect_contains(cursor, origin, size)
}

/// Whether gameplay mouse intents should be suppressed this frame.
pub fn gameplay_input_blocked_by_hud(hover: &PlayerHudHoverState) -> bool {
    hover.hovered || hover.dev_panel_blocks
}

#[cfg(test)]
mod tests {
    use super::super::floating_window::FloatingGameplayWindowId;
    use super::*;

    #[test]
    fn hover_state_defaults_to_not_blocking() {
        let hover = PlayerHudHoverState::default();
        assert!(!gameplay_input_blocked_by_hud(&hover));
    }

    #[test]
    fn window_drag_blocks_world_input_via_hover_gate() {
        let registry = FloatingGameplayWindowRegistry::default();
        let mut registry = registry;
        registry.begin_drag(FloatingGameplayWindowId::BuildingMenu, Vec2::ZERO);
        let hover = PlayerHudHoverState {
            hovered: registry.is_dragging(),
            dev_panel_blocks: false,
            blocks_camera_scroll: false,
        };
        assert!(gameplay_input_blocked_by_hud(&hover));
    }

    #[test]
    fn floating_rect_hover_blocks_world_without_button_interaction() {
        let mut registry = FloatingGameplayWindowRegistry::default();
        registry
            .session_mut(FloatingGameplayWindowId::SettlementWorkforce)
            .unwrap()
            .position = Vec2::new(100.0, 80.0);
        registry
            .session_mut(FloatingGameplayWindowId::SettlementWorkforce)
            .unwrap()
            .computed_size = Vec2::new(860.0, 500.0);
        let open = [FloatingGameplayWindowId::SettlementWorkforce];
        assert!(registry.pointer_inside_any_open(Vec2::new(200.0, 200.0), &open));
        let hover = PlayerHudHoverState {
            hovered: true,
            dev_panel_blocks: false,
            blocks_camera_scroll: true,
        };
        assert!(gameplay_input_blocked_by_hud(&hover));
    }

    #[test]
    fn click_outside_window_does_not_block_world() {
        let registry = FloatingGameplayWindowRegistry::default();
        let open = [FloatingGameplayWindowId::UnitSkills];
        assert!(!registry.pointer_inside_any_open(Vec2::new(5.0, 5.0), &open));
    }

    #[test]
    fn build_catalog_rect_blocks_interior_padding() {
        let viewport = Vec2::new(1280.0, 720.0);
        let hud_height = HudViewportGeometry::default().hud_height;
        let origin_y =
            viewport.y - hud_height - BUILD_CATALOG_BOTTOM_OFFSET_PX - BUILD_CATALOG_MAX_HEIGHT_PX;
        assert!(build_catalog_rect_contains(
            Vec2::new(20.0, origin_y + 10.0),
            viewport,
            hud_height,
        ));
        assert!(!build_catalog_rect_contains(
            Vec2::new(400.0, 400.0),
            viewport,
            hud_height,
        ));
    }

    #[test]
    fn bottom_hud_rect_blocks_interior_padding() {
        let viewport = Vec2::new(1280.0, 720.0);
        let hud_height = HudViewportGeometry::default().hud_height;
        assert!(bottom_hud_rect_contains(
            Vec2::new(640.0, 715.0),
            viewport,
            hud_height
        ));
        assert!(!bottom_hud_rect_contains(
            Vec2::new(640.0, 500.0),
            viewport,
            hud_height
        ));
    }
}
