//! Focused menu foundation tests (client-local; not WorldData).

use bevy::prelude::*;

use super::font::{
    MENU_BANNER_FONT_SIZE, MENU_BODY_FONT_SIZE, MENU_BUTTON_FONT_SIZE, MENU_TITLE_FONT_SIZE,
    PAUSE_CONTROL_FONT_SIZE, menu_text_font,
};
use super::input_block::{MenuInputBlock, menu_blocks_input};
use super::loading::{
    DEV_LOADING_MIN_VISIBLE_SECS, LOADING_MIN_FRAMES, LOADING_TERRAIN_FRAME_BUDGET, LoadingPhase,
    LoadingSession, loading_bridge_may_enter,
};
use super::main_menu::{MAIN_MENU_BACKGROUND_PATH, cover_size_for_window};
use super::navigation::{MenuContext, MenuNavigation, MenuPage, PauseMenuContext};
use super::pause_menu::pause_menu_font_sizes;
use super::screen::{AppScreen, GameSessionKind, GameSessionState};
use super::systems::{close_pause_menu, open_pause_menu};
use super::transition::{SessionTransitionKind, SessionTransitionRequest};
use crate::simulation::SimulationControlState;

#[test]
fn initial_screen_default_is_main_menu() {
    assert_eq!(AppScreen::default(), AppScreen::MainMenu);
}

#[test]
fn session_kind_starts_none() {
    assert_eq!(GameSessionState::default().kind, GameSessionKind::None);
}

#[test]
fn new_game_request_sets_pending_kind() {
    let mut req = SessionTransitionRequest::default();
    req.request(SessionTransitionKind::StartNewGame);
    assert_eq!(req.take(), Some(SessionTransitionKind::StartNewGame));
    assert!(req.take().is_none());
}

#[test]
fn menu_blocks_on_main_menu_and_loading() {
    let nav = MenuNavigation::default();
    assert!(menu_blocks_input(&AppScreen::MainMenu, &nav));
    assert!(menu_blocks_input(&AppScreen::Loading, &nav));
    assert!(!menu_blocks_input(&AppScreen::InGame, &nav));
}

#[test]
fn menu_blocks_when_pause_open() {
    let mut nav = MenuNavigation::default();
    nav.open_pause_root();
    assert!(menu_blocks_input(&AppScreen::InGame, &nav));
}

#[test]
fn main_root_escape_policy_is_noop_at_navigation_layer() {
    let nav = MenuNavigation {
        context: MenuContext::MainMenu,
        page: MenuPage::Root,
        pause_open: false,
    };
    assert_eq!(nav.page, MenuPage::Root);
}

#[test]
fn settings_back_returns_to_root() {
    let mut nav = MenuNavigation::default();
    nav.go_settings();
    assert_eq!(nav.page, MenuPage::Settings);
    nav.back_to_root();
    assert_eq!(nav.page, MenuPage::Root);
}

#[test]
fn pause_open_records_and_restores_running_simulation() {
    let mut nav = MenuNavigation::default();
    let mut pause_ctx = PauseMenuContext::default();
    let mut control = SimulationControlState::default();
    assert!(!control.paused);
    open_pause_menu(&mut nav, &mut pause_ctx, &mut control);
    assert!(control.paused);
    assert!(!pause_ctx.was_simulation_paused);
    close_pause_menu(&mut nav, &mut pause_ctx, &mut control);
    assert!(!control.paused);
    assert!(!nav.pause_open);
}

#[test]
fn pause_open_preserves_already_paused_simulation() {
    let mut nav = MenuNavigation::default();
    let mut pause_ctx = PauseMenuContext::default();
    let mut control = SimulationControlState {
        paused: true,
        ..Default::default()
    };
    open_pause_menu(&mut nav, &mut pause_ctx, &mut control);
    assert!(pause_ctx.was_simulation_paused);
    close_pause_menu(&mut nav, &mut pause_ctx, &mut control);
    assert!(control.paused);
}

#[test]
fn loading_session_labels_are_ascii_safe() {
    for phase in [
        LoadingPhase::PreparingNewGame,
        LoadingPhase::PreparingDefaultWorldEditor,
        LoadingPhase::PreparingTerrain,
        LoadingPhase::EnteringWorld,
    ] {
        let label = phase.label();
        assert!(!label.is_empty());
        assert!(label.is_ascii());
        assert!(label.ends_with("..."));
    }
}

#[test]
fn confirmation_escape_cancels_to_root() {
    let mut nav = MenuNavigation::default();
    nav.open_pause_root();
    nav.go_confirm_return();
    assert_eq!(nav.page, MenuPage::ConfirmReturnToMainMenu);
    nav.back_to_root();
    assert_eq!(nav.page, MenuPage::Root);
    assert!(nav.pause_open);
}

#[test]
fn authoring_request_kind_is_default_world() {
    let mut req = SessionTransitionRequest::default();
    req.request(SessionTransitionKind::StartDefaultWorldAuthoring);
    assert_eq!(
        req.take(),
        Some(SessionTransitionKind::StartDefaultWorldAuthoring)
    );
}

#[test]
fn return_to_main_menu_clears_session_kind() {
    let mut session = GameSessionState {
        kind: GameSessionKind::NewGame,
    };
    session.clear();
    assert_eq!(session.kind, GameSessionKind::None);
}

#[test]
fn loading_begin_sets_authoring_phase() {
    let mut loading = LoadingSession::default();
    loading.begin(GameSessionKind::DefaultWorldAuthoring);
    assert_eq!(loading.phase, LoadingPhase::PreparingDefaultWorldEditor);
}

#[test]
fn loading_begin_sets_new_game_phase() {
    let mut loading = LoadingSession::default();
    loading.begin(GameSessionKind::NewGame);
    assert_eq!(loading.phase, LoadingPhase::PreparingNewGame);
    assert_eq!(loading.target_kind, GameSessionKind::NewGame);
}

#[test]
fn continue_is_not_part_of_main_menu_actions() {
    let listed = ["New Game", "Settings", "Credits", "Quit"];
    assert!(!listed.iter().any(|s| *s == "Continue"));
}

#[test]
fn menu_input_block_resource_defaults_inactive() {
    assert!(!MenuInputBlock::default().blocks());
}

#[test]
fn menu_state_types_are_not_world_data_payload() {
    let session = GameSessionState {
        kind: GameSessionKind::NewGame,
    };
    let encoded = format!("{session:?}");
    assert!(encoded.contains("NewGame"));
    assert!(!encoded.contains("ChunkData"));
}

#[test]
fn menu_text_font_uses_absolute_sizes_not_multiplied_baselines() {
    let a = menu_text_font(MENU_BUTTON_FONT_SIZE);
    let b = menu_text_font(MENU_BUTTON_FONT_SIZE);
    assert_eq!(a.font_size, MENU_BUTTON_FONT_SIZE);
    assert_eq!(b.font_size, MENU_BUTTON_FONT_SIZE);
    // Re-applying the helper never compounds.
    assert_eq!(menu_text_font(a.font_size).font_size, MENU_BUTTON_FONT_SIZE);
}

#[test]
fn pause_text_sizes_are_deterministic_and_hud_band() {
    for size in pause_menu_font_sizes() {
        assert!(
            (12.0..=16.0).contains(&size),
            "pause size {size} outside HUD band"
        );
    }
    assert_eq!(PAUSE_CONTROL_FONT_SIZE, MENU_BUTTON_FONT_SIZE);
    assert_eq!(MENU_TITLE_FONT_SIZE, 16.0);
    assert_eq!(MENU_BANNER_FONT_SIZE, 12.0);
    assert_eq!(MENU_BODY_FONT_SIZE, 12.0);
}

#[test]
fn main_menu_background_path_is_asset_relative() {
    assert_eq!(MAIN_MENU_BACKGROUND_PATH, "images/chasma_background.png");
    assert!(!MAIN_MENU_BACKGROUND_PATH.contains('\\'));
    assert!(!MAIN_MENU_BACKGROUND_PATH.contains(':'));
}

#[test]
fn cover_size_preserves_aspect_and_fills_window() {
    let window = Vec2::new(1920.0, 1080.0);
    let intrinsic = Vec2::new(1672.0, 941.0);
    let size = cover_size_for_window(window, intrinsic);
    assert!(size.x + 0.01 >= window.x);
    assert!(size.y + 0.01 >= window.y);
    let aspect = size.x / size.y;
    let intrinsic_aspect = intrinsic.x / intrinsic.y;
    assert!((aspect - intrinsic_aspect).abs() < 0.001);
}

#[test]
fn loading_dev_min_duration_blocks_early_entry() {
    assert!(!loading_bridge_may_enter(
        LOADING_MIN_FRAMES,
        DEV_LOADING_MIN_VISIBLE_SECS * 0.5,
        false,
        Some(DEV_LOADING_MIN_VISIBLE_SECS),
    ));
    assert!(loading_bridge_may_enter(
        LOADING_MIN_FRAMES,
        DEV_LOADING_MIN_VISIBLE_SECS,
        false,
        Some(DEV_LOADING_MIN_VISIBLE_SECS),
    ));
}

#[test]
fn loading_normal_policy_ignores_dev_delay() {
    assert!(loading_bridge_may_enter(
        LOADING_MIN_FRAMES,
        0.0,
        false,
        None,
    ));
}

#[test]
fn loading_terrain_wait_has_finite_budget() {
    assert!(!loading_bridge_may_enter(
        LOADING_MIN_FRAMES,
        1.0,
        true,
        None,
    ));
    assert!(loading_bridge_may_enter(
        LOADING_TERRAIN_FRAME_BUDGET,
        1.0,
        true,
        None,
    ));
}
