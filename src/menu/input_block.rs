//! Single shared authority: a modal application menu owns input.

use bevy::prelude::*;

use super::navigation::MenuNavigation;
use super::screen::AppScreen;

/// Client-local menu input ownership. Checked by gameplay, camera, sim, and Dev gates.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MenuInputBlock {
    pub active: bool,
}

impl MenuInputBlock {
    pub fn blocks(&self) -> bool {
        self.active
    }
}

/// True when Main Menu, Loading, Pause, or a nested menu page owns input.
pub fn menu_blocks_input(screen: &AppScreen, nav: &MenuNavigation) -> bool {
    match screen {
        AppScreen::MainMenu | AppScreen::Loading => true,
        AppScreen::InGame => {
            nav.pause_open || !matches!(nav.page, super::navigation::MenuPage::Root)
        }
    }
}

/// Sync [`MenuInputBlock`] from screen + navigation (runs early each frame).
pub fn sync_menu_input_block(
    screen: Res<State<AppScreen>>,
    nav: Res<MenuNavigation>,
    mut block: ResMut<MenuInputBlock>,
) {
    block.active = menu_blocks_input(screen.get(), &nav);
}
