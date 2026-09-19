//! Archetype create/edit modal overlay.

use bevy::prelude::*;

use crate::dev::dev_mode::{DevModeState, DevTextFieldFocus};
use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::DevTooltipContent;
use crate::dev::widgets::{
    FIELD_BG_FOCUSED, FIELD_BG_IDLE, FIELD_BORDER_FOCUSED, FIELD_BORDER_IDLE, spawn_toggle_row,
};
use crate::world::SpeciesCatalog;

use super::actions::{
    DevArchetypeModalCancelButton, DevArchetypeModalDeleteButton, DevArchetypeModalSaveButton,
    DevArchetypeSpeciesToggle,
};
use super::state::DevArchetypeEditorState;

#[derive(Component, Debug, Clone)]
pub(crate) struct DevArchetypeModalRoot;

#[derive(Component, Debug, Clone)]
pub(crate) struct DevArchetypeModalPanel;

#[derive(Component, Debug, Clone)]
pub(crate) struct DevArchetypeModalNameField;

#[derive(Component, Debug, Clone)]
pub(crate) struct DevArchetypeModalNameText;

#[derive(Component, Debug, Clone)]
pub(crate) struct DevArchetypeModalGoldMinField;

#[derive(Component, Debug, Clone)]
pub(crate) struct DevArchetypeModalGoldMinText;

#[derive(Component, Debug, Clone)]
pub(crate) struct DevArchetypeModalGoldMaxField;

#[derive(Component, Debug, Clone)]
pub(crate) struct DevArchetypeModalGoldMaxText;

#[derive(Component, Debug, Clone)]
pub(crate) struct DevArchetypeModalStatusText;

#[derive(Component, Debug, Clone)]
pub(crate) struct DevArchetypeModalSpeciesList;

#[derive(Component, Debug, Clone)]
pub(crate) struct DevArchetypeModalUnitFields;

#[derive(Component, Debug, Clone)]
pub(crate) struct DevArchetypeModalTitleText;

pub fn setup_archetype_editor_modal(mut commands: Commands) {
    commands
        .spawn((
            DevArchetypeModalRoot,
            DevPanelUi,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
            ZIndex(200),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    DevArchetypeModalPanel,
                    DevPanelUi,
                    Node {
                        width: Val::Px(320.0),
                        max_height: Val::Percent(80.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.12, 0.16, 0.98)),
                    BorderColor::all(Color::srgba(0.35, 0.5, 0.62, 1.0)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        DevArchetypeModalTitleText,
                        DevPanelUi,
                        Text::new("Edit Archetype"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.9, 0.95, 1.0, 1.0)),
                    ));
                    spawn_text_field(panel, "Name", DevArchetypeModalNameField, DevArchetypeModalNameText);
                    panel
                        .spawn((
                            DevArchetypeModalUnitFields,
                            DevPanelUi,
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                ..default()
                            },
                        ))
                        .with_children(|unit_fields| {
                            unit_fields.spawn((
                                DevPanelUi,
                                Text::new("Applicable Species"),
                                TextFont {
                                    font_size: 11.0,
                                    ..default()
                                },
                                TextColor(Color::srgba(0.75, 0.85, 0.92, 1.0)),
                            ));
                            unit_fields.spawn((
                                DevArchetypeModalSpeciesList,
                                DevPanelUi,
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(2.0),
                                    max_height: Val::Px(120.0),
                                    overflow: Overflow::scroll_y(),
                                    ..default()
                                },
                            ));
                            spawn_text_field(
                                unit_fields,
                                "Gold Min",
                                DevArchetypeModalGoldMinField,
                                DevArchetypeModalGoldMinText,
                            );
                            spawn_text_field(
                                unit_fields,
                                "Gold Max",
                                DevArchetypeModalGoldMaxField,
                                DevArchetypeModalGoldMaxText,
                            );
                        });
                    panel.spawn((
                        DevArchetypeModalStatusText,
                        DevPanelUi,
                        Text::new(""),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.95, 0.7, 0.55, 1.0)),
                    ));
                    panel
                        .spawn((
                            DevPanelUi,
                            Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(6.0),
                                ..default()
                            },
                        ))
                        .with_children(|row| {
                            spawn_modal_button(row, "Save", DevArchetypeModalSaveButton);
                            spawn_modal_button(row, "Delete", DevArchetypeModalDeleteButton);
                            spawn_modal_button(row, "Cancel", DevArchetypeModalCancelButton);
                        });
                });
        });
}

fn spawn_text_field(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &str,
    field: impl Component + Clone,
    text: impl Component + Clone,
) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
        ))
        .with_children(|col| {
            col.spawn((
                DevPanelUi,
                Text::new(label),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgba(0.7, 0.8, 0.88, 1.0)),
            ));
            col.spawn((
                field,
                DevPanelUi,
                Button,
                Node {
                    min_height: Val::Px(22.0),
                    padding: UiRect::horizontal(Val::Px(6.0)),
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(FIELD_BG_IDLE),
                BorderColor::all(FIELD_BORDER_IDLE),
            ))
            .with_children(|btn| {
                btn.spawn((
                    text,
                    DevPanelUi,
                    Text::new(""),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.9, 0.95, 1.0, 1.0)),
                ));
            });
        });
}

fn spawn_modal_button<M: Component>(parent: &mut ChildSpawnerCommands<'_>, label: &str, marker: M) {
    parent.spawn((
        marker,
        DevPanelUi,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.12, 0.2, 0.28, 0.95)),
    ))
    .with_children(|btn| {
        btn.spawn((
            DevPanelUi,
            Text::new(label),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(Color::srgba(0.88, 0.92, 0.96, 1.0)),
        ));
    });
}

pub fn sync_archetype_editor_modal(
    editor: Res<DevArchetypeEditorState>,
    dev_state: Res<DevModeState>,
    species_catalog: Res<SpeciesCatalog>,
    child_of: Query<&ChildOf>,
    mut visibility: ParamSet<(
        Query<(&mut Node, &mut Visibility), With<DevArchetypeModalRoot>>,
        Query<&mut Visibility, With<DevArchetypeModalUnitFields>>,
        Query<&mut Visibility, With<DevArchetypeModalDeleteButton>>,
    )>,
    mut title: Query<&mut Text, With<DevArchetypeModalTitleText>>,
    mut name_text: Query<&mut Text, (With<DevArchetypeModalNameText>, Without<DevArchetypeModalGoldMinText>, Without<DevArchetypeModalGoldMaxText>, Without<DevArchetypeModalStatusText>, Without<DevArchetypeModalTitleText>)>,
    mut gold_min: Query<&mut Text, (With<DevArchetypeModalGoldMinText>, Without<DevArchetypeModalNameText>, Without<DevArchetypeModalGoldMaxText>, Without<DevArchetypeModalStatusText>, Without<DevArchetypeModalTitleText>)>,
    mut gold_max: Query<&mut Text, (With<DevArchetypeModalGoldMaxText>, Without<DevArchetypeModalNameText>, Without<DevArchetypeModalGoldMinText>, Without<DevArchetypeModalStatusText>, Without<DevArchetypeModalTitleText>)>,
    mut status: Query<&mut Text, (With<DevArchetypeModalStatusText>, Without<DevArchetypeModalNameText>, Without<DevArchetypeModalGoldMinText>, Without<DevArchetypeModalGoldMaxText>, Without<DevArchetypeModalTitleText>)>,
    species_list: Query<Entity, With<DevArchetypeModalSpeciesList>>,
    species_toggles: Query<(Entity, &DevArchetypeSpeciesToggle)>,
    mut commands: Commands,
) {
    let open = editor.modal_open && dev_state.enabled;
    for (mut node, mut root_visibility) in visibility.p0().iter_mut() {
        set_modal_root_shown(&mut node, &mut root_visibility, open);
    }
    if !open {
        despawn_archetype_species_toggle_rows(&species_toggles, &child_of, &mut commands);
        return;
    }

    let is_unit = editor.is_unit_modal();
    for mut unit_visibility in visibility.p1().iter_mut() {
        *unit_visibility = if is_unit {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    if let Ok(mut text) = title.single_mut() {
        **text = match editor.mode {
            Some(super::state::ArchetypeEditorMode::UnitCreate) => "Save Unit Archetype".to_string(),
            Some(super::state::ArchetypeEditorMode::UnitEdit) => "Edit Unit Archetype".to_string(),
            Some(super::state::ArchetypeEditorMode::BuildingCreate) => {
                "Save Building Archetype".to_string()
            }
            Some(super::state::ArchetypeEditorMode::BuildingEdit) => {
                "Edit Building Archetype".to_string()
            }
            None => "Archetype".to_string(),
        };
    }

    if let Ok(mut text) = name_text.single_mut() {
        **text = if dev_state.text_focus == DevTextFieldFocus::ArchetypeName {
            editor.name_input.clone()
        } else {
            editor.name_input.clone()
        };
    }
    if let Ok(mut text) = gold_min.single_mut() {
        **text = editor.gold_min_input.clone();
    }
    if let Ok(mut text) = gold_max.single_mut() {
        **text = editor.gold_max_input.clone();
    }
    if let Ok(mut text) = status.single_mut() {
        **text = editor.status_message.clone();
    }

    let show_delete = matches!(
        editor.mode,
        Some(super::state::ArchetypeEditorMode::UnitEdit | super::state::ArchetypeEditorMode::BuildingEdit)
    );
    for mut delete_visibility in visibility.p2().iter_mut() {
        *delete_visibility = if show_delete {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    if is_unit {
        if let Ok(list_entity) = species_list.single() {
            let existing: std::collections::HashSet<_> = species_toggles
                .iter()
                .map(|(_, toggle)| toggle.0.clone())
                .collect();
            let desired: std::collections::HashSet<_> = species_catalog
                .definitions()
                .iter()
                .filter(|species| species.enabled)
                .map(|species| species.id.clone())
                .collect();
            if existing != desired {
                despawn_archetype_species_toggle_rows(&species_toggles, &child_of, &mut commands);
                commands.entity(list_entity).with_children(|list| {
                    for species in species_catalog.definitions() {
                        if !species.enabled {
                            continue;
                        }
                        spawn_toggle_row(
                            list,
                            &species.display_name,
                            DevTooltipContent::new(format!(
                                "Applicable to {}",
                                species.display_name
                            )),
                            DevArchetypeSpeciesToggle(species.id.clone()),
                        );
                    }
                });
            }
        }
    }
}

pub fn sync_archetype_species_toggle_marks(
    editor: Res<DevArchetypeEditorState>,
    mut query: Query<
        (
            Entity,
            &DevArchetypeSpeciesToggle,
            &Children,
            &mut crate::dev::widgets::DevWidgetToggle,
        ),
        Without<DevArchetypeModalRoot>,
    >,
    mut marks: Query<&mut Visibility, With<crate::dev::widgets::DevWidgetToggleMark>>,
) {
    if !editor.modal_open {
        return;
    }
    for (_, toggle, children, mut widget) in &mut query {
        let on = editor.selected_species.contains(&toggle.0);
        widget.disabled = false;
        for child in children.iter() {
            if let Ok(mut visibility) = marks.get_mut(child) {
                *visibility = if on {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

pub fn sync_archetype_modal_field_styles(
    dev_state: Res<DevModeState>,
    editor: Res<DevArchetypeEditorState>,
    mut name_field: Query<
        (&mut BackgroundColor, &mut BorderColor),
        (With<DevArchetypeModalNameField>, Without<DevArchetypeModalGoldMinField>, Without<DevArchetypeModalGoldMaxField>),
    >,
    mut gold_min_field: Query<
        (&mut BackgroundColor, &mut BorderColor),
        (With<DevArchetypeModalGoldMinField>, Without<DevArchetypeModalNameField>, Without<DevArchetypeModalGoldMaxField>),
    >,
    mut gold_max_field: Query<
        (&mut BackgroundColor, &mut BorderColor),
        (With<DevArchetypeModalGoldMaxField>, Without<DevArchetypeModalNameField>, Without<DevArchetypeModalGoldMinField>),
    >,
) {
    if !editor.modal_open {
        return;
    }
    paint_field(
        &mut name_field,
        dev_state.text_focus == DevTextFieldFocus::ArchetypeName,
    );
    paint_field(
        &mut gold_min_field,
        dev_state.text_focus == DevTextFieldFocus::ArchetypeGoldMin,
    );
    paint_field(
        &mut gold_max_field,
        dev_state.text_focus == DevTextFieldFocus::ArchetypeGoldMax,
    );
}

fn paint_field<Q: bevy::ecs::query::QueryFilter>(
    query: &mut Query<(&mut BackgroundColor, &mut BorderColor), Q>,
    focused: bool,
) {
    for (mut bg, mut border) in query.iter_mut() {
        *bg = BackgroundColor(if focused {
            FIELD_BG_FOCUSED
        } else {
            FIELD_BG_IDLE
        });
        border.set_all(if focused {
            FIELD_BORDER_FOCUSED
        } else {
            FIELD_BORDER_IDLE
        });
    }
}

pub fn handle_archetype_modal_field_clicks(
    mut dev_state: ResMut<DevModeState>,
    editor: Res<DevArchetypeEditorState>,
    name: Query<&Interaction, (With<DevArchetypeModalNameField>, Without<DevArchetypeModalGoldMinField>, Without<DevArchetypeModalGoldMaxField>)>,
    gold_min: Query<&Interaction, (With<DevArchetypeModalGoldMinField>, Without<DevArchetypeModalNameField>, Without<DevArchetypeModalGoldMaxField>)>,
    gold_max: Query<&Interaction, (With<DevArchetypeModalGoldMaxField>, Without<DevArchetypeModalNameField>, Without<DevArchetypeModalGoldMinField>)>,
) {
    if !editor.modal_open {
        return;
    }
    if name.iter().any(|i| *i == Interaction::Pressed) {
        dev_state.text_focus = DevTextFieldFocus::ArchetypeName;
    } else if gold_min.iter().any(|i| *i == Interaction::Pressed) {
        dev_state.text_focus = DevTextFieldFocus::ArchetypeGoldMin;
    } else if gold_max.iter().any(|i| *i == Interaction::Pressed) {
        dev_state.text_focus = DevTextFieldFocus::ArchetypeGoldMax;
    }
}

fn set_modal_root_shown(node: &mut Node, visibility: &mut Visibility, shown: bool) {
    if shown {
        *visibility = Visibility::Visible;
        node.display = Display::Flex;
    } else {
        *visibility = Visibility::Hidden;
        node.display = Display::None;
    }
}

fn despawn_archetype_species_toggle_rows(
    species_toggles: &Query<(Entity, &DevArchetypeSpeciesToggle)>,
    child_of: &Query<&ChildOf>,
    commands: &mut Commands,
) {
    let rows = species_toggle_row_entities(species_toggles, child_of);
    for row in rows {
        commands.entity(row).despawn();
    }
}

fn species_toggle_row_entities(
    species_toggles: &Query<(Entity, &DevArchetypeSpeciesToggle)>,
    child_of: &Query<&ChildOf>,
) -> std::collections::HashSet<Entity> {
    species_toggles
        .iter()
        .filter_map(|(entity, _)| child_of.get(entity).ok().map(|parent| parent.parent()))
        .collect()
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::dev::dev_mode::DevModeState;
    use crate::world::relationship::species::{SpeciesCatalog, SpeciesDefinition};
    use crate::world::relationship::SpeciesId;
    use bevy::ecs::system::RunSystemOnce;

    fn test_world() -> World {
        let mut world = World::new();
        world.insert_resource(DevArchetypeEditorState::default());
        let mut dev_state = DevModeState::default();
        dev_state.enabled = true;
        world.insert_resource(dev_state);
        world.insert_resource(
            SpeciesCatalog::from_definitions(vec![
                SpeciesDefinition::new(
                    SpeciesId::new("human"),
                    "Human",
                    "test species",
                    true,
                ),
            ])
                .unwrap(),
        );
        world
    }

    fn run_sync(world: &mut World) {
        world
            .run_system_once(sync_archetype_editor_modal)
            .expect("sync archetype modal");
    }

    fn modal_root_state(world: &mut World) -> Option<(Visibility, Display)> {
        world
            .query_filtered::<(&Visibility, &Node), With<DevArchetypeModalRoot>>()
            .iter(world)
            .next()
            .map(|(visibility, node)| (*visibility, node.display))
    }

    fn count_modal_roots(world: &mut World) -> usize {
        world.query::<&DevArchetypeModalRoot>().iter(world).count()
    }

    fn count_species_toggle_rows(world: &mut World) -> usize {
        world
            .query::<&DevArchetypeSpeciesToggle>()
            .iter(world)
            .count()
    }

    fn open_unit_modal(world: &mut World) {
        {
            let mut editor = world.resource_mut::<DevArchetypeEditorState>();
            editor.modal_open = true;
            editor.mode = Some(super::super::state::ArchetypeEditorMode::UnitCreate);
            editor.name_input = "Bandit".to_string();
            editor.selected_species.insert(SpeciesId::new("human"));
        }
        run_sync(world);
    }

    fn close_modal(world: &mut World) {
        world.resource_mut::<DevArchetypeEditorState>().close();
        run_sync(world);
    }

    #[test]
    fn set_modal_root_shown_hides_layout_and_visibility() {
        let mut node = Node::default();
        let mut visibility = Visibility::Visible;
        set_modal_root_shown(&mut node, &mut visibility, false);
        assert_eq!(visibility, Visibility::Hidden);
        assert_eq!(node.display, Display::None);
        set_modal_root_shown(&mut node, &mut visibility, true);
        assert_eq!(visibility, Visibility::Visible);
        assert_eq!(node.display, Display::Flex);
    }

    #[test]
    fn modal_root_exists_after_setup() {
        let mut world = test_world();
        world
            .run_system_once(setup_archetype_editor_modal)
            .expect("setup archetype modal");
        run_sync(&mut world);
        assert_eq!(count_modal_roots(&mut world), 1);
        let (visibility, display) = modal_root_state(&mut world).expect("modal root");
        assert_eq!(visibility, Visibility::Hidden);
        assert_eq!(display, Display::None);
    }

    #[test]
    fn open_modal_shows_root_and_spawns_species_rows() {
        let mut world = test_world();
        world
            .run_system_once(setup_archetype_editor_modal)
            .expect("setup archetype modal");
        open_unit_modal(&mut world);
        let (visibility, display) = modal_root_state(&mut world).expect("modal root");
        assert_eq!(visibility, Visibility::Visible);
        assert_eq!(display, Display::Flex);
        assert_eq!(count_species_toggle_rows(&mut world), 1);
    }

    #[test]
    fn close_modal_hides_root_and_removes_species_rows() {
        let mut world = test_world();
        world
            .run_system_once(setup_archetype_editor_modal)
            .expect("setup archetype modal");
        open_unit_modal(&mut world);
        close_modal(&mut world);
        let (visibility, display) = modal_root_state(&mut world).expect("modal root");
        assert_eq!(visibility, Visibility::Hidden);
        assert_eq!(display, Display::None);
        assert_eq!(count_species_toggle_rows(&mut world), 0);
        assert_eq!(count_modal_roots(&mut world), 1);
    }

    #[test]
    fn repeated_open_close_does_not_accumulate_species_rows() {
        let mut world = test_world();
        world
            .run_system_once(setup_archetype_editor_modal)
            .expect("setup archetype modal");
        for _ in 0..3 {
            open_unit_modal(&mut world);
            assert_eq!(count_species_toggle_rows(&mut world), 1);
            close_modal(&mut world);
            assert_eq!(count_species_toggle_rows(&mut world), 0);
        }
        assert_eq!(count_modal_roots(&mut world), 1);
    }

    #[test]
    fn close_clears_editor_state() {
        let mut editor = DevArchetypeEditorState::default();
        editor.modal_open = true;
        editor.mode = Some(super::super::state::ArchetypeEditorMode::UnitCreate);
        editor.name_input = "Bandit".to_string();
        editor.status_message = "error".to_string();
        editor.selected_species.insert(SpeciesId::new("human"));
        editor.close();
        assert!(!editor.modal_open);
        assert!(editor.mode.is_none());
        assert!(editor.name_input.is_empty());
        assert!(editor.status_message.is_empty());
        assert!(editor.selected_species.is_empty());
    }
}
