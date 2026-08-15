//! Navigation Editor panel sync (IN-10 / IN-10a).

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::debug::{DebugOverlayConfig, NavigationOverlayDiagnostics};
use crate::dev::inspector::{
    BlueprintEditSelection, BlueprintEditTool, BlueprintInspectionState, WorldInspectorState,
    format_adopted_draft_status_message,
};
use crate::dev::widgets::{
    DevButtonChrome, DevButtonKind, DevCollapsibleSection, DevCollapsibleSectionId,
    DevCollapsibleState, DevStatusSeverity, DevWidgetStatusLine,
};
use crate::dev::window::{navigation_editor_panel_width, navigation_editor_uses_two_columns};

use super::capabilities::navigation_editor_capabilities;
use super::commands::authority_tooltip;
use super::panel::{
    DevNavigationEditorActionButton, DevNavigationEditorColumns, DevNavigationEditorContextDetails,
    DevNavigationEditorContextTitle, DevNavigationEditorDeleteButton,
    DevNavigationEditorFloorLabel, DevNavigationEditorFloorSelector,
    DevNavigationEditorGenerationDetailsText, DevNavigationEditorGenerationSummaryText,
    DevNavigationEditorLeftColumn, DevNavigationEditorNavRow, DevNavigationEditorOpacityRow,
    DevNavigationEditorOverlayStatusText, DevNavigationEditorRadiusRow,
    DevNavigationEditorRadiusValueText, DevNavigationEditorRegionIndexText,
    DevNavigationEditorRegionLabel, DevNavigationEditorRegionSelector,
    DevNavigationEditorRightColumn, DevNavigationEditorSectionHeader,
    DevNavigationEditorSelectedItemPanel, DevNavigationEditorSelectedItemText,
    DevNavigationEditorStatusCard, DevNavigationEditorStatusCounts,
    DevNavigationEditorStatusHeadline, DevNavigationEditorToastBanner,
    DevNavigationEditorToastText, DevNavigationEditorValidationText, NavigationEditorAction,
};
use super::selectors::{RegionSeverityHint, floor_selector_state, region_selector_state};
use super::state::{
    NavigationEditorUiState, format_concise_generation_summary, format_generation_details,
    navigation_editor_owns_session, wrap_panel_text,
};

const PANEL_TEXT_WRAP_CHARS: usize = 64;

/// Apply one-shot disclosure hints and mirror persistent generation expansion.
pub fn apply_navigation_editor_disclosure_hints(
    mut ui_state: ResMut<NavigationEditorUiState>,
    mut collapsible: ResMut<DevCollapsibleState>,
) {
    if ui_state.validation_expanded {
        collapsible.set_expanded(DevCollapsibleSectionId::NavEditorValidation, true);
        ui_state.validation_expanded = false;
    }
}

/// Keep generation-details expansion mirrored in UI state (stable across repaint).
pub fn sync_navigation_editor_disclosure_state(
    collapsible: Res<DevCollapsibleState>,
    mut ui_state: ResMut<NavigationEditorUiState>,
    toggles: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<crate::dev::widgets::DevCollapsibleToggleButton>,
        ),
    >,
) {
    for interaction in &toggles {
        if *interaction == Interaction::Pressed {
            ui_state.generation_details_expanded =
                collapsible.is_expanded(DevCollapsibleSectionId::NavEditorGeneration);
        }
    }
    if !ui_state.generation_details_expanded {
        ui_state.generation_details_expanded =
            collapsible.is_expanded(DevCollapsibleSectionId::NavEditorGeneration);
    }
}

/// Advance toast auto-dismiss and mirror inspector messages into the banner.
pub fn sync_navigation_editor_toast(
    time: Res<Time>,
    dev_state: Res<crate::dev::DevModeState>,
    registry: Res<crate::dev::window::DevWindowRegistry>,
    world_selection: Res<WorldSelectionState>,
    inspector: Res<WorldInspectorState>,
    mut ui_state: ResMut<NavigationEditorUiState>,
    mut banner: Query<&mut Node, With<DevNavigationEditorToastBanner>>,
    mut toast_text: Query<
        (&mut Text, &mut DevWidgetStatusLine),
        With<DevNavigationEditorToastText>,
    >,
) {
    let visible =
        dev_state.enabled && registry.is_visible(crate::dev::window::DevWindowId::NavigationEditor);
    let building_selected = world_selection.category == WorldSelectionCategory::Building
        && world_selection.building_id.is_some();
    let now = time.elapsed_secs();

    if visible && building_selected {
        let severity = infer_message_severity(inspector.last_message.as_str());
        ui_state.sync_toast_from_message(inspector.last_message.as_str(), severity, now);
    }

    if let Some(toast) = ui_state.toast.as_ref() {
        if toast.is_expired(now) {
            ui_state.toast = None;
        }
    }

    let show = visible
        && building_selected
        && ui_state
            .toast
            .as_ref()
            .is_some_and(|t| !t.message.is_empty());
    for mut node in &mut banner {
        node.display = if show { Display::Flex } else { Display::None };
    }
    if let Ok((mut text, mut line)) = toast_text.single_mut() {
        if let Some(toast) = ui_state.toast.as_ref() {
            **text = toast.message.clone();
            line.severity = toast.severity;
        } else {
            **text = String::new();
            line.severity = DevStatusSeverity::Info;
        }
    }
}

/// Responsive two-column body layout.
pub fn sync_navigation_editor_responsive_layout(
    registry: Res<crate::dev::window::DevWindowRegistry>,
    mut columns: Query<&mut Node, With<DevNavigationEditorColumns>>,
    mut left: Query<
        &mut Node,
        (
            With<DevNavigationEditorLeftColumn>,
            Without<DevNavigationEditorColumns>,
        ),
    >,
    mut right: Query<
        &mut Node,
        (
            With<DevNavigationEditorRightColumn>,
            Without<DevNavigationEditorColumns>,
            Without<DevNavigationEditorLeftColumn>,
        ),
    >,
) {
    let panel_width = navigation_editor_panel_width(registry.viewport);
    let two_column = navigation_editor_uses_two_columns(panel_width);
    for mut node in &mut columns {
        node.flex_direction = if two_column {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        };
    }
    let basis = if two_column {
        Val::Percent(50.0)
    } else {
        Val::Percent(100.0)
    };
    for mut node in left.iter_mut().chain(right.iter_mut()) {
        node.flex_basis = basis;
    }
}

/// Regions/entrances in a blueprint, for authoring-vs-runtime comparison.
fn blueprint_topology(blueprint: &crate::world::BuildingNavigationBlueprint) -> (usize, usize) {
    let regions = blueprint
        .floors
        .iter()
        .map(|floor| floor.regions.len())
        .sum();
    (regions, blueprint.entrances.len())
}

/// Authoring, persisted, and runtime topology side by side (IN-11b).
///
/// The blueprint overlay draws authoring data while the navigation overlay draws
/// runtime data, so saved geometry alone never proved that runtime spaces exist.
/// Showing both makes that divergence readable instead of invisible.
fn authoring_vs_runtime_lines(
    world: &crate::world::WorldData,
    inspection: &BlueprintInspectionState,
    snapshot: Option<&crate::dev::inspector::BuildingBlueprintInspectorSnapshot>,
    building_id: crate::world::BuildingId,
) -> String {
    let working = inspection.working_copy.as_ref().map(blueprint_topology);
    let persisted = snapshot
        .and_then(|snap| snap.resolved_blueprint.as_ref())
        .map(blueprint_topology);
    let (runtime_regions, runtime_portals) = world
        .building_navigation_runtime()
        .get(building_id)
        .map(|runtime| (runtime.regions.len(), runtime.portal_keys.len()))
        .unwrap_or((0, 0));

    let mut lines = String::new();
    if let Some((regions, entrances)) = working {
        let differs = persisted.is_some_and(|persisted| persisted != (regions, entrances));
        lines.push_str(&format!(
            "Working  {regions} regions · {entrances} entrances{}\n",
            if differs { "  (differs)" } else { "" }
        ));
    }
    if let Some((regions, entrances)) = persisted {
        lines.push_str(&format!(
            "Persisted  {regions} regions · {entrances} entrances\n"
        ));
    }
    let diverged = persisted.is_some_and(|(regions, _)| regions != runtime_regions);
    lines.push_str(&format!(
        "Runtime  {runtime_regions} regions · {runtime_portals} portals{}\n",
        if diverged { "  (not active)" } else { "" }
    ));
    match world.interior_activation_outcomes().get(building_id) {
        Some(outcome) => lines.push_str(&format!("Activation  {}", outcome.status.label())),
        None => lines.push_str("Activation  not evaluated"),
    }
    lines
}

/// Sync building context, draft status, and collapsible detail text.
pub fn sync_navigation_editor_panel_content(
    dev_state: Res<crate::dev::DevModeState>,
    registry: Res<crate::dev::window::DevWindowRegistry>,
    world_selection: Res<WorldSelectionState>,
    inspector: Res<WorldInspectorState>,
    inspection: Res<BlueprintInspectionState>,
    ui_state: Res<NavigationEditorUiState>,
    world: Res<crate::world::WorldData>,
    mut texts: ParamSet<(
        Query<&mut Text, With<DevNavigationEditorContextTitle>>,
        Query<&mut Text, With<DevNavigationEditorContextDetails>>,
        Query<&mut Text, With<DevNavigationEditorStatusHeadline>>,
        Query<&mut Text, With<DevNavigationEditorStatusCounts>>,
        Query<&mut Text, With<DevNavigationEditorGenerationDetailsText>>,
        Query<&mut Text, With<DevNavigationEditorValidationText>>,
        Query<&mut Text, With<DevNavigationEditorGenerationSummaryText>>,
    )>,
    mut status_card: Query<&mut Node, With<DevNavigationEditorStatusCard>>,
    mut generation_summary_node: Query<
        &mut Node,
        (
            With<DevNavigationEditorGenerationSummaryText>,
            Without<DevNavigationEditorStatusCard>,
        ),
    >,
) {
    let visible =
        dev_state.enabled && registry.is_visible(crate::dev::window::DevWindowId::NavigationEditor);
    let building_selected = world_selection.category == WorldSelectionCategory::Building
        && world_selection.building_id.is_some();
    let bp = inspector.blueprint_snapshot.as_ref();
    let editing = inspection.editing;
    let draft_invalid = inspection
        .generated_draft
        .as_ref()
        .is_some_and(|draft| !draft.validation.valid());

    if let Ok(mut title) = texts.p0().single_mut() {
        **title = if !visible || !building_selected {
            "Select a placed building".into()
        } else if let Some(building) = inspector.building_snapshot.as_ref() {
            building.display_name.clone()
        } else {
            "Building selection stale".into()
        };
    }

    if let Ok(mut details) = texts.p1().single_mut() {
        **details = if !visible || !building_selected {
            "Open the Navigation Editor after selecting a placed building.".into()
        } else if let Some(building) = inspector.building_snapshot.as_ref() {
            let blueprint = bp
                .and_then(|snap| snap.blueprint_id.as_deref())
                .unwrap_or("None");
            let source = bp
                .map(|s| s.blueprint_source.to_string())
                .unwrap_or_else(|| "-".into());
            let dirty = if inspection.dirty { "Unsaved" } else { "Saved" };
            format!(
                "Instance  #{}\nDefinition  {}\nBlueprint  {}\nSource  {}\nMode  {}  ·  {}\n{}",
                building.building_id.raw(),
                building.definition_id.as_str(),
                blueprint,
                source,
                if editing {
                    "Editing"
                } else if inspection.active {
                    "Inspecting"
                } else {
                    "Idle"
                },
                dirty,
                authoring_vs_runtime_lines(&world, &inspection, bp, building.building_id),
            )
        } else {
            "Reselect the building.".into()
        };
    }

    let show_status = visible
        && building_selected
        && (inspection.has_pending_generated_draft()
            || inspection.is_editing_adopted_draft()
            || bp.is_some());
    for mut node in status_card.iter_mut() {
        node.display = if show_status {
            Display::Flex
        } else {
            Display::None
        };
    }

    if let Ok(mut headline) = texts.p2().single_mut() {
        **headline = if !show_status {
            String::new()
        } else if inspection.is_editing_adopted_draft() {
            "EDITING ADOPTED DRAFT".into()
        } else if inspection.has_pending_generated_draft() {
            if draft_invalid {
                "GENERATED DRAFT (INVALID)".into()
            } else {
                "GENERATED DRAFT".into()
            }
        } else if let Some(snap) = bp {
            if snap.validation.valid() {
                "WORKING COPY".into()
            } else {
                "WORKING COPY (INVALID)".into()
            }
        } else {
            "NO BLUEPRINT".into()
        };
    }

    if let Ok(mut counts) = texts.p3().single_mut() {
        **counts = if !show_status {
            String::new()
        } else {
            let (regions, connections) = if inspection.has_pending_generated_draft() {
                inspection.draft_topology_summary().unwrap_or((0, 0))
            } else {
                inspection.working_topology_summary().unwrap_or((0, 0))
            };
            let validation = if inspection.has_pending_generated_draft() {
                inspection.generated_draft.as_ref().map(|d| &d.validation)
            } else {
                bp.map(|s| &s.validation)
            };
            let errors = validation.map(|v| v.error_count).unwrap_or(0);
            let warnings = validation.map(|v| v.warning_count).unwrap_or(0);
            let source = ui_state
                .regeneration_source_label
                .as_deref()
                .or_else(|| bp.map(|s| s.blueprint_source.as_str()))
                .unwrap_or("-");
            let build = ui_state
                .generation_diagnostics
                .as_ref()
                .map(|d| d.regeneration_source.as_str())
                .filter(|s| !s.is_empty());
            let mut line = format!(
                "{regions} regions · {connections} connections\n{errors} errors · {warnings} warnings\nSource {source}"
            );
            if let Some(build) = build {
                line.push_str(&format!("\nBuild {build}"));
            }
            if inspection.is_editing_adopted_draft() {
                let (wr, wc) = inspection.working_topology_summary().unwrap_or((0, 0));
                let validation = bp.map(|snap| snap.validation.clone()).unwrap_or_default();
                line = format_adopted_draft_status_message(&validation, wr, wc);
            }
            wrap_panel_text(&line, PANEL_TEXT_WRAP_CHARS)
        };
    }

    let show_generation_summary = visible
        && building_selected
        && (ui_state.regeneration_source_label.is_some()
            || ui_state.generation_diagnostics.is_some());
    for mut node in generation_summary_node.iter_mut() {
        node.display = if show_generation_summary {
            Display::Flex
        } else {
            Display::None
        };
    }
    if show_generation_summary {
        if let Ok(mut text) = texts.p6().single_mut() {
            let (region_count, connection_count) = if inspection.has_pending_generated_draft() {
                inspection.draft_topology_summary().unwrap_or((0, 0))
            } else {
                inspection.working_topology_summary().unwrap_or((0, 0))
            };
            let error_count = inspection
                .generated_draft
                .as_ref()
                .map(|draft| draft.validation.error_count)
                .or_else(|| bp.map(|snap| snap.validation.error_count))
                .unwrap_or(0);
            **text = wrap_panel_text(
                &format_concise_generation_summary(
                    ui_state.regeneration_source_label.as_deref(),
                    region_count,
                    connection_count,
                    error_count,
                ),
                PANEL_TEXT_WRAP_CHARS,
            );
        }
    }

    if let Ok(mut text) = texts.p4().single_mut() {
        **text = wrap_panel_text(
            &format_generation_details(
                ui_state.regeneration_source_label.as_deref(),
                ui_state.generation_diagnostics.as_ref(),
            ),
            PANEL_TEXT_WRAP_CHARS,
        );
    }

    if let Ok(mut text) = texts.p5().single_mut() {
        let diagnostics = if inspection.is_editing_adopted_draft() || inspection.editing {
            bp.map(|snap| snap.validation.diagnostics.as_slice())
        } else if inspection.has_pending_generated_draft() {
            inspection
                .generated_draft
                .as_ref()
                .map(|draft| draft.validation.diagnostics.as_slice())
        } else {
            bp.map(|snap| snap.validation.diagnostics.as_slice())
        };
        **text = if let Some(diags) = diagnostics {
            if diags.is_empty() {
                String::new()
            } else {
                let mut lines = String::from("Validation:\n");
                for diag in diags {
                    lines.push_str(&format!(
                        "- [{:?}] {}\n",
                        diag.level,
                        wrap_panel_text(&diag.message, PANEL_TEXT_WRAP_CHARS)
                    ));
                }
                lines
            }
        } else {
            String::new()
        };
    }

    let _ = authority_tooltip(bp.map(|s| s.blueprint_source.as_str()).unwrap_or(""));
}

/// Sync overlay diagnostic status lines.
pub fn sync_navigation_editor_overlay_status(
    dev_state: Res<crate::dev::DevModeState>,
    registry: Res<crate::dev::window::DevWindowRegistry>,
    overlay_diagnostics: Res<NavigationOverlayDiagnostics>,
    mut text: Query<&mut Text, With<DevNavigationEditorOverlayStatusText>>,
) {
    let visible =
        dev_state.enabled && registry.is_visible(crate::dev::window::DevWindowId::NavigationEditor);
    if let Ok(mut label) = text.single_mut() {
        if !visible {
            **label = String::new();
            return;
        }
        let mut lines = Vec::new();
        if !overlay_diagnostics.authored_blueprint.is_empty() {
            lines.push(overlay_diagnostics.authored_blueprint.clone());
        }
        if !overlay_diagnostics.runtime_entrances.is_empty() {
            lines.push(overlay_diagnostics.runtime_entrances.clone());
        }
        if !overlay_diagnostics.navigation_authority.is_empty() {
            lines.push(overlay_diagnostics.navigation_authority.clone());
        }
        if !overlay_diagnostics.authored_runtime_summary.is_empty()
            && (dev_state.debug_config.nav_blueprint && dev_state.debug_config.nav_entrances)
        {
            lines.push(overlay_diagnostics.authored_runtime_summary.clone());
        }
        **label = lines.join("\n");
    }
}

/// Sync selectors, selection panel, feedback line, and action buttons.
pub fn sync_navigation_editor_panel(
    dev_state: Res<crate::dev::DevModeState>,
    registry: Res<crate::dev::window::DevWindowRegistry>,
    world_selection: Res<WorldSelectionState>,
    inspector: Res<WorldInspectorState>,
    inspection: Res<BlueprintInspectionState>,
    mut texts: ParamSet<(
        Query<&mut Text, With<DevNavigationEditorFloorLabel>>,
        Query<&mut Text, With<DevNavigationEditorRegionLabel>>,
        Query<&mut Text, With<DevNavigationEditorRegionIndexText>>,
        Query<&mut Text, With<DevNavigationEditorSelectedItemText>>,
        Query<&mut Text, With<DevNavigationEditorRadiusValueText>>,
    )>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<DevNavigationEditorNavRow>>,
        Query<&mut Node, With<DevNavigationEditorFloorSelector>>,
        Query<&mut Node, With<DevNavigationEditorRegionSelector>>,
        Query<&mut Node, With<DevNavigationEditorSelectedItemPanel>>,
        Query<&mut Node, With<DevNavigationEditorOpacityRow>>,
        Query<&mut Node, With<DevNavigationEditorRadiusRow>>,
    )>,
) {
    let visible =
        dev_state.enabled && registry.is_visible(crate::dev::window::DevWindowId::NavigationEditor);
    let building_selected = world_selection.category == WorldSelectionCategory::Building
        && world_selection.building_id.is_some();
    let bp = inspector.blueprint_snapshot.as_ref();
    let editing = inspection.editing;
    let pending = inspection.pending_confirmation.is_some();
    let variant = inspection.variant_draft.is_some();
    let draft_invalid = inspection
        .generated_draft
        .as_ref()
        .is_some_and(|draft| !draft.validation.valid());
    let _ = draft_invalid;
    let working_copy_valid = bp.is_some_and(|snap| snap.validation.valid());

    let floor_state = bp.and_then(|snap| {
        floor_selector_state(
            &snap.floor_ids,
            inspection.selected_floor_id,
            snap.selected_floor_elevation,
        )
    });

    let region_rows = regions_on_active_floor(&inspection);
    let region_severity = region_validation_hint(
        bp,
        inspection.selected_floor_id,
        inspection.selected_region_key.as_deref(),
    );
    let region_state = region_selector_state(
        &region_rows,
        inspection.selected_region_key.as_deref(),
        region_severity,
    );

    let show_floor = visible && building_selected && inspection.active && !pending;
    let show_region = visible && building_selected && editing && !pending && !variant;
    for mut node in nodes.p0().iter_mut() {
        node.display = if show_floor || show_region {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut node in nodes.p1().iter_mut() {
        node.display = if show_floor {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut text) = texts.p0().single_mut() {
        **text = floor_state
            .as_ref()
            .map(|s| s.label_line.clone())
            .unwrap_or_else(|| "-".into());
    }

    for mut node in nodes.p2().iter_mut() {
        node.display = if show_region {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut text) = texts.p1().single_mut() {
        **text = region_state.region_label.clone();
    }
    if let Ok(mut text) = texts.p2().single_mut() {
        **text = if region_state.total == 0 {
            String::new()
        } else {
            format!(
                "{} of {} · {}",
                region_state.index_one_based, region_state.total, region_state.region_key
            )
        };
    }

    let caps = navigation_editor_capabilities(&inspection);
    let show_selected = show_region;
    for mut node in nodes.p3().iter_mut() {
        node.display = if show_selected {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut text) = texts.p3().single_mut() {
        **text = if !show_region {
            String::new()
        } else if caps.detail_lines.is_empty() {
            caps.guidance
                .clone()
                .unwrap_or_else(|| "Nothing selected".into())
        } else {
            let mut lines = caps.detail_lines.join("\n");
            if let Some(reason) = caps.delete_reason.as_ref().filter(|_| !caps.delete_enabled) {
                lines.push_str("\n");
                lines.push_str(reason);
            } else if let Some(guidance) = &caps.guidance {
                lines.push_str("\n");
                lines.push_str(guidance);
            }
            lines
        };
    }

    if let Ok(mut value) = texts.p4().single_mut() {
        **value = caps
            .radius_meters
            .map(|r| format!("Radius: {r:.2} m"))
            .unwrap_or_default();
    }

    for mut node in nodes.p5().iter_mut() {
        node.display = if show_region && caps.radius_visible {
            Display::Flex
        } else {
            Display::None
        };
    }

    let show_opacity = navigation_editor_owns_session(dev_state.enabled, &registry, &inspection);
    for mut node in nodes.p4().iter_mut() {
        node.display = if show_opacity {
            Display::Flex
        } else {
            Display::None
        };
    }
}

/// Sync action button visibility, labels, and enabled state (separate from label text queries).
pub fn sync_navigation_editor_action_buttons(
    dev_state: Res<crate::dev::DevModeState>,
    registry: Res<crate::dev::window::DevWindowRegistry>,
    world_selection: Res<WorldSelectionState>,
    inspector: Res<WorldInspectorState>,
    inspection: Res<BlueprintInspectionState>,
    mut buttons: Query<(
        &mut DevNavigationEditorActionButton,
        &mut DevButtonChrome,
        &mut Node,
        &mut Text,
    )>,
) {
    let visible =
        dev_state.enabled && registry.is_visible(crate::dev::window::DevWindowId::NavigationEditor);
    let building_selected = world_selection.category == WorldSelectionCategory::Building
        && world_selection.building_id.is_some();
    let bp = inspector.blueprint_snapshot.as_ref();
    let editing = inspection.editing;
    let pending = inspection.pending_confirmation.is_some();
    let variant = inspection.variant_draft.is_some();
    let working_copy_valid = bp.is_some_and(|snap| snap.validation.valid());
    let show_region = visible && building_selected && editing && !pending && !variant;

    let floor_state = bp.and_then(|snap| {
        floor_selector_state(
            &snap.floor_ids,
            inspection.selected_floor_id,
            snap.selected_floor_elevation,
        )
    });
    let region_rows = regions_on_active_floor(&inspection);
    let region_severity = region_validation_hint(
        bp,
        inspection.selected_floor_id,
        inspection.selected_region_key.as_deref(),
    );
    let region_state = region_selector_state(
        &region_rows,
        inspection.selected_region_key.as_deref(),
        region_severity,
    );
    let caps = navigation_editor_capabilities(&inspection);
    let region_count = region_rows.len();
    let has_region_target = inspection.selected_region_key.is_some() || region_count > 0;

    for (mut button, mut chrome, mut node, mut label) in &mut buttons {
        let show = nav_action_visible(
            button.action,
            visible,
            building_selected,
            bp.is_some(),
            editing,
            pending,
            variant,
            inspection.active,
            inspection.has_pending_generated_draft(),
        );
        node.display = if show { Display::Flex } else { Display::None };

        if button.action == NavigationEditorAction::Regenerate {
            **label = if inspection.has_pending_generated_draft() {
                "Regenerate draft".to_string()
            } else {
                "Generate draft".to_string()
            };
        }
        if button.action == NavigationEditorAction::DeleteSelection {
            **label = caps.delete_label.to_string();
            node.display = if show_region && caps.delete_visible {
                Display::Flex
            } else {
                Display::None
            };
        }

        let mut disabled = false;
        disabled |= button.action == NavigationEditorAction::FloorPrev
            && floor_state.as_ref().is_some_and(|s| !s.can_prev);
        disabled |= button.action == NavigationEditorAction::FloorNext
            && floor_state.as_ref().is_some_and(|s| !s.can_next);
        disabled |=
            button.action == NavigationEditorAction::SelectRegionPrev && !region_state.can_prev;
        disabled |=
            button.action == NavigationEditorAction::SelectRegionNext && !region_state.can_next;
        disabled |= button.action == NavigationEditorAction::ToolAddConnection && region_count < 2;
        disabled |= button.action == NavigationEditorAction::ToolAddCorner && !has_region_target;
        disabled |= (button.action == NavigationEditorAction::SaveInstance
            || button.action == NavigationEditorAction::ApplyToAsset)
            && !working_copy_valid;
        disabled |= button.action == NavigationEditorAction::DeleteSelection
            && (!caps.delete_visible || !caps.delete_enabled);
        disabled |= (button.action == NavigationEditorAction::RadiusUp
            || button.action == NavigationEditorAction::RadiusDown)
            && (!caps.radius_visible
                || !caps.radius_decrement_enabled
                    && button.action == NavigationEditorAction::RadiusDown);
        disabled |= button.action == NavigationEditorAction::ReplaceWorkingCopy
            && !inspection.has_pending_generated_draft();
        button.disabled = disabled;
        chrome.disabled = disabled;
        chrome.active = nav_action_active(button.action, &inspection, &dev_state.debug_config);
        chrome.kind = nav_action_kind(button.action);
    }
}

fn regions_on_active_floor(inspection: &BlueprintInspectionState) -> Vec<(String, Option<String>)> {
    let Some(floor_id) = inspection.selected_floor_id else {
        return Vec::new();
    };
    let Some(blueprint) = inspection.working_copy.as_ref() else {
        return Vec::new();
    };
    blueprint
        .floors
        .iter()
        .find(|floor| floor.floor_id == floor_id)
        .map(|floor| {
            floor
                .regions
                .iter()
                .map(|region| (region.key.clone(), region.room_tag.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn region_validation_hint(
    bp: Option<&crate::dev::inspector::BuildingBlueprintInspectorSnapshot>,
    floor_id: Option<i32>,
    region_key: Option<&str>,
) -> RegionSeverityHint {
    let Some(snap) = bp else {
        return RegionSeverityHint::None;
    };
    let Some(floor_id) = floor_id else {
        return RegionSeverityHint::None;
    };
    let Some(region_key) = region_key else {
        return RegionSeverityHint::None;
    };
    let mut hint = RegionSeverityHint::None;
    for diag in &snap.validation.diagnostics {
        let msg = diag.message.to_lowercase();
        if !msg.contains(&region_key.to_lowercase()) {
            continue;
        }
        if msg.contains(&format!("floor {floor_id}")) || msg.contains(region_key) {
            hint = match diag.level {
                crate::world::BlueprintDiagnosticLevel::Error => RegionSeverityHint::Error,
                crate::world::BlueprintDiagnosticLevel::Warning => {
                    if hint == RegionSeverityHint::Error {
                        hint
                    } else {
                        RegionSeverityHint::Warning
                    }
                }
                _ => hint,
            };
        }
    }
    hint
}

pub fn infer_message_severity(message: &str) -> DevStatusSeverity {
    let lower = message.to_lowercase();
    if lower.contains("complete")
        || lower.contains("saved")
        || lower.contains("added")
        || lower.contains("accepted")
        || lower.contains("enabled")
        || lower.contains("discarded")
    {
        DevStatusSeverity::Success
    } else if lower.contains("blocked")
        || lower.contains("cannot")
        || lower.contains("invalid")
        || message_contains_actionable_failure(&lower)
        || lower.contains("error")
    {
        DevStatusSeverity::Error
    } else if lower.contains("confirm") || lower.contains("unsaved") || lower.contains("warning") {
        DevStatusSeverity::Warning
    } else {
        DevStatusSeverity::Info
    }
}

/// Propagation summaries end with `failed N`; only treat as error when N > 0.
fn message_contains_actionable_failure(lower: &str) -> bool {
    if !lower.contains("failed") {
        return false;
    }
    if lower.contains("failed 0") {
        return false;
    }
    true
}

fn nav_action_kind(action: NavigationEditorAction) -> DevButtonKind {
    match action {
        NavigationEditorAction::SaveInstance
        | NavigationEditorAction::ConfirmPending
        | NavigationEditorAction::CreateVariant => DevButtonKind::Primary,
        NavigationEditorAction::DiscardDraft
        | NavigationEditorAction::ResetToAsset
        | NavigationEditorAction::DeleteSelection
        | NavigationEditorAction::ReplaceWorkingCopy => DevButtonKind::Destructive,
        NavigationEditorAction::ApplyToAsset
        | NavigationEditorAction::EditDraft
        | NavigationEditorAction::Regenerate
        | NavigationEditorAction::SaveAsVariant
        | NavigationEditorAction::Validate
        | NavigationEditorAction::InspectMode
        | NavigationEditorAction::EditMode
        | NavigationEditorAction::ExitEdit
        | NavigationEditorAction::FrameBuilding
        | NavigationEditorAction::ReturnCamera
        | NavigationEditorAction::ToolAddRegion => DevButtonKind::Secondary,
        _ => DevButtonKind::Normal,
    }
}

fn nav_action_active(
    action: NavigationEditorAction,
    inspection: &BlueprintInspectionState,
    debug: &DebugOverlayConfig,
) -> bool {
    match action {
        NavigationEditorAction::ToolSelect => inspection.active_tool == BlueprintEditTool::Select,
        NavigationEditorAction::ToolAddCorner => {
            inspection.active_tool == BlueprintEditTool::AddVertex
        }
        NavigationEditorAction::ToolAddEntrance => {
            inspection.active_tool == BlueprintEditTool::AddEntrance
        }
        NavigationEditorAction::ToolAddConnection => {
            inspection.active_tool == BlueprintEditTool::AddConnection
        }
        NavigationEditorAction::ToggleDraftPreview => inspection.draft_preview_active,
        NavigationEditorAction::OverlayBlueprint => debug.nav_blueprint,
        NavigationEditorAction::OverlayEntrances => debug.nav_entrances,
        NavigationEditorAction::OverlayBlockedArea => debug.nav_blockers,
        NavigationEditorAction::ClearRecordedPath => false,
        NavigationEditorAction::ReplaceWorkingCopy => false,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn nav_action_visible(
    action: NavigationEditorAction,
    visible: bool,
    building_selected: bool,
    has_blueprint: bool,
    editing: bool,
    pending: bool,
    variant: bool,
    inspection_active: bool,
    has_draft: bool,
) -> bool {
    if !(visible && building_selected) {
        return false;
    }
    match action {
        NavigationEditorAction::InspectMode => !editing && !pending,
        NavigationEditorAction::EditMode => !editing && !pending && has_blueprint,
        NavigationEditorAction::ExitEdit => editing && !pending,
        NavigationEditorAction::FloorPrev | NavigationEditorAction::FloorNext => {
            inspection_active && !pending
        }
        NavigationEditorAction::ToolSelect
        | NavigationEditorAction::ToolAddCorner
        | NavigationEditorAction::ToolAddEntrance
        | NavigationEditorAction::ToolAddConnection
        | NavigationEditorAction::SelectRegionPrev
        | NavigationEditorAction::SelectRegionNext
        | NavigationEditorAction::DeleteSelection
        | NavigationEditorAction::RadiusUp
        | NavigationEditorAction::RadiusDown => editing && !pending && !variant,
        NavigationEditorAction::ToolAddRegion => editing && !pending && !variant,
        NavigationEditorAction::FrameBuilding | NavigationEditorAction::ReturnCamera => {
            inspection_active && !pending
        }
        NavigationEditorAction::Regenerate => !pending,
        NavigationEditorAction::EditDraft => has_draft && !pending,
        NavigationEditorAction::ReplaceWorkingCopy => has_draft && !pending,
        NavigationEditorAction::AcceptDraft => false,
        NavigationEditorAction::DiscardDraft | NavigationEditorAction::ToggleDraftPreview => {
            has_draft && !pending
        }
        NavigationEditorAction::Validate => inspection_active && !pending,
        NavigationEditorAction::SaveInstance | NavigationEditorAction::ApplyToAsset => {
            editing && !pending && !variant
        }
        NavigationEditorAction::ResetToAsset | NavigationEditorAction::SaveAsVariant => {
            editing && !pending && !variant
        }
        NavigationEditorAction::CreateVariant | NavigationEditorAction::CancelVariant => variant,
        NavigationEditorAction::ConfirmPending | NavigationEditorAction::CancelPending => pending,
        NavigationEditorAction::OverlayBlueprint
        | NavigationEditorAction::OverlayEntrances
        | NavigationEditorAction::OverlayBlockedArea
        | NavigationEditorAction::ClearRecordedPath => visible && building_selected,
    }
}

/// Hide collapsible section headers when their content is not relevant.
pub fn sync_navigation_editor_section_visibility(
    dev_state: Res<crate::dev::DevModeState>,
    registry: Res<crate::dev::window::DevWindowRegistry>,
    world_selection: Res<WorldSelectionState>,
    inspector: Res<WorldInspectorState>,
    inspection: Res<BlueprintInspectionState>,
    ui_state: Res<NavigationEditorUiState>,
    mut sections: Query<
        (&DevCollapsibleSection, &mut Node),
        Without<DevNavigationEditorSectionHeader>,
    >,
    mut headers: Query<&mut Node, With<DevNavigationEditorSectionHeader>>,
) {
    let visible =
        dev_state.enabled && registry.is_visible(crate::dev::window::DevWindowId::NavigationEditor);
    let building_selected = world_selection.category == WorldSelectionCategory::Building
        && world_selection.building_id.is_some();
    let bp = inspector.blueprint_snapshot.as_ref();
    let show_generation_summary = visible
        && building_selected
        && (ui_state.regeneration_source_label.is_some()
            || ui_state.generation_diagnostics.is_some());
    let validation_diag_count = if inspection.has_pending_generated_draft() {
        inspection
            .generated_draft
            .as_ref()
            .map(|draft| draft.validation.diagnostics.len())
            .unwrap_or(0)
    } else {
        bp.map(|snap| snap.validation.diagnostics.len())
            .unwrap_or(0)
    };

    for (section, mut node) in &mut sections {
        let show_section = visible
            && building_selected
            && match section.id {
                DevCollapsibleSectionId::NavEditorGeneration => {
                    show_generation_summary || ui_state.generation_details_expanded
                }
                DevCollapsibleSectionId::NavEditorValidation => validation_diag_count > 0,
                _ => true,
            };
        node.display = if show_section {
            Display::Flex
        } else {
            Display::None
        };
    }

    for mut node in headers.iter_mut() {
        node.display = if visible && building_selected {
            Display::Flex
        } else {
            Display::None
        };
    }
}
