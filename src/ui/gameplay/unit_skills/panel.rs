//! Unit Skills floating panel (BP5).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::ui::gameplay::floating_window::{
    FloatingGameplayWindowId, FloatingGameplayWindowRoot, floating_window_shell_colors,
    floating_window_shell_node, spawn_floating_title_rail, spawn_floating_window_body,
    spawn_floating_window_inner_frame,
};
use crate::ui::gameplay::layout::PlayerHudUi;
use crate::ui::gameplay::player_hud_state::primary_selected_unit;
use crate::ui::gameplay::styles::{TEXT_PRIMARY, panel_body_font, panel_title_font};
use crate::ui::gameplay::text::format_ui_title;
use crate::units::input::SelectedUnits;
use crate::world::{UnitCatalog, WeaponCatalog, WorkSkillCatalog, WorldData};

use super::content::{build_unit_skills_snapshot, format_unit_skills_panel_text};
use super::state::UnitSkillsPanelState;

#[derive(Component, Debug)]
pub struct UnitSkillsPanelRoot;

#[derive(Component, Debug)]
pub struct UnitSkillsPanelCloseButton;

#[derive(Component, Debug)]
pub struct UnitSkillsPanelTitleText;

#[derive(Component, Debug)]
pub struct UnitSkillsPanelBodyText;

pub fn spawn_unit_skills_panel(mut commands: Commands) {
    let (shell_bg, shell_border) = floating_window_shell_colors();
    let mut shell_node = floating_window_shell_node();
    shell_node.width = Val::Px(300.0);
    shell_node.max_height = Val::Percent(70.0);

    commands
        .spawn((
            UnitSkillsPanelRoot,
            FloatingGameplayWindowRoot {
                id: FloatingGameplayWindowId::UnitSkills,
            },
            PlayerHudUi,
            Button,
            Interaction::None,
            FocusPolicy::Block,
            shell_node,
            shell_bg,
            shell_border,
            ZIndex(411),
        ))
        .with_children(|root| {
            spawn_floating_window_inner_frame(root, |frame| {
                spawn_floating_title_rail(
                    frame,
                    FloatingGameplayWindowId::UnitSkills,
                    |title| {
                        title.spawn((
                            UnitSkillsPanelTitleText,
                            Text::new("Unit Skills"),
                            panel_title_font(),
                            TextColor(TEXT_PRIMARY),
                        ));
                    },
                    Some((UnitSkillsPanelCloseButton, "X")),
                );
                spawn_floating_window_body(frame, |body| {
                    body.spawn((
                        UnitSkillsPanelBodyText,
                        Text::new(""),
                        panel_body_font(),
                        TextColor(TEXT_PRIMARY),
                        Node {
                            overflow: Overflow::scroll_y(),
                            max_height: Val::Px(420.0),
                            ..default()
                        },
                    ));
                });
            });
        });
}

pub fn sync_unit_skills_panel_visibility(
    panel: Res<UnitSkillsPanelState>,
    mut roots: Query<&mut Node, With<UnitSkillsPanelRoot>>,
) {
    let display = if panel.open {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut roots {
        node.display = display;
    }
}

pub fn reconcile_unit_skills_panel(
    selection: Res<SelectedUnits>,
    world: Res<WorldData>,
    mut panel: ResMut<UnitSkillsPanelState>,
) {
    if !panel.open {
        return;
    }
    let primary = primary_selected_unit(&selection);
    let valid = primary.is_some_and(|id| world.get_unit(id).is_some());
    if !valid {
        panel.close();
        return;
    }
    panel.displayed_unit_id = primary;
}

pub fn sync_unit_skills_panel(
    panel: Res<UnitSkillsPanelState>,
    world: Res<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    weapon_catalog: Res<WeaponCatalog>,
    work_skill_catalog: Res<WorkSkillCatalog>,
    mut cache: Local<Option<String>>,
    mut body: Query<&mut Text, With<UnitSkillsPanelBodyText>>,
    mut title: Query<
        &mut Text,
        (
            With<UnitSkillsPanelTitleText>,
            Without<UnitSkillsPanelBodyText>,
        ),
    >,
) {
    if !panel.open {
        *cache = None;
        return;
    }
    let Some(unit_id) = panel.displayed_unit_id else {
        return;
    };
    let Some(snapshot) = build_unit_skills_snapshot(
        unit_id,
        &world,
        &unit_catalog,
        &weapon_catalog,
        &work_skill_catalog,
    ) else {
        return;
    };
    let formatted = format_unit_skills_panel_text(&snapshot);
    if cache.as_ref() == Some(&formatted) {
        return;
    }
    *cache = Some(formatted.clone());
    if let Ok(mut text) = body.single_mut() {
        **text = formatted;
    }
    if let Ok(mut text) = title.single_mut() {
        **text = format_ui_title("Unit Skills", &snapshot.title);
    }
}

pub fn handle_unit_skills_close_button(
    mut panel: ResMut<UnitSkillsPanelState>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<UnitSkillsPanelCloseButton>)>,
) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            panel.close();
        }
    }
}
