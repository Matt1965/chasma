//! Dialogue floating panel.

use bevy::ecs::system::ParamSet;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::ui::gameplay::floating_window::{
    FloatingGameplayWindowId, FloatingGameplayWindowRoot, floating_window_shell_colors,
    floating_window_shell_node, spawn_floating_raised_button, spawn_floating_title_rail,
    spawn_floating_window_body, spawn_floating_window_inner_frame,
};
use crate::ui::gameplay::hud::hud_button_shell_style;
use crate::ui::gameplay::layout::PlayerHudUi;
use crate::ui::gameplay::styles::{TEXT_MUTED, TEXT_PRIMARY, panel_body_font, panel_title_font};
use crate::world::relationship::AuthoredRelationshipCatalog;
use crate::world::{DialogueActionKind, UnitCatalog, WorldData};

use super::content::{build_dialogue_option_rows, target_display_name};
use super::state::{DialogueSessionState, DialogueView};

#[derive(Component, Debug)]
pub struct DialoguePanelRoot;

#[derive(Component, Debug)]
pub struct DialoguePanelCloseButton;

#[derive(Component, Debug)]
pub struct DialoguePanelTitleText;

#[derive(Component, Debug)]
pub struct DialoguePanelBodyText;

#[derive(Component, Debug)]
pub struct DialoguePanelFeedbackText;

#[derive(Component, Debug, Clone, Copy)]
pub struct DialogueOptionButton(DialogueActionKind);

#[derive(Component, Debug)]
pub struct DialogueBackButton;

pub fn spawn_dialogue_panel(mut commands: Commands) {
    let (shell_bg, shell_border) = floating_window_shell_colors();
    let mut shell_node = floating_window_shell_node();
    shell_node.width = Val::Px(280.0);
    shell_node.max_height = Val::Percent(70.0);

    commands
        .spawn((
            DialoguePanelRoot,
            FloatingGameplayWindowRoot {
                id: FloatingGameplayWindowId::Dialogue,
            },
            PlayerHudUi,
            Button,
            Interaction::None,
            FocusPolicy::Block,
            shell_node,
            shell_bg,
            shell_border,
            Visibility::Hidden,
            ZIndex(412),
        ))
        .with_children(|root| {
            spawn_floating_window_inner_frame(root, |frame| {
                spawn_floating_title_rail(
                    frame,
                    FloatingGameplayWindowId::Dialogue,
                    |title| {
                        title.spawn((
                            DialoguePanelTitleText,
                            Text::new("Dialogue"),
                            panel_title_font(),
                            TextColor(TEXT_PRIMARY),
                        ));
                    },
                    Some((DialoguePanelCloseButton, "X")),
                );
                spawn_floating_window_body(frame, |body| {
                    body.spawn((
                        DialoguePanelBodyText,
                        Text::new(""),
                        panel_body_font(),
                        TextColor(TEXT_PRIMARY),
                    ));
                    body.spawn((
                        DialoguePanelFeedbackText,
                        Text::new(""),
                        panel_body_font(),
                        TextColor(TEXT_MUTED),
                    ));
                    let button_node = Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    };
                    for kind in DialogueActionKind::ALL {
                        spawn_floating_raised_button(
                            body,
                            DialogueOptionButton(kind),
                            kind.label(),
                            button_node.clone(),
                        );
                    }
                    spawn_floating_raised_button(
                        body,
                        DialogueBackButton,
                        "Back",
                        button_node.clone(),
                    );
                    spawn_floating_raised_button(
                        body,
                        DialoguePanelCloseButton,
                        "Close",
                        button_node,
                    );
                });
            });
        });
}

pub fn sync_dialogue_panel_visibility(
    dialogue: Res<DialogueSessionState>,
    mut roots: Query<(&mut Node, &mut Visibility), With<DialoguePanelRoot>>,
) {
    let visible = dialogue.open;
    for (mut node, mut visibility) in &mut roots {
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub fn reconcile_dialogue_panel(
    mut dialogue: ResMut<DialogueSessionState>,
    world: Res<WorldData>,
) {
    if !dialogue.open {
        return;
    }
    let valid = match (dialogue.actor_unit_id, dialogue.target_unit_id) {
        (Some(actor), Some(target)) => {
            world.get_unit(actor).is_some() && world.get_unit(target).is_some()
        }
        _ => false,
    };
    if !valid {
        dialogue.close();
    }
}

pub fn sync_dialogue_panel(
    dialogue: Res<DialogueSessionState>,
    world: Res<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    authored_relationships: Res<AuthoredRelationshipCatalog>,
    mut texts: ParamSet<(
        Query<&mut Text, With<DialoguePanelTitleText>>,
        Query<&mut Text, (With<DialoguePanelBodyText>, Without<DialoguePanelFeedbackText>)>,
        Query<&mut Text, (With<DialoguePanelFeedbackText>, Without<DialoguePanelBodyText>)>,
    )>,
    mut option_buttons: Query<
        (
            &DialogueOptionButton,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut Node,
        ),
        (Without<DialogueBackButton>, Without<DialoguePanelCloseButton>),
    >,
    mut back_buttons: Query<&mut Node, With<DialogueBackButton>>,
) {
    if !dialogue.open {
        return;
    }
    let actor = match dialogue.actor_unit_id {
        Some(id) => id,
        None => return,
    };
    let target = match dialogue.target_unit_id {
        Some(id) => id,
        None => return,
    };

    if let Ok(mut text) = texts.p0().single_mut() {
        **text = target_display_name(&world, &unit_catalog, target);
    }

    if let Ok(mut text) = texts.p1().single_mut() {
        **text = match &dialogue.view {
            DialogueView::Options => String::new(),
            DialogueView::TalkResponse(content) => content.lines.join("\n"),
            DialogueView::TradePlaceholder | DialogueView::RecruitPlaceholder => String::new(),
        };
    }

    if let Ok(mut text) = texts.p2().single_mut() {
        **text = dialogue.feedback.clone();
    }

    let options_view = matches!(dialogue.view, DialogueView::Options);
    let rows = build_dialogue_option_rows(
        &world,
        &authored_relationships,
        world.relationship_standing_store(),
        actor,
        target,
    );

    for (button, interaction, mut bg, mut border, mut node) in &mut option_buttons {
        let row = rows.iter().find(|row| row.kind == button.0);
        let visible = options_view && row.is_some();
        let enabled = row.is_some_and(|row| row.enabled);
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        let (next_bg, next_border) = hud_button_shell_style(interaction, enabled, false);
        *bg = next_bg;
        *border = next_border;
    }

    let show_back = matches!(
        dialogue.view,
        DialogueView::TradePlaceholder | DialogueView::RecruitPlaceholder
    );
    for mut node in &mut back_buttons {
        node.display = if show_back {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub fn handle_dialogue_close_button(
    mut dialogue: ResMut<DialogueSessionState>,
    close: Query<&Interaction, With<DialoguePanelCloseButton>>,
) {
    if close.iter().any(|i| *i == Interaction::Pressed) {
        dialogue.close();
    }
}

pub fn handle_dialogue_back_button(
    mut dialogue: ResMut<DialogueSessionState>,
    back: Query<&Interaction, With<DialogueBackButton>>,
) {
    if back.iter().any(|i| *i == Interaction::Pressed) {
        dialogue.back_to_options();
    }
}

pub fn handle_dialogue_option_buttons(
    mut dialogue: ResMut<DialogueSessionState>,
    mut pending: ResMut<crate::client::PendingDialogueInteractionState>,
    mut trade: ResMut<crate::client::TradeSessionState>,
    mut inventory_ui: ResMut<crate::ui::gameplay::InventoryUiState>,
    mut inventory_queue: ResMut<crate::client::inventory_intent::InventoryIntentQueue>,
    mut registry: ResMut<crate::ui::gameplay::FloatingGameplayWindowRegistry>,
    mut world: ResMut<WorldData>,
    authored_relationships: Res<AuthoredRelationshipCatalog>,
    unit_catalog: Res<UnitCatalog>,
    weapon_catalog: Res<crate::world::WeaponCatalog>,
    doodad_catalog: Res<crate::world::DoodadCatalog>,
    nav_config: Res<crate::world::NavigationConfig>,
    buttons: Query<(&Interaction, &DialogueOptionButton), Without<DialogueBackButton>>,
) {
    if !dialogue.open || !matches!(dialogue.view, DialogueView::Options) {
        return;
    }
    let actor = match dialogue.actor_unit_id {
        Some(id) => id,
        None => return,
    };
    let target = match dialogue.target_unit_id {
        Some(id) => id,
        None => return,
    };
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if button.0 == DialogueActionKind::Trade {
            let standing = world.relationship_standing_store();
            if !crate::world::evaluate_dialogue_option(
                &world,
                &authored_relationships,
                standing,
                actor,
                target,
                DialogueActionKind::Trade,
            )
            .is_available()
            {
                continue;
            }
            let outcome = crate::client::dispatch_dialogue_action(
                &mut world,
                &mut dialogue,
                &mut pending,
                &mut trade,
                &mut inventory_ui,
                &mut inventory_queue,
                &mut registry,
                &authored_relationships,
                &unit_catalog,
                &weapon_catalog,
                &doodad_catalog,
                &nav_config,
                actor,
                target,
                DialogueActionKind::Trade,
            );
            if outcome == crate::client::DialogueDispatchOutcome::Opened {
                dialogue.close();
            }
            continue;
        }
        let standing = world.relationship_standing_store();
        let availability = crate::world::evaluate_dialogue_option(
            &world,
            &authored_relationships,
            standing,
            actor,
            target,
            button.0,
        );
        if !availability.is_available() {
            continue;
        }
        dialogue.select_option(button.0, actor, target);
    }
}
