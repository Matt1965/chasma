//! Bottom-right command panel (P-UI1).

use bevy::prelude::*;

use crate::client::{
    available_commands_for_selection, ClientIntent, ClientIntentQueue, CommandType,
};
use crate::units::input::SelectedUnits;
use crate::world::UnitCatalog;

use super::layout::PlayerHudUi;
use super::player_hud_state::PlayerHudState;
use super::styles::{
    command_button_bg, hud_body_font, CMD_BTN_BORDER, PANEL_BG, TEXT_MUTED, TEXT_PRIMARY,
};

/// Marker for the command panel root.
#[derive(Component, Debug)]
pub struct CommandPanelRoot;

/// All command buttons rendered in the panel grid.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HudCommandButton {
    Move,
    Stop,
    HoldPosition,
    Attack,
    Harvest,
    Interact,
}

impl HudCommandButton {
    pub fn label(self) -> &'static str {
        match self {
            Self::Move => "Move",
            Self::Stop => "Stop",
            Self::HoldPosition => "Hold",
            Self::Attack => "Attack",
            Self::Harvest => "Harvest",
            Self::Interact => "Interact",
        }
    }

    pub fn palette_command(self) -> Option<CommandType> {
        match self {
            Self::Move => Some(CommandType::Move),
            Self::Stop => Some(CommandType::Stop),
            Self::HoldPosition => Some(CommandType::HoldPosition),
            Self::Attack | Self::Harvest | Self::Interact => None,
        }
    }

    pub fn is_future_placeholder(self) -> bool {
        matches!(self, Self::Attack | Self::Harvest | Self::Interact)
    }
}

/// Whether a HUD command button is interactable for the current selection.
pub fn command_button_enabled(
    button: HudCommandButton,
    selection: &SelectedUnits,
    catalog: &UnitCatalog,
) -> bool {
    if button.is_future_placeholder() {
        return false;
    }
    let Some(command_type) = button.palette_command() else {
        return false;
    };
    available_commands_for_selection(selection, catalog)
        .iter()
        .any(|entry| entry.command_type == command_type && entry.enabled)
}

/// Whether pressing the button should enqueue a gameplay palette intent.
pub fn command_button_emits_palette_intent(button: HudCommandButton) -> bool {
    matches!(button, HudCommandButton::Stop | HudCommandButton::HoldPosition)
}

pub const COMMAND_GRID: [HudCommandButton; 6] = [
    HudCommandButton::Move,
    HudCommandButton::Stop,
    HudCommandButton::HoldPosition,
    HudCommandButton::Attack,
    HudCommandButton::Harvest,
    HudCommandButton::Interact,
];

pub fn spawn_command_panel(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            CommandPanelRoot,
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: Val::Percent(32.0),
                padding: UiRect::all(Val::Px(super::styles::PANEL_PADDING_PX)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Commands"),
                hud_body_font(),
                TextColor(TEXT_MUTED),
            ));
            panel
                .spawn(Node {
                    display: Display::Grid,
                    grid_template_columns: RepeatedGridTrack::flex(3, 1.0),
                    row_gap: Val::Px(4.0),
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|grid| {
                    for button in COMMAND_GRID {
                        spawn_command_button(grid, button);
                    }
                });
        });
}

fn spawn_command_button(parent: &mut ChildSpawnerCommands, button: HudCommandButton) {
    parent
        .spawn((
            button,
            PlayerHudUi,
            Button,
            Node {
                min_height: Val::Px(36.0),
                padding: UiRect::all(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(super::styles::CMD_BTN_DISABLED_BG),
            BorderColor::all(CMD_BTN_BORDER),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(button.label()),
                hud_body_font(),
                TextColor(TEXT_PRIMARY),
            ));
        });
}

/// Sync enabled / armed visuals on command buttons.
pub fn sync_command_panel_buttons(
    selection: Res<SelectedUnits>,
    catalog: Res<UnitCatalog>,
    hud: Res<PlayerHudState>,
    mut buttons: Query<(
        &HudCommandButton,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    mut cache: Local<u32>,
) {
    let sig = selection.0.len() as u32;
    if *cache == sig && !selection.is_empty() {
        return;
    }
    *cache = sig;
    for (button, mut bg, mut border) in &mut buttons {
        let enabled = command_button_enabled(*button, &selection, &catalog);
        let armed = hud.armed_command == button.palette_command();
        *bg = if enabled {
            if armed {
                BackgroundColor(super::styles::CMD_BTN_ARMED_BG)
            } else {
                BackgroundColor(super::styles::CMD_BTN_ENABLED_BG)
            }
        } else {
            BackgroundColor(super::styles::CMD_BTN_DISABLED_BG)
        };
        border.set_all(if armed && enabled {
            super::styles::ACCENT_GREEN
        } else {
            CMD_BTN_BORDER
        });
    }
}

pub fn update_command_button_hover(
    selection: Res<SelectedUnits>,
    catalog: Res<UnitCatalog>,
    hud: Res<PlayerHudState>,
    mut query: Query<
        (
            &Interaction,
            &HudCommandButton,
            &mut BackgroundColor,
        ),
        Changed<Interaction>,
    >,
) {
    for (interaction, button, mut bg) in &mut query {
        let enabled = command_button_enabled(*button, &selection, &catalog);
        let armed = hud.armed_command == button.palette_command();
        *bg = command_button_bg(interaction, enabled, armed);
    }
}

pub fn handle_command_button_clicks(
    mut queue: ResMut<ClientIntentQueue>,
    mut hud: ResMut<PlayerHudState>,
    selection: Res<SelectedUnits>,
    catalog: Res<UnitCatalog>,
    interaction: Query<(&Interaction, &HudCommandButton), Changed<Interaction>>,
) {
    for (state, button) in &interaction {
        if *state != Interaction::Pressed {
            continue;
        }
        if !command_button_enabled(*button, &selection, &catalog) {
            continue;
        }
        hud.hovered_command = button.palette_command();
        match *button {
            HudCommandButton::Move => {
                hud.armed_command = Some(CommandType::Move);
            }
            HudCommandButton::Stop | HudCommandButton::HoldPosition => {
                if let Some(command_type) = button.palette_command() {
                    queue.push(ClientIntent::PaletteCommand { command_type });
                }
                hud.armed_command = None;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::CommandPaletteEntry;

    #[test]
    fn move_command_is_enabled_with_selection() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        assert!(command_button_enabled(
            HudCommandButton::Move,
            &selection,
            &UnitCatalog::default()
        ));
    }

    #[test]
    fn future_commands_are_disabled() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        assert!(!command_button_enabled(
            HudCommandButton::Attack,
            &selection,
            &UnitCatalog::default()
        ));
        assert!(!command_button_enabled(
            HudCommandButton::Harvest,
            &selection,
            &UnitCatalog::default()
        ));
        assert!(!command_button_enabled(
            HudCommandButton::Interact,
            &selection,
            &UnitCatalog::default()
        ));
    }

    #[test]
    fn disabled_buttons_do_not_emit_palette_intents() {
        assert!(!command_button_emits_palette_intent(HudCommandButton::Attack));
        assert!(!command_button_emits_palette_intent(HudCommandButton::Move));
        assert!(command_button_emits_palette_intent(HudCommandButton::Stop));
    }

    #[test]
    fn palette_exposes_move_for_selection() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let entries = available_commands_for_selection(&selection, &UnitCatalog::default());
        assert!(entries.contains(&CommandPaletteEntry {
            command_type: CommandType::Move,
            enabled: true,
        }));
    }
}
