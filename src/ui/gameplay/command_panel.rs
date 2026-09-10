//! Bottom-right command panel (P-UI1, REVIEW-B3).

use bevy::prelude::*;

use crate::client::{
    ClientIntent, ClientIntentQueue, CommandType, available_commands_for_selection,
    command_availability,
};
use crate::units::input::SelectedUnits;
use crate::world::UnitCatalog;

use super::hud::{
    HudViewportGeometry, hud_button_depth_style, hud_button_shell_style, hud_section_node,
    spawn_hud_button_top_highlight,
};
use super::layout::PlayerHudUi;
use super::player_hud_state::PlayerHudState;
use super::styles::{HUD_COMMAND_BUTTON_HEIGHT_PERCENT, TEXT_PRIMARY, hud_title_font};

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
            Self::AttackMove => "Atk Mv",
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

    /// Stop and Hold issue immediately; Move/Attack/AttackMove arm for right-click.
    pub fn emits_palette_intent(self) -> bool {
        matches!(
            self,
            HudCommandButton::Stop | HudCommandButton::HoldPosition
        )
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

/// Visible command buttons in the bottom HUD command panel.
pub const COMMAND_GRID: [HudCommandButton; 5] = [
    HudCommandButton::Move,
    HudCommandButton::Attack,
    HudCommandButton::AttackMove,
    HudCommandButton::HoldPosition,
    HudCommandButton::Stop,
];

pub fn spawn_command_panel(parent: &mut ChildSpawnerCommands<'_>) {
    let geom = HudViewportGeometry::default();
    parent
        .spawn((
            CommandPanelRoot,
            PlayerHudUi,
            hud_section_node(geom.command_width),
        ))
        .with_children(|panel| {
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceEvenly,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                })
                .with_children(|row| {
                    for button in COMMAND_GRID {
                        spawn_command_button(row, button);
                    }
                });
        });
}

fn spawn_command_button(parent: &mut ChildSpawnerCommands<'_>, button: HudCommandButton) {
    let geom = HudViewportGeometry::default();
    let radius = BorderRadius::all(Val::Px(6.0));
    parent
        .spawn((
            button,
            PlayerHudUi,
            Button,
            Node {
                flex_grow: 0.0,
                flex_basis: Val::Px(geom.command_button_width),
                width: Val::Px(geom.command_button_width),
                height: Val::Percent(HUD_COMMAND_BUTTON_HEIGHT_PERCENT),
                align_self: AlignSelf::Center,
                padding: UiRect::all(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: radius,
                overflow: Overflow::clip(),
                ..default()
            },
            hud_button_depth_style(),
        ))
        .with_children(|btn| {
            spawn_hud_button_top_highlight(btn);
            btn.spawn((
                Text::new(button.label()),
                hud_title_font(),
                TextColor(TEXT_PRIMARY),
            ));
        });
}

fn apply_command_button_shell(
    button: HudCommandButton,
    selection: &SelectedUnits,
    catalog: &UnitCatalog,
    hud: &PlayerHudState,
    interaction: &Interaction,
    bg: &mut BackgroundColor,
    border: &mut BorderColor,
) {
    let enabled = command_button_enabled(button, selection, catalog);
    let armed = hud.armed_command == Some(button.command_type());
    let (next_bg, next_border) = hud_button_shell_style(interaction, enabled, armed);
    *bg = next_bg;
    *border = next_border;
}

/// Sync enabled / armed visuals on command buttons.
pub fn sync_command_panel_buttons(
    selection: Res<SelectedUnits>,
    catalog: Res<UnitCatalog>,
    hud: Res<PlayerHudState>,
    mut buttons: Query<(
        &HudCommandButton,
        &Interaction,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    if !selection.is_changed() && !hud.is_changed() {
        return;
    }
    for (button, interaction, mut bg, mut border) in &mut buttons {
        apply_command_button_shell(
            *button,
            &selection,
            &catalog,
            &hud,
            interaction,
            &mut bg,
            &mut border,
        );
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
            &mut BorderColor,
        ),
        Changed<Interaction>,
    >,
) {
    for (interaction, button, mut bg, mut border) in &mut query {
        apply_command_button_shell(
            *button,
            &selection,
            &catalog,
            &hud,
            interaction,
            &mut bg,
            &mut border,
        );
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
            HudCommandButton::Stop | HudCommandButton::HoldPosition
                if button.emits_palette_intent() =>
            {
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
    use crate::ui::gameplay::hud::hud_button_shell_style;

    fn command_button_tooltip(button: HudCommandButton, selection: &SelectedUnits) -> String {
        let command_type = button.command_type();
        command_tooltip(command_type, command_availability(command_type, selection))
    }

    #[test]
    fn command_shell_uses_hud_palette_not_legacy_cyan() {
        let (_, border) = hud_button_shell_style(&Interaction::None, true, false);
        let edge = border.left.to_srgba();
        assert!(
            edge.green > edge.blue,
            "command buttons must not restore the cyan HUD border palette"
        );
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
    fn hold_position_enabled_with_selection() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        assert!(command_button_enabled(
            HudCommandButton::HoldPosition,
            &selection,
            &UnitCatalog::default()
        ));
    }

    #[test]
    fn interact_not_in_visible_command_grid() {
        assert!(!COMMAND_GRID.contains(&HudCommandButton::Interact));
    }

    #[test]
    fn stop_and_hold_emit_palette_intent() {
        assert!(HudCommandButton::Stop.emits_palette_intent());
        assert!(HudCommandButton::HoldPosition.emits_palette_intent());
        assert!(!HudCommandButton::Move.emits_palette_intent());
    }

    #[test]
    fn visible_grid_includes_hold_and_attack_move() {
        assert!(COMMAND_GRID.contains(&HudCommandButton::HoldPosition));
        assert!(COMMAND_GRID.contains(&HudCommandButton::AttackMove));
        assert!(!COMMAND_GRID.contains(&HudCommandButton::Interact));
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
