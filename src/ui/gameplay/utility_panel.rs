//! Compact bottom-right utility controls (unit + settlement groups).

use bevy::prelude::*;

use crate::client::inventory_intent::{InventoryIntent, InventoryIntentQueue, InventoryOpenMode};
use crate::ui::gameplay::build_mode::BuildModeState;
use crate::ui::gameplay::fields_menu::{FieldsMenuState, FieldsUtilityButton, spawn_fields_menu};
use crate::ui::gameplay::settlement_workforce::SettlementWorkforcePanelState;
use crate::ui::gameplay::unit_skills::UnitSkillsPanelState;
use crate::units::input::SelectedUnits;

use super::hud::{
    HudViewportGeometry, hud_button_depth_style, hud_button_shell_style,
    hud_section_node_with_horizontal_padding, spawn_hud_button_top_highlight,
    spawn_hud_utility_group_divider,
};
use super::layout::PlayerHudUi;
use super::player_hud_state::primary_selected_unit;
use super::styles::HUD_SECTION_PADDING_X_UTILITY_RIGHT_PX;
use super::styles::{HUD_UTILITY_BUTTON_HEIGHT_PX, TEXT_PRIMARY, hud_body_font, hud_caption_font};

/// Marker for the utility column root.
#[derive(Component, Debug)]
pub struct UtilityPanelRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HudUtilityButton {
    Inventory,
    UnitSkills,
    SettlementWorkforce,
    Fields,
}

impl HudUtilityButton {
    fn label(self) -> &'static str {
        match self {
            Self::Inventory => "Inv",
            Self::UnitSkills => "Skills",
            Self::SettlementWorkforce => "Work",
            Self::Fields => "Fields",
        }
    }

    fn hotkey_hint(self) -> Option<&'static str> {
        match self {
            Self::Inventory => Some("I"),
            Self::UnitSkills => Some("U"),
            Self::SettlementWorkforce => Some("N"),
            Self::Fields => None,
        }
    }

    fn is_settlement(self) -> bool {
        matches!(self, Self::SettlementWorkforce | Self::Fields)
    }
}

const UNIT_UTILITY_BUTTONS: [HudUtilityButton; 2] =
    [HudUtilityButton::Inventory, HudUtilityButton::UnitSkills];

const SETTLEMENT_UTILITY_BUTTONS: [HudUtilityButton; 2] = [
    HudUtilityButton::SettlementWorkforce,
    HudUtilityButton::Fields,
];

/// Permanent HUD utility buttons (Inv, Skills, Work, Fields). Field overlay
/// options live only in the compact Fields popup.
pub const PERMANENT_UTILITY_BUTTONS: [HudUtilityButton; 4] = [
    HudUtilityButton::Inventory,
    HudUtilityButton::UnitSkills,
    HudUtilityButton::SettlementWorkforce,
    HudUtilityButton::Fields,
];

pub fn spawn_utility_panel(parent: &mut ChildSpawnerCommands<'_>) {
    let geom = HudViewportGeometry::default();
    parent
        .spawn((
            UtilityPanelRoot,
            PlayerHudUi,
            hud_section_node_with_horizontal_padding(
                geom.utility_width,
                super::styles::HUD_SECTION_PADDING_X_PX,
                HUD_SECTION_PADDING_X_UTILITY_RIGHT_PX,
            ),
        ))
        .with_children(|panel| {
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(super::styles::HUD_UTILITY_ROW_GAP_PX),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Stretch,
                    ..default()
                })
                .with_children(|col| {
                    spawn_utility_group(col, "Unit controls", &UNIT_UTILITY_BUTTONS);
                    spawn_hud_utility_group_divider(col);
                    spawn_utility_group(col, "Settlement controls", &SETTLEMENT_UTILITY_BUTTONS);
                });
        });
}

fn spawn_utility_group(
    parent: &mut ChildSpawnerCommands<'_>,
    _label: &str,
    buttons: &[HudUtilityButton],
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(super::styles::HUD_UTILITY_ROW_GAP_PX),
            align_items: AlignItems::Stretch,
            ..default()
        })
        .with_children(|group| {
            for button in buttons {
                spawn_utility_button(group, *button);
            }
        });
}

fn spawn_utility_button(parent: &mut ChildSpawnerCommands<'_>, button: HudUtilityButton) -> Entity {
    let radius = BorderRadius::all(Val::Px(6.0));
    let mut entity_commands = parent.spawn((
        button,
        PlayerHudUi,
        Button,
        Node {
            height: Val::Px(HUD_UTILITY_BUTTON_HEIGHT_PX),
            flex_shrink: 0.0,
            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Row,
            position_type: PositionType::Relative,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: radius,
            overflow: Overflow::clip(),
            ..default()
        },
        hud_button_depth_style(),
    ));
    if button == HudUtilityButton::Fields {
        entity_commands.insert(FieldsUtilityButton);
    }
    entity_commands
        .with_children(|shell| {
            spawn_hud_button_top_highlight(shell);
            shell.spawn((
                Text::new(button.label()),
                hud_body_font(),
                TextColor(TEXT_PRIMARY),
            ));
            if let Some(hint) = button.hotkey_hint() {
                shell.spawn((
                    Text::new(hint),
                    hud_caption_font(),
                    TextColor(TEXT_PRIMARY.with_alpha(0.6)),
                ));
            }
            if button == HudUtilityButton::Fields {
                spawn_fields_menu(shell);
            }
        })
        .id()
}

pub fn update_utility_button_hover(
    mut query: Query<
        (
            &Interaction,
            &HudUtilityButton,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Changed<Interaction>,
    >,
) {
    for (interaction, _button, mut bg, mut border) in &mut query {
        let (next_bg, next_border) = hud_button_shell_style(interaction, true, false);
        *bg = next_bg;
        *border = next_border;
    }
}

pub fn handle_utility_button_clicks(
    selection: Res<SelectedUnits>,
    build_mode: Res<BuildModeState>,
    mut inventory_ui: ResMut<crate::ui::gameplay::inventory::InventoryUiState>,
    mut inventory_queue: ResMut<InventoryIntentQueue>,
    mut unit_skills: ResMut<UnitSkillsPanelState>,
    mut workforce: ResMut<SettlementWorkforcePanelState>,
    mut fields_menu: ResMut<FieldsMenuState>,
    menu_block: Option<Res<crate::menu::MenuInputBlock>>,
    interaction: Query<(&Interaction, &HudUtilityButton), Changed<Interaction>>,
) {
    if menu_block.is_some_and(|block| block.blocks()) {
        return;
    }
    if build_mode.search_focused {
        return;
    }

    for (state, button) in &interaction {
        if *state != Interaction::Pressed {
            continue;
        }
        match *button {
            HudUtilityButton::Inventory => {
                let Some(unit_id) = primary_selected_unit(&selection) else {
                    continue;
                };
                if inventory_ui.open {
                    inventory_queue.push(InventoryIntent::Close);
                } else {
                    inventory_queue.push(InventoryIntent::Open(InventoryOpenMode::UnitOnly {
                        unit_id,
                    }));
                }
            }
            HudUtilityButton::UnitSkills => {
                if unit_skills.open {
                    unit_skills.close();
                    continue;
                }
                let Some(unit_id) = primary_selected_unit(&selection) else {
                    continue;
                };
                unit_skills.open_for(unit_id);
            }
            HudUtilityButton::SettlementWorkforce => {
                workforce.toggle();
            }
            HudUtilityButton::Fields => {
                fields_menu.open = !fields_menu.open;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utility_labels_are_ascii_only() {
        for button in PERMANENT_UTILITY_BUTTONS {
            let label = button.label();
            assert!(label.is_ascii());
            if let Some(hint) = button.hotkey_hint() {
                assert!(hint.is_ascii());
            }
            assert!(!label.contains("Thirst"));
        }
    }

    #[test]
    fn permanent_hud_has_exactly_one_fields_button() {
        let fields = PERMANENT_UTILITY_BUTTONS
            .iter()
            .filter(|button| **button == HudUtilityButton::Fields)
            .count();
        assert_eq!(fields, 1);
    }

    #[test]
    fn field_overlay_options_are_not_permanent_buttons() {
        for button in PERMANENT_UTILITY_BUTTONS {
            assert!(!button.label().eq_ignore_ascii_case("water"));
            assert!(!button.label().eq_ignore_ascii_case("stone"));
        }
    }

    #[test]
    fn unit_and_settlement_groups_are_separated() {
        assert!(UNIT_UTILITY_BUTTONS.iter().all(|b| !b.is_settlement()));
        assert!(SETTLEMENT_UTILITY_BUTTONS.iter().all(|b| b.is_settlement()));
    }
}
