//! Compact Fields popup for settlement terrain overlays.

use bevy::prelude::*;

use crate::terrain::field_overlay::{
    PLAYER_FIELD_MENU_IDS, TerrainOverlayState, apply_player_field_overlay_selection,
};
use crate::world::{TerrainFieldCatalog, TerrainFieldId};

use super::hud::{hud_inset_fill_style, hud_inset_rim_style};
use super::layout::PlayerHudUi;
use super::styles::{
    HUD_FIELDS_MENU_WIDTH_PX, HUD_UTILITY_BUTTON_HEIGHT_PX, TEXT_PRIMARY, hud_body_font,
};
use super::utility_panel::HudUtilityButton;

/// Whether the compact Fields menu is open.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FieldsMenuState {
    pub open: bool,
}

/// Root of the anchored Fields popup (not part of the permanent button row).
#[derive(Component, Debug)]
pub struct FieldsMenuRoot;

/// One field option inside the popup.
#[derive(Component, Debug, Clone)]
pub struct FieldsMenuOption {
    pub field_id: TerrainFieldId,
}

/// Marker on the permanent Fields utility button for hit-testing / queries.
#[derive(Component, Debug)]
pub struct FieldsUtilityButton;

/// Player-facing field ids and display labels for the compact menu.
pub fn player_field_menu_entries() -> [(&'static str, &'static str); 4] {
    [
        ("water", "Water"),
        ("iron", "Iron"),
        ("copper", "Copper"),
        ("stone", "Stone"),
    ]
}

pub fn spawn_fields_menu(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            FieldsMenuRoot,
            PlayerHudUi,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                bottom: Val::Percent(100.0),
                width: Val::Px(HUD_FIELDS_MENU_WIDTH_PX),
                margin: UiRect::bottom(Val::Px(4.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(4.0)),
                display: Display::None,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(super::styles::HUD_MENU_BG),
            BorderColor::all(super::styles::HUD_PLATE_TRIM),
            ZIndex(400),
        ))
        .with_children(|menu| {
            for (id, label) in player_field_menu_entries() {
                spawn_fields_menu_option(menu, TerrainFieldId::new(id), label);
            }
        });
}

fn spawn_fields_menu_option(
    parent: &mut ChildSpawnerCommands<'_>,
    field_id: TerrainFieldId,
    label: &str,
) {
    let radius = BorderRadius::all(Val::Px(4.0));
    parent
        .spawn((
            FieldsMenuOption { field_id },
            PlayerHudUi,
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(HUD_UTILITY_BUTTON_HEIGHT_PX - 4.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(0.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: radius,
                ..default()
            },
            hud_inset_rim_style(),
        ))
        .with_children(|rim| {
            rim.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: radius,
                    ..default()
                },
                hud_inset_fill_style(),
            ))
            .with_children(|fill| {
                fill.spawn((Text::new(label), hud_body_font(), TextColor(TEXT_PRIMARY)));
            });
        });
}

pub fn sync_fields_menu_visibility(
    state: Res<FieldsMenuState>,
    mut menus: Query<&mut Node, With<FieldsMenuRoot>>,
) {
    let display = if state.open {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut menus {
        node.display = display;
    }
}

pub fn sync_fields_menu_option_highlights(
    overlay_state: Res<TerrainOverlayState>,
    mut options: Query<(&FieldsMenuOption, &mut BorderColor)>,
) {
    let active = overlay_state.effective_field().cloned();
    for (option, mut border) in &mut options {
        let selected = active.as_ref() == Some(&option.field_id);
        border.set_all(if selected {
            super::styles::HUD_ACTIVE_ACCENT
        } else {
            super::styles::HUD_PLATE_TRIM
        });
    }
}

pub fn handle_fields_menu_option_clicks(
    mut menu_state: ResMut<FieldsMenuState>,
    mut overlay_state: ResMut<TerrainOverlayState>,
    catalog: Res<TerrainFieldCatalog>,
    interaction: Query<(&Interaction, &FieldsMenuOption), Changed<Interaction>>,
) {
    for (state, option) in &interaction {
        if *state != Interaction::Pressed {
            continue;
        }
        apply_player_field_overlay_selection(&mut overlay_state, &catalog, option.field_id.clone());
        menu_state.open = false;
    }
}

pub fn dismiss_fields_menu_on_outside_click(
    mut menu_state: ResMut<FieldsMenuState>,
    mouse: Res<ButtonInput<MouseButton>>,
    menu_hits: Query<&Interaction, With<FieldsMenuRoot>>,
    option_hits: Query<&Interaction, With<FieldsMenuOption>>,
    fields_button_hits: Query<&Interaction, With<FieldsUtilityButton>>,
) {
    if !menu_state.open || !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let ui_hit = menu_hits
        .iter()
        .chain(option_hits.iter())
        .chain(fields_button_hits.iter())
        .any(|interaction| *interaction != Interaction::None);
    if !ui_hit {
        menu_state.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::field_overlay::TerrainOverlayState;
    use crate::world::TerrainFieldCatalog;

    #[test]
    fn player_field_menu_lists_four_starter_fields() {
        let entries = player_field_menu_entries();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].0, "water");
        assert_eq!(entries[3].0, "stone");
        assert_eq!(PLAYER_FIELD_MENU_IDS.len(), 4);
    }

    #[test]
    fn selecting_field_updates_overlay_authority() {
        let catalog = TerrainFieldCatalog::default();
        let mut overlay = TerrainOverlayState::default();
        let field = TerrainFieldId::new("stone");
        apply_player_field_overlay_selection(&mut overlay, &catalog, field.clone());
        assert_eq!(overlay.effective_field(), Some(&field));
    }

    #[test]
    fn selecting_active_field_clears_overlay() {
        let catalog = TerrainFieldCatalog::default();
        let mut overlay = TerrainOverlayState::default();
        let field = TerrainFieldId::new("iron");
        apply_player_field_overlay_selection(&mut overlay, &catalog, field.clone());
        apply_player_field_overlay_selection(&mut overlay, &catalog, field.clone());
        assert!(overlay.effective_field().is_none());
    }

    #[test]
    fn fields_button_is_distinct_from_field_options() {
        assert_ne!(
            std::mem::discriminant(&HudUtilityButton::Fields),
            std::mem::discriminant(&HudUtilityButton::Inventory)
        );
    }
}
