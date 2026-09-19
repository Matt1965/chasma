//! Road Editor window panel.

use bevy::prelude::*;

use crate::dev::dev_mode::DevModeState;
use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::DevTooltipTarget;
use crate::dev::widgets::spawn_action_button;
use crate::dev::window::{DevWindowBody, DevWindowId, DevWindowRegistry, DevWindowUi};

use super::actions::RoadEditorButton;
use super::state::RoadEditorUiState;

#[derive(Component, Debug)]
pub(crate) struct DevRoadsWindowUi;

#[derive(Component, Debug)]
pub(crate) struct DevRoadEditorTitleText;

#[derive(Component, Debug)]
pub(crate) struct DevRoadEditorStatusText;

#[derive(Component, Debug)]
pub(crate) struct DevRoadNameField;

#[derive(Component, Debug)]
pub(crate) struct DevRoadSelectionText;

pub fn sync_dev_roads_panel_visibility(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut visibility: Query<&mut Visibility, With<DevRoadsWindowUi>>,
) {
    let visible = dev_state.enabled && registry.is_visible(DevWindowId::Roads);
    for mut vis in &mut visibility {
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

pub fn setup_roads_window_panel(mut commands: Commands, bodies: Query<(Entity, &DevWindowBody)>) {
    for (entity, body) in &bodies {
        if body.id != DevWindowId::Roads {
            continue;
        }
        commands.entity(entity).with_children(|panel| {
            panel
                .spawn((
                    DevRoadsWindowUi,
                    DevPanelUi,
                    DevWindowUi,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        ..default()
                    },
                ))
                .with_children(|root| {
                    root.spawn((
                        DevRoadEditorTitleText,
                        DevPanelUi,
                        Text::new("Road Editor"),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.8, 0.88, 0.95, 1.0)),
                    ));
                    spawn_action_button(
                        root,
                        "Create Road",
                        Some("Place control points on terrain, then Finish"),
                        RoadEditorButton::Create,
                    );
                    spawn_action_button(
                        root,
                        "Finish",
                        Some("Complete the current create or extend operation"),
                        RoadEditorButton::Finish,
                    );
                    spawn_action_button(
                        root,
                        "Cancel",
                        Some("Abandon the current draft operation"),
                        RoadEditorButton::Cancel,
                    );
                    spawn_action_button(
                        root,
                        "Insert Point",
                        Some("Click a road segment to insert a control point"),
                        RoadEditorButton::InsertPoint,
                    );
                    spawn_action_button(
                        root,
                        "Extend Start",
                        Some("Append control points before the road start"),
                        RoadEditorButton::ExtendStart,
                    );
                    spawn_action_button(
                        root,
                        "Extend End",
                        Some("Append control points after the road end"),
                        RoadEditorButton::ExtendEnd,
                    );
                    spawn_action_button(
                        root,
                        "Delete Point",
                        Some("Remove the selected control point"),
                        RoadEditorButton::DeletePoint,
                    );
                    spawn_action_button(
                        root,
                        "Cycle Style",
                        Some("Cycle Trail / DirtRoad / MajorRoad for the selected or draft road"),
                        RoadEditorButton::CycleStyle,
                    );
                    spawn_action_button(
                        root,
                        "Delete Road",
                        Some("Remove the selected road from the network"),
                        RoadEditorButton::DeleteRoad,
                    );
                    spawn_action_button(
                        root,
                        "Save Roads",
                        Some("Write the road network to assets/worlds/main/roads/network.ron"),
                        RoadEditorButton::Save,
                    );
                    spawn_action_button(
                        root,
                        "Reload",
                        Some("Reload roads from disk and discard unsaved edits"),
                        RoadEditorButton::Reload,
                    );
                    root.spawn((
                        DevRoadSelectionText,
                        DevPanelUi,
                        Text::new("Selected: none"),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.75, 0.85, 0.92, 1.0)),
                    ));
                    root.spawn((
                        DevRoadNameField,
                        DevPanelUi,
                        DevTooltipTarget::new(
                            "Click to edit the selected road display name (RoadId stays stable)",
                        ),
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.08, 0.11, 0.14, 0.95)),
                        Text::new("Name: —"),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.85, 0.9, 0.95, 1.0)),
                    ));
                    root.spawn((
                        DevRoadEditorStatusText,
                        DevPanelUi,
                        Text::new(""),
                        TextFont {
                            font_size: 9.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.65, 0.78, 0.88, 1.0)),
                    ));
                });
        });
        return;
    }
}

pub fn sync_road_editor_panel(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    editor: Res<RoadEditorUiState>,
    mut title: Query<&mut Text, (With<DevRoadEditorTitleText>, Without<DevRoadEditorStatusText>, Without<DevRoadSelectionText>, Without<DevRoadNameField>)>,
    mut selection: Query<&mut Text, (With<DevRoadSelectionText>, Without<DevRoadEditorTitleText>, Without<DevRoadEditorStatusText>, Without<DevRoadNameField>)>,
    mut name_field: Query<&mut Text, (With<DevRoadNameField>, Without<DevRoadEditorTitleText>, Without<DevRoadEditorStatusText>, Without<DevRoadSelectionText>)>,
    mut status: Query<&mut Text, (With<DevRoadEditorStatusText>, Without<DevRoadEditorTitleText>, Without<DevRoadSelectionText>, Without<DevRoadNameField>)>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Roads) {
        return;
    }
    if let Ok(mut text) = title.single_mut() {
        **text = format!("Road Editor{}", editor.window_title_suffix()).into();
    }
    if let Ok(mut text) = selection.single_mut() {
        **text = if let Some(road_id) = &editor.selected_road_id {
            if let Some(index) = editor.selected_point_index {
                format!("Selected: {} point {}", road_id, index + 1)
            } else {
                format!("Selected: {}", road_id)
            }
        } else if editor.mode == super::state::RoadEditMode::Create {
            format!("Creating: {} points", editor.create_points.len())
        } else {
            "Selected: none".to_string()
        }
        .into();
    }
    if let Ok(mut text) = name_field.single_mut() {
        **text = if editor.name_input.is_empty() {
            "Name: —".to_string()
        } else {
            format!("Name: {}", editor.name_input)
        }
        .into();
    }
    if let Ok(mut text) = status.single_mut() {
        let dirty = if editor.dirty { "Unsaved changes. " } else { "" };
        **text = format!("{dirty}{}", editor.status_message).into();
    }
}
