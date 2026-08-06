//! Menu plugin registration and system sets.

use bevy::prelude::*;

use super::input_block::{MenuInputBlock, sync_menu_input_block};
use super::loading::{LoadingSession, tick_loading_session};
use super::loading_ui::{despawn_loading_ui, spawn_loading_ui, sync_loading_status_text};
use super::main_menu::{
    despawn_main_menu, handle_main_menu_buttons, spawn_main_menu, sync_main_menu_background_cover,
    sync_main_menu_page, warn_main_menu_background_load_failure,
};
use super::navigation::{MenuNavigation, PauseMenuContext};
use super::pause_menu::{handle_pause_menu_buttons, sync_pause_menu_page};
use super::screen::{AppScreen, GameSessionState};
use super::settings::{SettingsMenuState, handle_settings_actions};
use super::systems::{
    enforce_simulation_pause_for_screen, ensure_unique_main_menu_root, handle_menu_escape,
    pause_simulation_on_enter_main_menu, prepare_main_menu_navigation,
    resume_simulation_on_enter_ingame, sync_gameplay_hud_for_screen, sync_pause_menu_presence,
};
use super::transition::{SessionTransitionRequest, apply_session_transition_requests};

/// Menu Escape / input-block sync — runs before gameplay and Dev collectors.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct MenuInputSystems;

/// Menu presentation sync (HUD visibility, pause presence, page hosts).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct MenuUiSystems;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppScreen>()
            .init_resource::<GameSessionState>()
            .init_resource::<MenuNavigation>()
            .init_resource::<MenuInputBlock>()
            .init_resource::<PauseMenuContext>()
            .init_resource::<SettingsMenuState>()
            .init_resource::<SessionTransitionRequest>()
            .init_resource::<LoadingSession>()
            .configure_sets(
                Update,
                (MenuInputSystems, MenuUiSystems.after(MenuInputSystems)),
            )
            .add_systems(
                OnEnter(AppScreen::MainMenu),
                (
                    pause_simulation_on_enter_main_menu,
                    prepare_main_menu_navigation,
                    spawn_main_menu,
                ),
            )
            .add_systems(OnExit(AppScreen::MainMenu), despawn_main_menu)
            .add_systems(OnEnter(AppScreen::Loading), spawn_loading_ui)
            .add_systems(OnExit(AppScreen::Loading), despawn_loading_ui)
            .add_systems(
                OnEnter(AppScreen::InGame),
                resume_simulation_on_enter_ingame,
            )
            .add_systems(
                Update,
                (
                    sync_menu_input_block,
                    handle_menu_escape,
                    apply_session_transition_requests,
                    tick_loading_session,
                    enforce_simulation_pause_for_screen,
                )
                    .chain()
                    .in_set(MenuInputSystems),
            )
            .add_systems(
                Update,
                (
                    sync_pause_menu_presence,
                    sync_gameplay_hud_for_screen,
                    handle_main_menu_buttons.run_if(in_state(AppScreen::MainMenu)),
                    handle_pause_menu_buttons.run_if(in_state(AppScreen::InGame)),
                    handle_settings_actions,
                    sync_main_menu_page.run_if(in_state(AppScreen::MainMenu)),
                    sync_main_menu_background_cover.run_if(in_state(AppScreen::MainMenu)),
                    warn_main_menu_background_load_failure.run_if(in_state(AppScreen::MainMenu)),
                    ensure_unique_main_menu_root.run_if(in_state(AppScreen::MainMenu)),
                    sync_pause_menu_page.run_if(in_state(AppScreen::InGame)),
                    sync_loading_status_text.run_if(in_state(AppScreen::Loading)),
                )
                    .in_set(MenuUiSystems),
            );
    }
}
