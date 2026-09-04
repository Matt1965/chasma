//! Unit Skills floating panel (BP5).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::ui::gameplay::floating_window::{
    FloatingGameplayWindowId, FloatingGameplayWindowRoot, FloatingWindowTitleBarDragRegion,
    TITLE_BAR_HEIGHT_PX,
};
use crate::ui::gameplay::layout::PlayerHudUi;
use crate::ui::gameplay::player_hud_state::primary_selected_unit;
use crate::ui::gameplay::styles::{
    BAR_BG, TEXT_MUTED, TEXT_PRIMARY, hud_body_font, hud_title_font,
};
use crate::units::input::SelectedUnits;
use crate::world::{UnitCatalog, WeaponCatalog, WorldData};

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
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(300.0),
                max_height: Val::Percent(70.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(6.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(BAR_BG),
            ZIndex(411),
        ))
        .with_children(|root| {
            root.spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|header| {
                header
                    .spawn((
                        FloatingWindowTitleBarDragRegion {
                            id: FloatingGameplayWindowId::UnitSkills,
                        },
                        Button,
                        Node {
                            flex_grow: 1.0,
                            min_height: Val::Px(TITLE_BAR_HEIGHT_PX),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                    ))
                    .with_children(|title| {
                        title.spawn((
                            UnitSkillsPanelTitleText,
                            Text::new("Unit Skills"),
                            hud_title_font(),
                            TextColor(TEXT_PRIMARY),
                        ));
                    });
                header.spawn((
                    UnitSkillsPanelCloseButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                        ..default()
                    },
                    Text::new("×"),
                    hud_title_font(),
                    TextColor(TEXT_MUTED),
                ));
            });
            root.spawn((
                UnitSkillsPanelBodyText,
                Text::new(""),
                hud_body_font(),
                TextColor(TEXT_PRIMARY),
                Node {
                    overflow: Overflow::scroll_y(),
                    max_height: Val::Px(420.0),
                    ..default()
                },
            ));
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
    let Some(snapshot) =
        build_unit_skills_snapshot(unit_id, &world, &unit_catalog, &weapon_catalog)
    else {
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
        **text = format!("Unit Skills — {}", snapshot.title);
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
