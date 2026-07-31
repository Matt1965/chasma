//! Shared Main Menu / Pause Menu navigation stack (client-local).

use bevy::prelude::*;

use super::screen::GameSessionKind;

/// Which menu shell owns the shared page stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MenuContext {
    #[default]
    MainMenu,
    PauseMenu,
}

/// Page within the shared menu navigation resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MenuPage {
    #[default]
    Root,
    Settings,
    Credits,
    ConfirmReturnToMainMenu,
    ConfirmQuitToDesktop,
}

/// Shared navigation for Main Menu and Pause Menu.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct MenuNavigation {
    pub context: MenuContext,
    pub page: MenuPage,
    /// True while the pause overlay is open (AppScreen remains InGame).
    pub pause_open: bool,
}

impl Default for MenuNavigation {
    fn default() -> Self {
        Self {
            context: MenuContext::MainMenu,
            page: MenuPage::Root,
            pause_open: false,
        }
    }
}

impl MenuNavigation {
    pub fn open_main_root(&mut self) {
        self.context = MenuContext::MainMenu;
        self.page = MenuPage::Root;
        self.pause_open = false;
    }

    pub fn open_pause_root(&mut self) {
        self.context = MenuContext::PauseMenu;
        self.page = MenuPage::Root;
        self.pause_open = true;
    }

    pub fn close_pause(&mut self) {
        self.pause_open = false;
        self.page = MenuPage::Root;
        self.context = MenuContext::MainMenu;
    }

    pub fn go_settings(&mut self) {
        self.page = MenuPage::Settings;
    }

    pub fn go_credits(&mut self) {
        self.page = MenuPage::Credits;
    }

    pub fn go_confirm_return(&mut self) {
        self.page = MenuPage::ConfirmReturnToMainMenu;
    }

    pub fn go_confirm_quit(&mut self) {
        self.page = MenuPage::ConfirmQuitToDesktop;
    }

    pub fn back_to_root(&mut self) {
        self.page = MenuPage::Root;
    }

    pub fn is_modal_page(&self) -> bool {
        !matches!(self.page, MenuPage::Root) || self.pause_open
    }
}

/// Records simulation pause state when the Pause Menu opens.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PauseMenuContext {
    pub was_simulation_paused: bool,
    pub active: bool,
}

pub fn pause_menu_is_open(nav: &MenuNavigation) -> bool {
    nav.pause_open
}

/// Visible authoring banner copy for default-world sessions.
pub fn authoring_banner_label(kind: GameSessionKind) -> Option<&'static str> {
    match kind {
        GameSessionKind::DefaultWorldAuthoring => {
            Some("DEVELOPMENT - EDITING DEFAULT WORLD (persistence not connected)")
        }
        _ => None,
    }
}
