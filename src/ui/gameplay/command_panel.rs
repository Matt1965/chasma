//! Bottom-right command panel (P-UI1, REVIEW-B3).

use bevy::prelude::*;

use crate::client::{
    ClientIntent, ClientIntentQueue, CommandType, available_commands_for_selection,
    command_availability,
};
use crate::units::input::SelectedUnits;
use crate::world::UnitCatalog;

use super::layout::PlayerHudUi;
use super::player_hud_state::PlayerHudState;
use super::styles::{
    CMD_BTN_BORDER, PANEL_BG, TEXT_MUTED, TEXT_PRIMARY, command_button_bg, hud_body_font,
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
    AttackMove,
    Interact,
}

impl HudCommandButton {
    pub fn label(self) -> &'static str {
        match self {
            Self::Move => "Move",
            Self::Stop => "Stop",
            Self::HoldPosition => "Hold",
            Self::Attack => "Attack",
            Self::AttackMove => "Attack Move",
            Self::Interact => "Interact",
        }
    }

    pub fn command_type(self) -> CommandType {
        match self {
            Self::Move => CommandType::Move,
            Self::Stop => CommandType::Stop,
            Self::HoldPosition => CommandType::HoldPosition,
            Self::Attack => CommandType::Attack,
            Self::AttackMove => CommandType::AttackMove,
            Self::Interact => CommandType::Interact,
        }
    }

    /// Only Stop uses immediate palette dispatch; other commands arm for right-click.
    pub fn emits_palette_intent(self) -> bool {
        matches!(self, HudCommandButton::Stop)
    }
}

/// Whether a HUD command button is interactable for the current selection.
pub fn command_button_enabled(
    button: HudCommandButton,
    selection: &SelectedUnits,
    catalog: &UnitCatalog,
) -> bool {
    let command_type = button.command_type();
    if let Some(entry) = available_commands_for_selection(selection, catalog)
        .into_iter()
        .find(|entry| entry.command_type == command_type)
    {
        entry.is_enabled()
    } else if selection.is_empty() {
        false
    } else {
        command_availability(command_type, selection).is_available()
    }
}

pub const COMMAND_GRID: [HudCommandButton; 6] = [
    HudCommandButton::Move,
    HudCommandButton::Stop,
    HudCommandButton::HoldPosition,
    HudCommandButton::Attack,
    HudCommandButton::AttackMove,
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
    mut buttons: Query<(&HudCommandButton, &mut BackgroundColor, &mut BorderColor)>,
) {
    if !selection.is_changed() && !hud.is_changed() {
        return;
    }
    for (button, mut bg, mut border) in &mut buttons {
        let enabled = command_button_enabled(*button, &selection, &catalog);
        let armed = hud.armed_command == Some(button.command_type());
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
    mut query: Query<(&Interaction, &HudCommandButton, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, button, mut bg) in &mut query {
        let enabled = command_button_enabled(*button, &selection, &catalog);
        let armed = hud.armed_command == Some(button.command_type());
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
        let command_type = button.command_type();
        hud.hovered_command = Some(command_type);
        match *button {
            HudCommandButton::Move => hud.armed_command = Some(CommandType::Move),
            HudCommandButton::Attack => hud.armed_command = Some(CommandType::Attack),
            HudCommandButton::AttackMove => hud.armed_command = Some(CommandType::AttackMove),
            HudCommandButton::Stop if button.emits_palette_intent() => {
                queue.push(ClientIntent::PaletteCommand { command_type });
                hud.armed_command = None;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{
        CommandAvailability, CommandPaletteEntry, CommandUnavailableReason, command_tooltip,
    };

    fn command_button_tooltip(button: HudCommandButton, selection: &SelectedUnits) -> String {
        let command_type = button.command_type();
        command_tooltip(command_type, command_availability(command_type, selection))
    }

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
    fn attack_command_is_enabled_with_selection() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        assert!(command_button_enabled(
            HudCommandButton::Attack,
            &selection,
            &UnitCatalog::default()
        ));
    }

    #[test]
    fn attack_disabled_without_selection() {
        let selection = SelectedUnits::default();
        assert!(!command_button_enabled(
            HudCommandButton::Attack,
            &selection,
            &UnitCatalog::default()
        ));
    }

    #[test]
    fn attack_move_enabled_with_selection() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        assert!(command_button_enabled(
            HudCommandButton::AttackMove,
            &selection,
            &UnitCatalog::default()
        ));
    }

    #[test]
    fn hold_position_disabled_with_explicit_reason() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        assert!(!command_button_enabled(
            HudCommandButton::HoldPosition,
            &selection,
            &UnitCatalog::default()
        ));
        let tooltip = command_button_tooltip(HudCommandButton::HoldPosition, &selection);
        assert!(tooltip.contains("Not implemented"));
    }

    #[test]
    fn interact_disabled_with_explicit_reason() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        assert!(!command_button_enabled(
            HudCommandButton::Interact,
            &selection,
            &UnitCatalog::default()
        ));
        let tooltip = command_button_tooltip(HudCommandButton::Interact, &selection);
        assert!(tooltip.contains("Not implemented"));
    }

    #[test]
    fn only_stop_emits_palette_intent() {
        assert!(HudCommandButton::Stop.emits_palette_intent());
        assert!(!HudCommandButton::HoldPosition.emits_palette_intent());
        assert!(!HudCommandButton::Move.emits_palette_intent());
    }

    #[test]
    fn palette_exposes_move_for_selection() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let entries = available_commands_for_selection(&selection, &UnitCatalog::default());
        assert!(entries.contains(&CommandPaletteEntry {
            command_type: CommandType::Move,
            availability: CommandAvailability::Available,
        }));
    }
}
