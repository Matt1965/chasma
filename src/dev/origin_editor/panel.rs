//! Origin Editor dev window panel.

use bevy::prelude::*;

use crate::dev::dev_mode::DevModeState;
use crate::dev::input::DevPanelUi;
use crate::dev::widgets::spawn_action_button;
use crate::dev::window::{DevWindowBody, DevWindowId, DevWindowRegistry, DevWindowUi};
use crate::world::OriginCatalog;

use super::actions::OriginEditorButton;
use super::state::DevOriginEditorState;

#[derive(Component, Debug)]
pub(crate) struct DevOriginEditorWindowUi;

#[derive(Component, Debug)]
pub(crate) struct DevOriginEditorSummaryText;

#[derive(Component, Debug)]
pub(crate) struct DevOriginEditorStatusText;

pub fn sync_dev_origin_editor_panel_visibility(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut visibility: Query<&mut Visibility, With<DevOriginEditorWindowUi>>,
) {
    let visible = dev_state.enabled && registry.is_visible(DevWindowId::OriginEditor);
    for mut vis in &mut visibility {
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

pub fn setup_origin_editor_panel(mut commands: Commands, bodies: Query<(Entity, &DevWindowBody)>) {
    for (entity, body) in &bodies {
        if body.id != DevWindowId::OriginEditor {
            continue;
        }
        commands.entity(entity).with_children(|panel| {
            panel
                .spawn((
                    DevOriginEditorWindowUi,
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
                        DevOriginEditorSummaryText,
                        DevPanelUi,
                        Text::new("No origins loaded"),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.8, 0.88, 0.95, 1.0)),
                    ));
                    spawn_action_button(
                        root,
                        "Prev",
                        Some("Select previous origin"),
                        OriginEditorButton::Prev,
                    );
                    spawn_action_button(
                        root,
                        "Next",
                        Some("Select next origin"),
                        OriginEditorButton::Next,
                    );
                    spawn_action_button(
                        root,
                        "Add From Selection",
                        Some("Create a new origin from the selected unit"),
                        OriginEditorButton::AddFromSelection,
                    );
                    spawn_action_button(
                        root,
                        "Overwrite",
                        Some("Replace the selected origin members from the selected unit"),
                        OriginEditorButton::OverwriteFromSelection,
                    );
                    spawn_action_button(
                        root,
                        "Delete",
                        Some("Delete the selected origin (confirm required)"),
                        OriginEditorButton::Delete,
                    );
                    spawn_action_button(
                        root,
                        "Confirm Delete",
                        Some("Confirm deletion of the selected origin"),
                        OriginEditorButton::ConfirmDelete,
                    );
                    spawn_action_button(
                        root,
                        "Cancel Delete",
                        Some("Cancel pending deletion"),
                        OriginEditorButton::CancelDelete,
                    );
                    spawn_action_button(
                        root,
                        "Set Start Here",
                        Some("Click terrain to set the origin spawn anchor"),
                        OriginEditorButton::SetStartHere,
                    );
                    spawn_action_button(
                        root,
                        "Facing From Camera",
                        Some("Set origin facing from the RTS camera yaw"),
                        OriginEditorButton::FacingFromCamera,
                    );
                    spawn_action_button(
                        root,
                        "Save",
                        Some("Write origins to assets/worlds/main/origins.ron"),
                        OriginEditorButton::Save,
                    );
                    spawn_action_button(
                        root,
                        "Reload",
                        Some("Reload origins from disk"),
                        OriginEditorButton::Reload,
                    );
                    root.spawn((
                        DevOriginEditorStatusText,
                        DevPanelUi,
                        Text::new(""),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.7, 0.8, 0.88, 1.0)),
                    ));
                });
        });
    }
}

pub fn sync_origin_editor_panel(
    origins: Res<OriginCatalog>,
    editor: Res<DevOriginEditorState>,
    mut summary: Query<
        &mut Text,
        (
            With<DevOriginEditorSummaryText>,
            Without<DevOriginEditorStatusText>,
        ),
    >,
    mut status: Query<
        &mut Text,
        (
            With<DevOriginEditorStatusText>,
            Without<DevOriginEditorSummaryText>,
        ),
    >,
) {
    let definitions = origins.definitions();
    if let Ok(mut text) = summary.single_mut() {
        if definitions.is_empty() {
            **text = "No origins loaded".into();
        } else {
            let index = editor
                .selected_index
                .min(definitions.len().saturating_sub(1));
            let origin = &definitions[index];
            **text = format!(
                "[{}/{}] {} ({})\n{}\nStart: ({:.1}, {:.1}) yaw {:.0}° | {} member(s)",
                index + 1,
                definitions.len(),
                origin.display_name,
                origin.id.as_str(),
                editor.scratch_description,
                origin.start_x,
                origin.start_z,
                origin.yaw_deg,
                origin.member_count(),
            );
        }
    }
    if let Ok(mut text) = status.single_mut() {
        let mode = if editor.pending_start_pick {
            " | PICK START"
        } else if editor.pending_delete {
            " | CONFIRM DELETE"
        } else {
            ""
        };
        let dirty = if editor.dirty { " (dirty)" } else { "" };
        **text = format!("{}{}{}", editor.status_message, dirty, mode);
    }
}
