//! Selected Object window — shell, sync, and action buttons (Slice 5).

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::debug::CommandTraceBuffer;
use crate::dev::DevModeState;
use crate::dev::gizmo::{DevToolState, TransformEditState};
use crate::dev::input::DevPanelUi;
use crate::dev::inspector::{
    BuildingProductionRepeatModeButton, BuildingProductionRepeatModeButtonText, WorldInspectorState,
};
use crate::dev::window::{DevWindowBody, DevWindowId, DevWindowRegistry, DevWindowUi};
use crate::ui::gameplay::primary_selected_unit;
use crate::units::input::SelectedUnits;

use crate::dev::navigation_editor::{
    navigation_editor_owns_session, spawn_open_navigation_editor_button,
};
use crate::dev::tooltip::DevTooltipTarget;

use super::building_actions_ui::DevBuildingActionsRoot;
use super::format::{
    EMPTY_STATE, format_building_diagnostics, format_building_navigation_strip,
    format_building_summary, format_doodad_diagnostics, format_doodad_summary,
    format_pile_diagnostics, format_pile_summary, format_unit_diagnostics, format_unit_summary,
};
use super::state::SelectedObjectUiState;

use crate::dev::widgets::theme::{
    BTN_BG_IDLE, SPACE_BUTTON_PAD_X, SPACE_BUTTON_PAD_Y, SPACE_CONTROL, TEXT_PRIMARY,
    label_text_font,
};

fn action_tooltip(action: SelectedObjectAction) -> &'static str {
    match action {
        SelectedObjectAction::Move => {
            "Translate gizmo for doodads and buildings. Shortcut: , (comma). Unavailable for fixed-dimension buildings."
        }
        SelectedObjectAction::Rotate => {
            "Rotate gizmo for doodads and buildings. Shortcut: . (period)."
        }
        SelectedObjectAction::Scale => {
            "Scale gizmo when the definition allows instance scale. Shortcut: / (slash). \
             Disabled when authored dimensions are fixed."
        }
        SelectedObjectAction::Delete => {
            "Request delete for the selected object. Requires confirmation."
        }
        SelectedObjectAction::ConfirmDelete => "Permanently delete the selected object.",
        SelectedObjectAction::CancelDelete => "Cancel the pending delete request.",
        SelectedObjectAction::ExitBlueprintInspection => {
            "Leave blueprint inspection mode and restore normal selection view."
        }
        SelectedObjectAction::ExitBlueprintEdit => {
            "Exit blueprint edit mode. Prompts if navigation edits are unsaved."
        }
        SelectedObjectAction::CancelBlueprintPending => {
            "Cancel the pending destructive navigation action."
        }
        SelectedObjectAction::CancelVariantDraft => "Discard the Save As Variant draft.",
    }
}

#[derive(Component, Debug)]
pub(crate) struct DevSelectedObjectUi;

#[derive(Component, Debug)]
pub(crate) struct DevSelectedObjectSummaryText;

#[derive(Component, Debug)]
pub(crate) struct DevSelectedObjectDiagnosticsText;

#[derive(Component, Debug)]
pub(crate) struct DevSelectedObjectNavigationText;

#[derive(Component, Debug)]
pub(crate) struct DevSelectedObjectActionButton {
    pub action: SelectedObjectAction,
}

#[derive(Component, Debug)]
pub(crate) struct DevSelectedObjectToggleButton {
    pub toggle: SelectedObjectToggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedObjectAction {
    Move,
    Rotate,
    Scale,
    Delete,
    ConfirmDelete,
    CancelDelete,
    ExitBlueprintInspection,
    ExitBlueprintEdit,
    CancelBlueprintPending,
    CancelVariantDraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedObjectToggle {
    Diagnostics,
}

/// Spawn Selected Object content inside the dedicated window body.
pub fn setup_selected_object_panel(
    mut commands: Commands,
    bodies: Query<(Entity, &DevWindowBody)>,
) {
    for (entity, body) in &bodies {
        if body.id != DevWindowId::SelectedObject {
            continue;
        }
        commands.entity(entity).with_children(|panel| {
            panel
                .spawn((
                    DevSelectedObjectUi,
                    DevPanelUi,
                    DevWindowUi,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(SPACE_CONTROL),
                        ..default()
                    },
                ))
                .with_children(|root| {
                    for (label, action) in [
                        ("Move (,)", SelectedObjectAction::Move),
                        ("Rotate (.)", SelectedObjectAction::Rotate),
                        ("Scale (/)", SelectedObjectAction::Scale),
                        ("Delete", SelectedObjectAction::Delete),
                        ("Confirm delete", SelectedObjectAction::ConfirmDelete),
                        ("Cancel", SelectedObjectAction::CancelDelete),
                        (
                            "Exit blueprint inspection",
                            SelectedObjectAction::ExitBlueprintInspection,
                        ),
                        (
                            "Exit blueprint edit",
                            SelectedObjectAction::ExitBlueprintEdit,
                        ),
                        (
                            "Cancel blueprint action",
                            SelectedObjectAction::CancelBlueprintPending,
                        ),
                        (
                            "Cancel variant draft",
                            SelectedObjectAction::CancelVariantDraft,
                        ),
                    ] {
                        root.spawn((
                            DevSelectedObjectActionButton { action },
                            DevTooltipTarget::new(action_tooltip(action)),
                            DevPanelUi,
                            DevWindowUi,
                            Button,
                            Node {
                                padding: UiRect::axes(
                                    Val::Px(SPACE_BUTTON_PAD_X),
                                    Val::Px(SPACE_BUTTON_PAD_Y),
                                ),
                                display: Display::None,
                                ..default()
                            },
                            BackgroundColor(BTN_BG_IDLE),
                            Text::new(label),
                            label_text_font(),
                            TextColor(TEXT_PRIMARY),
                        ));
                    }

                    root.spawn((
                        BuildingProductionRepeatModeButton,
                        DevPanelUi,
                        DevWindowUi,
                        Button,
                        Node {
                            padding: UiRect::axes(
                                Val::Px(SPACE_BUTTON_PAD_X),
                                Val::Px(SPACE_BUTTON_PAD_Y),
                            ),
                            display: Display::None,
                            ..default()
                        },
                        DevTooltipTarget::new(
                            "Cycle production repeat mode (Continuous ↔ Count). Dev override \
                             of building production policy.",
                        ),
                        BackgroundColor(BTN_BG_IDLE),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            BuildingProductionRepeatModeButtonText,
                            Text::new("Production repeat: —"),
                            label_text_font(),
                            TextColor(TEXT_PRIMARY),
                        ));
                    });

                    root.spawn((
                        DevSelectedObjectToggleButton {
                            toggle: SelectedObjectToggle::Diagnostics,
                        },
                        DevTooltipTarget::new(
                            "Expand detailed diagnostics for the current selection. \
                             Command trace and inspector snapshots; does not affect simulation.",
                        ),
                        DevPanelUi,
                        DevWindowUi,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.16, 0.22, 0.30, 0.95)),
                        Text::new("Diagnostics"),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.8, 0.88, 0.95, 1.0)),
                    ));

                    spawn_open_navigation_editor_button(root, "Open Navigation Editor");

                    super::building_actions_ui::spawn_building_dev_actions(root);

                    root.spawn((
                        DevSelectedObjectNavigationText,
                        DevPanelUi,
                        Text::new(""),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.72, 0.82, 0.9, 1.0)),
                        Node {
                            display: Display::None,
                            ..default()
                        },
                    ));

                    root.spawn((
                        DevSelectedObjectSummaryText,
                        DevPanelUi,
                        Text::new(EMPTY_STATE),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.88, 0.92, 0.96, 1.0)),
                    ));

                    root.spawn((
                        DevSelectedObjectDiagnosticsText,
                        DevPanelUi,
                        Text::new(""),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.65, 0.75, 0.85, 1.0)),
                        Node {
                            display: Display::None,
                            max_height: Val::Px(240.0),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                    ));
                });
        });
        return;
    }
}

/// Sync Selected Object window from shared selection and inspector snapshots.
pub fn sync_selected_object_panel(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    world_selection: Res<WorldSelectionState>,
    selected_units: Res<SelectedUnits>,
    inspector: Res<WorldInspectorState>,
    ui_state: Res<SelectedObjectUiState>,
    blueprint_inspection: Res<crate::dev::BlueprintInspectionState>,
    tool_state: Res<DevToolState>,
    edit: Res<TransformEditState>,
    trace: Res<CommandTraceBuffer>,
    mut texts: ParamSet<(
        Query<
            &mut Text,
            (
                With<DevSelectedObjectSummaryText>,
                Without<DevSelectedObjectDiagnosticsText>,
                Without<DevSelectedObjectNavigationText>,
            ),
        >,
        Query<
            &mut Text,
            (
                With<DevSelectedObjectDiagnosticsText>,
                Without<DevSelectedObjectSummaryText>,
                Without<DevSelectedObjectNavigationText>,
            ),
        >,
        Query<
            &mut Text,
            (
                With<DevSelectedObjectNavigationText>,
                Without<DevSelectedObjectSummaryText>,
                Without<DevSelectedObjectDiagnosticsText>,
            ),
        >,
        Query<&mut Text, With<BuildingProductionRepeatModeButtonText>>,
    )>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<DevSelectedObjectDiagnosticsText>>,
        Query<
            &mut Node,
            (
                With<DevSelectedObjectNavigationText>,
                Without<DevSelectedObjectDiagnosticsText>,
            ),
        >,
        Query<(&DevSelectedObjectActionButton, &mut Node)>,
        Query<(&DevSelectedObjectToggleButton, &mut Node)>,
        Query<
            &mut Node,
            (
                With<BuildingProductionRepeatModeButton>,
                Without<BuildingProductionRepeatModeButtonText>,
            ),
        >,
        Query<&mut Node, With<DevBuildingActionsRoot>>,
    )>,
) {
    let visible = dev_state.enabled && registry.is_visible(DevWindowId::SelectedObject);

    let show_transform = visible
        && matches!(
            world_selection.category,
            WorldSelectionCategory::Doodad | WorldSelectionCategory::Building
        );
    let show_delete = visible
        && matches!(
            world_selection.category,
            WorldSelectionCategory::Doodad
                | WorldSelectionCategory::Building
                | WorldSelectionCategory::ItemPile
        );
    let pending = ui_state.pending_delete.is_some();
    let show_building = visible && world_selection.category == WorldSelectionCategory::Building;
    let blueprint = inspector.blueprint_snapshot.as_ref();
    let inspection_active = blueprint.is_some_and(|bp| bp.inspection_active);
    let edit_active = blueprint.is_some_and(|bp| bp.edit_active);
    let blueprint_pending = blueprint_inspection.pending_confirmation.is_some();
    let variant_draft = blueprint.is_some_and(|bp| bp.variant_draft_active);

    let nav_editor_active =
        navigation_editor_owns_session(dev_state.enabled, &registry, &blueprint_inspection);

    for (button, mut node) in nodes.p2().iter_mut() {
        let show = match button.action {
            SelectedObjectAction::Move
            | SelectedObjectAction::Rotate
            | SelectedObjectAction::Scale => show_transform && !pending && !nav_editor_active,
            SelectedObjectAction::Delete => show_delete && !pending,
            SelectedObjectAction::ConfirmDelete | SelectedObjectAction::CancelDelete => pending,
            SelectedObjectAction::ExitBlueprintInspection => {
                show_building && inspection_active && !edit_active
            }
            SelectedObjectAction::ExitBlueprintEdit => show_building && edit_active,
            SelectedObjectAction::CancelBlueprintPending => show_building && blueprint_pending,
            SelectedObjectAction::CancelVariantDraft => show_building && variant_draft,
        };
        node.display = if show { Display::Flex } else { Display::None };
    }
    if let Ok(mut btn_node) = nodes.p4().single_mut() {
        btn_node.display = if show_building {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut root) = nodes.p5().single_mut() {
        root.display = if show_building {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut label) = texts.p3().single_mut() {
        let mode_label = inspector
            .building_snapshot
            .as_ref()
            .and_then(|snap| snap.repeat_mode.as_deref())
            .unwrap_or("—");
        **label = format!("Production repeat: {mode_label} (click to toggle)");
    }

    let has_selection = world_selection.category != WorldSelectionCategory::None;
    for (toggle, mut node) in nodes.p3().iter_mut() {
        let show = visible && has_selection && toggle.toggle == SelectedObjectToggle::Diagnostics;
        node.display = if show { Display::Flex } else { Display::None };
    }

    if let Ok(mut node) = nodes.p0().single_mut() {
        node.display = if visible && ui_state.diagnostics_expanded && has_selection {
            Display::Flex
        } else {
            Display::None
        };
    }

    if !visible {
        return;
    }

    if let Ok(mut label) = texts.p0().single_mut() {
        **label = if pending {
            "Confirm deletion?".into()
        } else if let Some(snapshot) = inspector.doodad_snapshot.as_ref() {
            format_doodad_summary(snapshot, &tool_state)
        } else if let Some(snapshot) = inspector.building_snapshot.as_ref() {
            let mut body = format_building_summary(snapshot);
            body.push_str("\n\n");
            body.push_str(&format_building_navigation_strip(
                inspector.blueprint_snapshot.as_ref(),
            ));
            body
        } else if let Some(snapshot) = inspector.unit_snapshot.as_ref() {
            let count = if world_selection.category == WorldSelectionCategory::Units {
                selected_units.0.len().max(1)
            } else {
                1
            };
            format_unit_summary(snapshot, count)
        } else if let Some(snapshot) = inspector.pile_snapshot.as_ref() {
            format_pile_summary(snapshot)
        } else if !inspector.last_message.is_empty() {
            inspector.last_message.clone()
        } else {
            EMPTY_STATE.into()
        };
    }

    if let Ok(mut label) = texts.p1().single_mut() {
        **label = if ui_state.diagnostics_expanded {
            if let Some(snapshot) = inspector.doodad_snapshot.as_ref() {
                format_doodad_diagnostics(snapshot, &tool_state, &edit)
            } else if let Some(snapshot) = inspector.building_snapshot.as_ref() {
                format_building_diagnostics(
                    snapshot,
                    inspector.production_advanced_expanded,
                    inspector.blueprint_snapshot.as_ref(),
                )
            } else if let Some(snapshot) = inspector.unit_snapshot.as_ref() {
                let mut body = format_unit_diagnostics(snapshot);
                let unit_filter = world_selection
                    .primary_unit(&selected_units)
                    .or_else(|| primary_selected_unit(&selected_units));
                let log_lines = crate::debug::recent_combat_log_lines(&trace, unit_filter, 6);
                if !log_lines.is_empty() {
                    body.push_str("\nCombat log:\n");
                    for line in log_lines {
                        body.push_str(&format!("  {line}\n"));
                    }
                }
                body
            } else if let Some(snapshot) = inspector.pile_snapshot.as_ref() {
                format_pile_diagnostics(snapshot)
            } else {
                String::new()
            }
        } else {
            String::new()
        };
    }
}
