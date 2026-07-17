//! Player build catalog panel (ADR-081 B4).

use bevy::prelude::*;

use crate::client::{ClientIntent, ClientIntentQueue};
use crate::world::{
    BuildingCatalog, BuildingCategoryCatalog, BuildingCategoryId, BuildingDefinition,
    BuildingDefinitionId, FootprintSpec,
};

use super::state::{BuildModePhase, BuildModeState};
use crate::ui::gameplay::layout::PlayerHudUi;
use crate::ui::gameplay::styles::{
    BAR_BG, PANEL_BG, TEXT_MUTED, TEXT_PRIMARY, hud_body_font, hud_title_font,
};
/// Root node for the build catalog overlay.
#[derive(Component, Debug)]
pub struct BuildCatalogRoot;

/// Category filter button.
#[derive(Component, Debug, Clone)]
pub struct BuildCategoryButton {
    pub category_id: Option<BuildingCategoryId>,
}

/// Building list entry button.
#[derive(Component, Debug, Clone)]
pub struct BuildDefinitionButton {
    pub definition_id: BuildingDefinitionId,
}

/// Search field marker.
#[derive(Component, Debug)]
pub struct BuildSearchField;

/// Status line under catalog.
#[derive(Component, Debug)]
pub struct BuildStatusText;

const ALL_CATEGORY_LABEL: &str = "All";

pub fn spawn_build_catalog_panel(mut commands: Commands) {
    commands
        .spawn((
            BuildCatalogRoot,
            PlayerHudUi,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                bottom: Val::Px(210.0),
                width: Val::Px(280.0),
                max_height: Val::Px(420.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(6.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(BAR_BG),
            ZIndex(350),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Build Mode"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
            ));
            root.spawn((
                BuildSearchField,
                Button,
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ))
            .with_children(|search| {
                search.spawn((
                    Text::new("Search..."),
                    hud_body_font(),
                    TextColor(TEXT_MUTED),
                ));
            });
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BuildCategoryList,
            ));
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    max_height: Val::Px(260.0),
                    overflow: Overflow::scroll_y(),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BuildDefinitionList,
            ));
            root.spawn((
                BuildStatusText,
                Text::new("Press B to exit • R rotate • Esc cancel"),
                hud_body_font(),
                TextColor(TEXT_MUTED),
            ));
        });
}

#[derive(Component, Debug)]
pub struct BuildCategoryList;

#[derive(Component, Debug)]
pub struct BuildDefinitionList;

pub fn sync_build_catalog_visibility(
    build_mode: Res<BuildModeState>,
    mut query: Query<&mut Node, With<BuildCatalogRoot>>,
) {
    let Ok(mut node) = query.single_mut() else {
        return;
    };
    node.display = if build_mode.is_active() {
        Display::Flex
    } else {
        Display::None
    };
}

pub fn sync_build_catalog_contents(
    build_mode: Res<BuildModeState>,
    building_catalog: Res<BuildingCatalog>,
    category_catalog: Res<BuildingCategoryCatalog>,
    mut commands: Commands,
    category_list: Query<Entity, With<BuildCategoryList>>,
    definition_list: Query<Entity, With<BuildDefinitionList>>,
    mut status: Query<&mut Text, With<BuildStatusText>>,
    mut search_text: Query<&mut Text, (With<BuildSearchField>, Without<BuildStatusText>)>,
) {
    if !build_mode.is_active() {
        return;
    }

    if let Ok(mut text) = status.single_mut() {
        **text = build_status_line(&build_mode, &building_catalog);
    }

    if let Ok(mut text) = search_text.single_mut() {
        let label = if build_mode.search_query.is_empty() {
            "Search...".to_string()
        } else {
            format!("Search: {}", build_mode.search_query)
        };
        **text = label;
    }

    if let Ok(parent) = category_list.single() {
        commands.entity(parent).despawn_children();
        spawn_category_button(&mut commands, parent, None, ALL_CATEGORY_LABEL);
        for category in category_catalog.enabled_definitions() {
            spawn_category_button(
                &mut commands,
                parent,
                Some(category.id.clone()),
                &category.display_name,
            );
        }
    }

    if let Ok(parent) = definition_list.single() {
        commands.entity(parent).despawn_children();
        let mut definitions: Vec<&BuildingDefinition> = building_catalog
            .enabled_definitions()
            .filter(|def| category_matches(&build_mode, def))
            .filter(|def| search_matches(&build_mode, def))
            .collect();
        definitions.sort_by_key(|def| def.id.as_str().to_string());
        for definition in definitions {
            spawn_definition_button(&mut commands, parent, definition);
        }
    }
}

fn build_status_line(build_mode: &BuildModeState, building_catalog: &BuildingCatalog) -> String {
    match &build_mode.phase {
        BuildModePhase::Inactive => "Build Mode".to_string(),
        BuildModePhase::CatalogOpen => "Select a building to place".to_string(),
        BuildModePhase::GhostPlacing { definition_id, .. } => {
            let name = building_catalog
                .get(definition_id)
                .map(|def| def.display_name.as_str())
                .unwrap_or(definition_id.as_str());
            let rotation = build_mode.ghost_rotation_quadrants() * 90;
            let placement_line = if let Some(validation) = &build_mode.last_validation {
                if validation.valid {
                    "Valid — click to place".to_string()
                } else if let Some(reason) = validation.primary_reason {
                    reason.label().to_string()
                } else {
                    "Unavailable".to_string()
                }
            } else {
                "...".to_string()
            };
            let terrain_line = build_mode
                .last_terrain_assessment
                .as_ref()
                .filter(|assessment| !assessment.per_requirement.is_empty())
                .map(format_build_terrain_status)
                .unwrap_or_default();
            if terrain_line.is_empty() {
                format!("{name} • {rotation}° • {placement_line}")
            } else {
                format!("{name} • {rotation}° • {placement_line}\n{terrain_line}")
            }
        }
    }
}

fn format_build_terrain_status(assessment: &crate::world::BuildingTerrainAssessment) -> String {
    let mut lines = Vec::new();
    for requirement in &assessment.per_requirement {
        let field = requirement.field_id.as_str();
        let average = crate::world::format_field_average_display(requirement.average_value);
        let coverage =
            crate::world::format_coverage_display(requirement.usable_coverage_basis_points);
        lines.push(format!("{field}: {average} • Coverage {coverage}"));
    }
    lines.push(format!(
        "Expected Output Rate: {}",
        crate::world::format_efficiency_display(assessment.terrain_efficiency_basis_points)
    ));
    lines.push(format!("Status: {}", assessment.status_label()));
    if !assessment.can_operate {
        lines.push("Placement Allowed".to_string());
    }
    lines.join("\n")
}

fn category_matches(build_mode: &BuildModeState, definition: &BuildingDefinition) -> bool {
    build_mode
        .selected_category
        .as_ref()
        .is_none_or(|selected| selected == &definition.category_id)
}

fn search_matches(build_mode: &BuildModeState, definition: &BuildingDefinition) -> bool {
    if build_mode.search_query.is_empty() {
        return true;
    }
    let needle = build_mode.search_query.to_lowercase();
    definition.display_name.to_lowercase().contains(&needle)
        || definition.id.as_str().to_lowercase().contains(&needle)
}

fn spawn_category_button(
    commands: &mut Commands,
    parent: Entity,
    category_id: Option<BuildingCategoryId>,
    label: &str,
) {
    commands.entity(parent).with_children(|row| {
        row.spawn((
            BuildCategoryButton { category_id },
            Button,
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|btn| {
            btn.spawn((Text::new(label), hud_body_font(), TextColor(TEXT_PRIMARY)));
        });
    });
}

fn spawn_definition_button(
    commands: &mut Commands,
    parent: Entity,
    definition: &BuildingDefinition,
) {
    let summary = footprint_summary(definition);
    let details = format!(
        "{} • {} HP • {:.0}s",
        summary, definition.max_hp, definition.build_time_seconds
    );
    commands.entity(parent).with_children(|list| {
        list.spawn((
            BuildDefinitionButton {
                definition_id: definition.id.clone(),
            },
            Button,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(definition.display_name.clone()),
                hud_title_font(),
                TextColor(TEXT_PRIMARY),
            ));
            btn.spawn((Text::new(details), hud_body_font(), TextColor(TEXT_MUTED)));
        });
    });
}

fn footprint_summary(definition: &BuildingDefinition) -> String {
    match &definition.footprint {
        FootprintSpec::Rectangle {
            width_meters,
            depth_meters,
        } => format!("{width_meters:.0}×{depth_meters:.0}m"),
        FootprintSpec::Circle { radius_meters } => format!("r={radius_meters:.1}m"),
        FootprintSpec::MeshDerived => "mesh".to_string(),
    }
}

pub fn handle_build_catalog_clicks(
    mut build_mode: ResMut<BuildModeState>,
    mut queue: ResMut<ClientIntentQueue>,
    category_buttons: Query<
        (&Interaction, &BuildCategoryButton),
        (Changed<Interaction>, With<Button>),
    >,
    definition_buttons: Query<
        (&Interaction, &BuildDefinitionButton),
        (Changed<Interaction>, With<Button>),
    >,
    search_buttons: Query<&Interaction, (Changed<Interaction>, With<BuildSearchField>)>,
) {
    for (interaction, button) in &category_buttons {
        if *interaction == Interaction::Pressed {
            build_mode.selected_category = button.category_id.clone();
        }
    }

    for (interaction, button) in &definition_buttons {
        if *interaction == Interaction::Pressed {
            queue.push(ClientIntent::SelectBuildingDefinition {
                definition_id: button.definition_id.clone(),
            });
        }
    }

    for interaction in &search_buttons {
        if *interaction == Interaction::Pressed {
            build_mode.search_focused = true;
        }
    }
}

pub fn handle_build_search_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut build_mode: ResMut<BuildModeState>,
) {
    if !build_mode.search_focused {
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        build_mode.search_focused = false;
        return;
    }
    if keyboard.just_pressed(KeyCode::Backspace) {
        build_mode.search_query.pop();
        return;
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        build_mode.search_focused = false;
        return;
    }
    for key in keyboard.get_just_pressed() {
        if let Some(ch) = search_key_char(*key) {
            build_mode.search_query.push(ch);
        }
    }
}

fn search_key_char(key: KeyCode) -> Option<char> {
    match key {
        KeyCode::Space => Some(' '),
        KeyCode::Minus => Some('-'),
        KeyCode::Period => Some('.'),
        KeyCode::KeyA => Some('a'),
        KeyCode::KeyB => Some('b'),
        KeyCode::KeyC => Some('c'),
        KeyCode::KeyD => Some('d'),
        KeyCode::KeyE => Some('e'),
        KeyCode::KeyF => Some('f'),
        KeyCode::KeyG => Some('g'),
        KeyCode::KeyH => Some('h'),
        KeyCode::KeyI => Some('i'),
        KeyCode::KeyJ => Some('j'),
        KeyCode::KeyK => Some('k'),
        KeyCode::KeyL => Some('l'),
        KeyCode::KeyM => Some('m'),
        KeyCode::KeyN => Some('n'),
        KeyCode::KeyO => Some('o'),
        KeyCode::KeyP => Some('p'),
        KeyCode::KeyQ => Some('q'),
        KeyCode::KeyR => Some('r'),
        KeyCode::KeyS => Some('s'),
        KeyCode::KeyT => Some('t'),
        KeyCode::KeyU => Some('u'),
        KeyCode::KeyV => Some('v'),
        KeyCode::KeyW => Some('w'),
        KeyCode::KeyX => Some('x'),
        KeyCode::KeyY => Some('y'),
        KeyCode::KeyZ => Some('z'),
        KeyCode::Digit0 => Some('0'),
        KeyCode::Digit1 => Some('1'),
        KeyCode::Digit2 => Some('2'),
        KeyCode::Digit3 => Some('3'),
        KeyCode::Digit4 => Some('4'),
        KeyCode::Digit5 => Some('5'),
        KeyCode::Digit6 => Some('6'),
        KeyCode::Digit7 => Some('7'),
        KeyCode::Digit8 => Some('8'),
        KeyCode::Digit9 => Some('9'),
        _ => None,
    }
}
