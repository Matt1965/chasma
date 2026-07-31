//! Menu Escape handling, pause open/close, and gameplay HUD visibility.

use bevy::prelude::*;

use super::input_block::MenuInputBlock;
use super::navigation::{MenuNavigation, MenuPage, PauseMenuContext};
use super::pause_menu::{PauseMenuRoot, spawn_pause_menu};
use super::screen::AppScreen;
use crate::simulation::SimulationControlState;
use crate::ui::gameplay::{
    BuildModeState, GameplayHudRoot, InventoryUiState, PlayerHudState, PlayerHudUi,
};

pub fn open_pause_menu(
    nav: &mut MenuNavigation,
    pause_ctx: &mut PauseMenuContext,
    control: &mut SimulationControlState,
) {
    pause_ctx.was_simulation_paused = control.paused;
    pause_ctx.active = true;
    control.pause();
    nav.open_pause_root();
}

pub fn close_pause_menu(
    nav: &mut MenuNavigation,
    pause_ctx: &mut PauseMenuContext,
    control: &mut SimulationControlState,
) {
    if pause_ctx.active && !pause_ctx.was_simulation_paused {
        control.resume();
    }
    pause_ctx.active = false;
    pause_ctx.was_simulation_paused = false;
    nav.close_pause();
}

/// Escape: menu Back / open Pause / Resume. Never world deselection or tool cancel.
pub fn handle_menu_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    screen: Res<State<AppScreen>>,
    mut nav: ResMut<MenuNavigation>,
    mut pause_ctx: ResMut<PauseMenuContext>,
    mut control: ResMut<SimulationControlState>,
    mut inventory: ResMut<InventoryUiState>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }

    match *screen.get() {
        AppScreen::Loading => {}
        AppScreen::MainMenu => match nav.page {
            MenuPage::Root => {}
            MenuPage::Settings | MenuPage::Credits => nav.back_to_root(),
            _ => nav.back_to_root(),
        },
        AppScreen::InGame => {
            if nav.pause_open {
                match nav.page {
                    MenuPage::Root => {
                        close_pause_menu(&mut nav, &mut pause_ctx, &mut control);
                    }
                    MenuPage::Settings
                    | MenuPage::ConfirmReturnToMainMenu
                    | MenuPage::ConfirmQuitToDesktop
                    | MenuPage::Credits => nav.back_to_root(),
                }
            } else {
                inventory.cancel_drag(None);
                open_pause_menu(&mut nav, &mut pause_ctx, &mut control);
            }
        }
    }
}

/// Ensure pause overlay entity exists while pause_open.
pub fn sync_pause_menu_presence(
    nav: Res<MenuNavigation>,
    screen: Res<State<AppScreen>>,
    roots: Query<Entity, With<PauseMenuRoot>>,
    mut commands: Commands,
) {
    let want = *screen.get() == AppScreen::InGame && nav.pause_open;
    let has = !roots.is_empty();
    if want && !has {
        spawn_pause_menu(&mut commands);
    } else if !want && has {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
    }
}

/// Spawn-once HUD roots: hide outside InGame; show in InGame (unless pause wants dim — HUD stays but blocked).
pub fn sync_gameplay_hud_for_screen(
    screen: Res<State<AppScreen>>,
    menu_block: Res<MenuInputBlock>,
    mut hud_state: ResMut<PlayerHudState>,
    mut roots: Query<&mut Visibility, Or<(With<GameplayHudRoot>, With<PlayerHudUi>)>>,
    mut build_mode: ResMut<BuildModeState>,
    mut inventory: ResMut<InventoryUiState>,
) {
    let in_game = *screen.get() == AppScreen::InGame;
    hud_state.visible = in_game && !menu_block.active;

    let visibility = if in_game && !menu_block.active {
        Visibility::Visible
    } else if in_game && menu_block.active {
        // Pause: hide HUD interaction by hiding roots (menu owns input).
        Visibility::Hidden
    } else {
        Visibility::Hidden
    };

    for mut vis in &mut roots {
        *vis = visibility;
    }

    if !in_game || menu_block.active {
        if build_mode.is_active() {
            build_mode.exit();
        }
        if inventory.open {
            inventory.open = false;
        }
        inventory.cancel_drag(None);
    }
}

pub fn pause_simulation_on_enter_main_menu(mut control: ResMut<SimulationControlState>) {
    control.pause();
}

pub fn resume_simulation_on_enter_ingame(mut control: ResMut<SimulationControlState>) {
    // Temporary bridge: entering gameplay resumes unless Pause Menu immediately re-pauses.
    control.resume();
}

/// Keep simulation paused outside active InGame play (MainMenu / Loading / Pause overlay).
/// Does not resume InGame — Space / Pause restore own that.
pub fn enforce_simulation_pause_for_screen(
    screen: Res<State<AppScreen>>,
    nav: Res<MenuNavigation>,
    mut control: ResMut<SimulationControlState>,
) {
    match *screen.get() {
        AppScreen::MainMenu | AppScreen::Loading => {
            control.pause();
        }
        AppScreen::InGame if nav.pause_open => {
            control.pause();
        }
        AppScreen::InGame => {}
    }
}

pub fn prepare_main_menu_navigation(mut nav: ResMut<MenuNavigation>) {
    nav.open_main_root();
}

pub fn ensure_unique_main_menu_root(
    roots: Query<Entity, With<super::main_menu::MainMenuRoot>>,
    mut commands: Commands,
) {
    let mut iter = roots.iter();
    let _first = iter.next();
    for extra in iter {
        commands.entity(extra).despawn();
    }
}
