//! Navigation Editor window — panel spawn and sync (Slice 7).

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::dev::input::DevPanelUi;
use crate::dev::inspector::{BlueprintEditTool, BlueprintInspectionState, WorldInspectorState};
use crate::dev::tooltip::{DevTooltipContent, DevTooltipTarget};
use crate::dev::widgets::{BTN_BG_ACTIVE, BTN_BG_IDLE, spawn_bounded_slider_row};
use crate::dev::window::{DevWindowBody, DevWindowId, DevWindowRegistry, DevWindowUi};

use super::commands::authority_tooltip;
use super::opacity::NAV_EDITOR_BUILDING_OPACITY_FIELD_ID;
use super::state::{NavigationEditorUiState, navigation_editor_owns_session};

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorUi;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorSummaryText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorValidationText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorOpacityRow;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorSourceText;

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorActionButton {
    pub action: NavigationEditorAction,
}

#[derive(Component, Debug)]
pub(crate) struct DevNavigationEditorOpenButton;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationEditorAction {
    InspectMode,
    EditMode,
    ExitEdit,
    FloorPrev,
    FloorNext,
    ToolSelect,
    ToolAddCorner,
    ToolAddEntrance,
    DeleteSelection,
    RadiusUp,
    RadiusDown,
    FrameBuilding,
    ReturnCamera,
    Regenerate,
    Validate,
    SaveInstance,
    ApplyToAsset,
    ResetToAsset,
    SaveAsVariant,
    ConfirmPending,
    CancelPending,
    CancelVariant,
    CreateVariant,
    OverlayBlueprint,
    OverlayEntrances,
    OverlayRuntimePath,
}

pub fn setup_navigation_editor_panel(
    mut commands: Commands,
    bodies: Query<(Entity, &DevWindowBody)>,
) {
    for (entity, body) in &bodies {
        if body.id != DevWindowId::NavigationEditor {
            continue;
        }
        commands.entity(entity).with_children(|panel| {
            panel
                .spawn((
                    DevNavigationEditorUi,
                    DevPanelUi,
                    DevWindowUi,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                ))
                .with_children(|root| {
                    root.spawn((
                        DevNavigationEditorSummaryText,
                        DevPanelUi,
                        Text::new(
                            "Select a placed building to inspect or edit its navigation blueprint.",
                        ),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.88, 0.92, 0.96, 1.0)),
                    ));
                    root.spawn((
                        DevNavigationEditorValidationText,
                        DevPanelUi,
                        Text::new(""),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.75, 0.82, 0.9, 1.0)),
                        Node {
                            display: Display::None,
                            ..default()
                        },
                    ));
                    root.spawn((
                        DevNavigationEditorOpacityRow,
                        DevPanelUi,
                        DevWindowUi,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(2.0),
                            display: Display::None,
                            ..default()
                        },
                    ))
                    .with_children(|row| {
                        spawn_bounded_slider_row(
                            row,
                            "Building opacity",
                            NAV_EDITOR_BUILDING_OPACITY_FIELD_ID,
                            110.0,
                            "Editor presentation only - fades the selected building mesh so \
                                 blueprint geometry stays readable. Does not affect collision, \
                                 pathfinding, or saved data.",
                        );
                    });
                    root.spawn((
                        DevNavigationEditorSourceText,
                        DevPanelUi,
                        Text::new(""),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.7, 0.78, 0.88, 1.0)),
                        Node {
                            display: Display::None,
                            ..default()
                        },
                    ));
                    for (label, action, tip) in action_rows() {
                        spawn_action_button(root, label, action, tip);
                    }
                });
        });
        return;
    }
}

fn action_rows() -> [(&'static str, NavigationEditorAction, &'static str); 26] {
    [
        (
            "Inspect",
            NavigationEditorAction::InspectMode,
            "Read-only blueprint view with floor selection.",
        ),
        (
            "Edit",
            NavigationEditorAction::EditMode,
            "Edit walkable outline corners and entrances in the world view.",
        ),
        (
            "Exit edit",
            NavigationEditorAction::ExitEdit,
            "Return to inspect mode. Prompts if edits are unsaved.",
        ),
        (
            "Floor -",
            NavigationEditorAction::FloorPrev,
            "Previous floor ID (sparse/negative IDs supported). Building-local elevation.",
        ),
        (
            "Floor +",
            NavigationEditorAction::FloorNext,
            "Next floor ID. Draft edits on other floors are kept.",
        ),
        (
            "Select",
            NavigationEditorAction::ToolSelect,
            "Select and drag corners, entrances, or transitions.",
        ),
        (
            "Add corner",
            NavigationEditorAction::ToolAddCorner,
            "Place exactly one corner on a walkable edge (building-local XZ), then return to Select. \
             Right-click cancels back to Select.",
        ),
        (
            "Add entrance",
            NavigationEditorAction::ToolAddEntrance,
            "Click to place an exterior portal entrance on the active floor. \
             The portal disc is the usable exterior traversal radius; it does not cut the floor polygon. \
             Interior spawn marks where the unit continues inside; the floor walkable outline controls movement after entry. \
             Visualization: exterior disc, interior spawn marker, and the connection between them.",
        ),
        (
            "Delete",
            NavigationEditorAction::DeleteSelection,
            "Delete the selected element. Also available via Delete when the editor is focused.",
        ),
        (
            "Radius +",
            NavigationEditorAction::RadiusUp,
            "Increase selected entrance or transition radius by 0.1 m. \
             Entrance radius is the exterior traversal-disc size (portal between surface and floor). \
             It does not modify the floor polygon; occupancy may stay traversable through the disc. \
             Interior spawn is separate and controls where the unit appears inside.",
        ),
        (
            "Radius -",
            NavigationEditorAction::RadiusDown,
            "Decrease selected entrance or transition radius by 0.1 m. \
             Entrance radius is the exterior traversal-disc size only - not a floor cut.",
        ),
        (
            "Frame building",
            NavigationEditorAction::FrameBuilding,
            "Center the camera on the selected building.",
        ),
        (
            "Return view",
            NavigationEditorAction::ReturnCamera,
            "Restore the camera from before inspection began.",
        ),
        (
            "Regenerate",
            NavigationEditorAction::Regenerate,
            "Rebuild the editable draft from the building model (occupancy_collision preferred). \
             Saved instance overrides and asset defaults stay unchanged until Save Instance or \
             Apply to Asset.",
        ),
        (
            "Validate",
            NavigationEditorAction::Validate,
            "Run blueprint validation against runtime requirements.",
        ),
        (
            "Save instance",
            NavigationEditorAction::SaveInstance,
            "Persist override for this placed building only.",
        ),
        (
            "Apply to asset",
            NavigationEditorAction::ApplyToAsset,
            "Update the shared asset-default blueprint for all inheriting instances.",
        ),
        (
            "Reset to asset",
            NavigationEditorAction::ResetToAsset,
            "Remove instance override and revert to asset/generated blueprint.",
        ),
        (
            "Save As Variant",
            NavigationEditorAction::SaveAsVariant,
            "Create a new building definition variant from the working blueprint.",
        ),
        (
            "Create variant",
            NavigationEditorAction::CreateVariant,
            "Commit variant fields and create the definition.",
        ),
        (
            "Confirm",
            NavigationEditorAction::ConfirmPending,
            "Confirm the pending destructive action.",
        ),
        (
            "Cancel",
            NavigationEditorAction::CancelPending,
            "Cancel the pending action.",
        ),
        (
            "Cancel variant",
            NavigationEditorAction::CancelVariant,
            "Discard Save As Variant draft.",
        ),
        (
            "Overlay blueprint",
            NavigationEditorAction::OverlayBlueprint,
            "Toggle debug nav blueprint overlay (diagnostic only).",
        ),
        (
            "Overlay entrances",
            NavigationEditorAction::OverlayEntrances,
            "Toggle debug entrance overlay.",
        ),
        (
            "Overlay runtime path",
            NavigationEditorAction::OverlayRuntimePath,
            "Toggle debug path overlay.",
        ),
    ]
}

fn spawn_action_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: NavigationEditorAction,
    tooltip: &str,
) {
    parent.spawn((
        DevNavigationEditorActionButton { action },
        DevTooltipTarget::from_content(DevTooltipContent::new(tooltip)),
        DevPanelUi,
        DevWindowUi,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(BTN_BG_IDLE),
        Text::new(label),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(Color::srgba(0.88, 0.94, 0.98, 1.0)),
    ));
}

/// Spawn the shared “Open Navigation Editor” launcher (Selected Object + Catalog Editor tab).
pub fn spawn_open_navigation_editor_button(parent: &mut ChildSpawnerCommands, label: &str) {
    parent.spawn((
        DevNavigationEditorOpenButton,
        DevTooltipTarget::new(
            "Open the Navigation Editor for the selected building. Requires a placed building in world selection.",
        ),
        DevPanelUi,
        DevWindowUi,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(BTN_BG_IDLE),
        Text::new(label),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(Color::srgba(0.88, 0.94, 0.98, 1.0)),
    ));
}

pub fn sync_navigation_editor_panel(
    dev_state: Res<crate::dev::DevModeState>,
    registry: Res<DevWindowRegistry>,
    world_selection: Res<WorldSelectionState>,
    inspector: Res<WorldInspectorState>,
    inspection: Res<BlueprintInspectionState>,
    ui_state: Res<NavigationEditorUiState>,
    // Every panel widget below mutates `Text`/`Node` on entities that share the
    // same broad UI markers, so they live in one ParamSet rather than relying on
    // pairwise `Without` filters that grow quadratically with each new row.
    mut panel: bevy::ecs::system::ParamSet<(
        Query<&mut Text, With<DevNavigationEditorSummaryText>>,
        Query<(&mut Text, &mut Node), With<DevNavigationEditorValidationText>>,
        Query<(
            &DevNavigationEditorActionButton,
            &mut Node,
            &mut BackgroundColor,
        )>,
        Query<(&mut Text, &mut Node), With<DevNavigationEditorSourceText>>,
        Query<&mut Node, With<DevNavigationEditorOpacityRow>>,
    )>,
) {
    let visible = dev_state.enabled && registry.is_visible(DevWindowId::NavigationEditor);
    let building_selected = world_selection.category == WorldSelectionCategory::Building
        && world_selection.building_id.is_some();
    let bp = inspector.blueprint_snapshot.as_ref();
    let editing = inspection.editing;
    let pending = inspection.pending_confirmation.is_some();
    let variant = inspection.variant_draft.is_some();

    if let Ok(mut text) = panel.p0().single_mut() {
        **text = if !visible {
            String::new()
        } else if !building_selected {
            "Select a placed building to inspect or edit its navigation blueprint.".into()
        } else if let (Some(snap), Some(building)) = (bp, inspector.building_snapshot.as_ref()) {
            format!(
                "{}\n{}\nInstance #{}  Blueprint: {}\nSource: {}  {}\nFloor: {}  Tool: {:?}\n{}",
                building.display_name,
                building.definition_id.as_str(),
                building.building_id.raw(),
                snap.blueprint_id.as_deref().unwrap_or("-"),
                snap.blueprint_source,
                if inspection.dirty { "UNSAVED" } else { "" },
                snap.selected_floor_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "-".into()),
                inspection.active_tool,
                inspector.last_message,
            )
        } else if let Some(building) = inspector.building_snapshot.as_ref() {
            format!(
                "{}\n{}\nInstance #{} - no blueprint resolved.\n{}",
                building.display_name,
                building.definition_id.as_str(),
                building.building_id.raw(),
                inspector.last_message,
            )
        } else {
            "Building selection is stale - reselect the building.".into()
        };
    }

    let show_validation = visible
        && building_selected
        && ui_state.validation_expanded
        && bp.is_some_and(|s| !s.validation.diagnostics.is_empty());
    if let Ok((mut text, mut node)) = panel.p1().single_mut() {
        node.display = if show_validation {
            Display::Flex
        } else {
            Display::None
        };
        if show_validation {
            let snap = bp.unwrap();
            let mut lines = String::from("Validation:\n");
            for diag in &snap.validation.diagnostics {
                lines.push_str(&format!("- [{:?}] {}\n", diag.level, diag.message));
            }
            **text = lines;
        }
    }

    let show_source = visible
        && building_selected
        && (ui_state.regeneration_source_label.is_some()
            || ui_state.generation_diagnostics.is_some());
    if let Ok((mut text, mut node)) = panel.p3().single_mut() {
        node.display = if show_source {
            Display::Flex
        } else {
            Display::None
        };
        if show_source {
            let mut lines = format!(
                "Regeneration source: {}",
                ui_state.regeneration_source_label.as_deref().unwrap_or("-")
            );
            if let Some(diag) = ui_state.generation_diagnostics.as_ref() {
                lines.push_str(&format!(
                    "\nEntrances generated: {}  Explicit markers: {}  Synthesized: {}  Deduplicated: {}",
                    diag.entrances_generated,
                    diag.explicit_markers,
                    diag.synthesized_entrances,
                    diag.deduplicated_candidates,
                ));
                if !diag.candidate_details.is_empty() {
                    lines.push('\n');
                    lines.push_str(&diag.summary_line());
                }
            }
            **text = lines;
        }
    }

    let show_opacity = navigation_editor_owns_session(dev_state.enabled, &registry, &inspection);
    for mut node in panel.p4().iter_mut() {
        node.display = if show_opacity {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (button, mut node, mut bg) in panel.p2().iter_mut() {
        let show =
            visible
                && building_selected
                && match button.action {
                    NavigationEditorAction::InspectMode => !editing && !pending,
                    NavigationEditorAction::EditMode => !editing && !pending && bp.is_some(),
                    NavigationEditorAction::ExitEdit => editing && !pending,
                    NavigationEditorAction::FloorPrev | NavigationEditorAction::FloorNext => {
                        inspection.active && !pending
                    }
                    NavigationEditorAction::ToolSelect
                    | NavigationEditorAction::ToolAddCorner
                    | NavigationEditorAction::ToolAddEntrance
                    | NavigationEditorAction::DeleteSelection
                    | NavigationEditorAction::RadiusUp
                    | NavigationEditorAction::RadiusDown => editing && !pending && !variant,
                    NavigationEditorAction::FrameBuilding
                    | NavigationEditorAction::ReturnCamera => inspection.active && !pending,
                    NavigationEditorAction::Regenerate => !pending,
                    NavigationEditorAction::Validate => inspection.active && !pending,
                    NavigationEditorAction::SaveInstance
                    | NavigationEditorAction::ApplyToAsset
                    | NavigationEditorAction::ResetToAsset => editing && !pending && !variant,
                    NavigationEditorAction::SaveAsVariant => editing && !pending && !variant,
                    NavigationEditorAction::CreateVariant
                    | NavigationEditorAction::CancelVariant => variant,
                    NavigationEditorAction::ConfirmPending
                    | NavigationEditorAction::CancelPending => pending,
                    NavigationEditorAction::OverlayBlueprint
                    | NavigationEditorAction::OverlayEntrances
                    | NavigationEditorAction::OverlayRuntimePath => inspection.active,
                };
        node.display = if show { Display::Flex } else { Display::None };

        let active_tool = matches!(
            (button.action, &inspection.active_tool),
            (
                NavigationEditorAction::ToolSelect,
                BlueprintEditTool::Select
            ) | (
                NavigationEditorAction::ToolAddCorner,
                BlueprintEditTool::AddVertex
            ) | (
                NavigationEditorAction::ToolAddEntrance,
                BlueprintEditTool::AddEntrance
            )
        );
        *bg = if active_tool {
            BackgroundColor(BTN_BG_ACTIVE)
        } else {
            BackgroundColor(BTN_BG_IDLE)
        };
    }

    let _ = authority_tooltip(bp.map(|s| s.blueprint_source.as_str()).unwrap_or(""));
}

pub fn sync_open_navigation_editor_buttons(
    dev_state: Res<crate::dev::DevModeState>,
    world_selection: Res<WorldSelectionState>,
    mut buttons: Query<&mut Node, With<DevNavigationEditorOpenButton>>,
) {
    let show = dev_state.enabled
        && world_selection.category == WorldSelectionCategory::Building
        && world_selection.building_id.is_some();
    for mut node in &mut buttons {
        node.display = if show { Display::Flex } else { Display::None };
    }
}
