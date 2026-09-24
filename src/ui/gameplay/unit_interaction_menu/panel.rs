//! Cursor-anchored unit interaction menu.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::PrimaryWindow;

use crate::ui::gameplay::hud::hud_button_shell_style;
use crate::ui::gameplay::layout::PlayerHudUi;
use crate::ui::gameplay::styles::{TEXT_MUTED, TEXT_PRIMARY, hud_body_font, panel_title_font};
use crate::world::relationship::AuthoredRelationshipCatalog;
use crate::world::{DialogueActionKind, UnitCatalog, WorldData};

use super::content::{build_interaction_menu_rows, interaction_menu_title};
use super::state::UnitInteractionMenuState;

const MENU_WIDTH_PX: f32 = 220.0;
const MENU_PADDING_PX: f32 = 6.0;
const OPTION_HEIGHT_PX: f32 = 28.0;
const MENU_MARGIN_PX: f32 = 8.0;

#[derive(Component, Debug)]
pub struct UnitInteractionMenuBackdrop;

#[derive(Component, Debug)]
pub struct UnitInteractionMenuRoot;

#[derive(Component, Debug)]
pub struct UnitInteractionMenuTitle;

#[derive(Component, Debug, Clone, Copy)]
pub struct UnitInteractionMenuOptionButton(pub DialogueActionKind);

#[derive(Component, Debug)]
pub struct UnitInteractionMenuOptionText;

pub fn spawn_unit_interaction_menu(mut commands: Commands) {
    commands.spawn((
        UnitInteractionMenuBackdrop,
        PlayerHudUi,
        Button,
        Interaction::None,
        FocusPolicy::Block,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::None,
            ..default()
        },
        ZIndex(420),
    ));

    commands
        .spawn((
            UnitInteractionMenuRoot,
            PlayerHudUi,
            FocusPolicy::Block,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(MENU_WIDTH_PX),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(MENU_PADDING_PX)),
                display: Display::None,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(super::super::styles::HUD_MENU_BG),
            BorderColor::all(super::super::styles::HUD_PLATE_TRIM),
            ZIndex(421),
        ))
        .with_children(|menu| {
            menu.spawn((
                UnitInteractionMenuTitle,
                Text::new(""),
                panel_title_font(),
                TextColor(TEXT_PRIMARY),
            ));
            for kind in DialogueActionKind::ALL {
                spawn_menu_option_button(menu, kind);
            }
        });
}

fn spawn_menu_option_button(parent: &mut ChildSpawnerCommands<'_>, kind: DialogueActionKind) {
    parent
        .spawn((
            UnitInteractionMenuOptionButton(kind),
            PlayerHudUi,
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(OPTION_HEIGHT_PX),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(0.0)),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(super::super::styles::HUD_PLATE_TRIM),
        ))
        .with_children(|row| {
            row.spawn((
                UnitInteractionMenuOptionText,
                Text::new(kind.label()),
                hud_body_font(),
                TextColor(TEXT_PRIMARY),
            ));
        });
}

pub fn sync_unit_interaction_menu_visibility(
    menu: Res<UnitInteractionMenuState>,
    mut backdrop: Query<&mut Node, (With<UnitInteractionMenuBackdrop>, Without<UnitInteractionMenuRoot>)>,
    mut roots: Query<&mut Node, (With<UnitInteractionMenuRoot>, Without<UnitInteractionMenuBackdrop>)>,
) {
    let display = if menu.open {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in backdrop.iter_mut().chain(roots.iter_mut()) {
        node.display = display;
    }
}

pub fn sync_unit_interaction_menu_layout(
    menu: Res<UnitInteractionMenuState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut roots: Query<&mut Node, With<UnitInteractionMenuRoot>>,
) {
    if !menu.open {
        return;
    }
    let viewport = windows
        .single()
        .ok()
        .map(|window| Vec2::new(window.width(), window.height()))
        .unwrap_or(Vec2::ONE);
    let max_left = (viewport.x - MENU_WIDTH_PX - MENU_MARGIN_PX).max(MENU_MARGIN_PX);
    let max_top = (viewport.y - 160.0).max(MENU_MARGIN_PX);
    let left = menu.screen_position.x.clamp(MENU_MARGIN_PX, max_left);
    let top = menu.screen_position.y.clamp(MENU_MARGIN_PX, max_top);

    for mut node in &mut roots {
        node.left = Val::Px(left);
        node.top = Val::Px(top);
    }
}

pub fn sync_unit_interaction_menu(
    menu: Res<UnitInteractionMenuState>,
    world: Res<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    authored_relationships: Res<AuthoredRelationshipCatalog>,
    mut title: Query<&mut Text, With<UnitInteractionMenuTitle>>,
    mut options: Query<
        (
            Entity,
            &UnitInteractionMenuOptionButton,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        (Without<UnitInteractionMenuTitle>, Without<UnitInteractionMenuOptionText>),
    >,
    mut option_text: Query<
        (&mut Text, &mut TextColor),
        (With<UnitInteractionMenuOptionText>, Without<UnitInteractionMenuTitle>),
    >,
    children: Query<&Children>,
) {
    if !menu.open {
        return;
    }
    let actor = match menu.actor_unit_id {
        Some(id) => id,
        None => return,
    };
    let target = match menu.target_unit_id {
        Some(id) => id,
        None => return,
    };

    if let Ok(mut text) = title.single_mut() {
        **text = interaction_menu_title(&world, &unit_catalog, target);
    }

    let rows = build_interaction_menu_rows(
        &world,
        &authored_relationships,
        world.relationship_standing_store(),
        actor,
        target,
    );

    for (entity, button, interaction, mut node, mut bg, mut border) in &mut options {
        let row = rows.iter().find(|row| row.kind == button.0);
        let visible = row.is_some();
        let enabled = row.is_some_and(|row| row.enabled);
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        let (next_bg, next_border) = hud_button_shell_style(interaction, enabled, false);
        *bg = next_bg;
        *border = next_border;
        if let Some(row) = row {
            if let Ok(kids) = children.get(entity) {
                if let Some(child) = kids.first() {
                    if let Ok((mut text, mut color)) = option_text.get_mut(*child) {
                        **text = row.label.clone();
                        *color = TextColor(if enabled {
                            TEXT_PRIMARY
                        } else {
                            TEXT_MUTED
                        });
                    }
                }
            }
        }
    }
}
