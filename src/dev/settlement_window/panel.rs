//! Settlement Dev window panel (camera-derived context).

use bevy::prelude::*;

use crate::client::CameraSettlementContext;
use crate::dev::dev_mode::DevModeState;
use crate::dev::input::DevPanelUi;
use crate::dev::settlement_placement::DevSettlementPlacementButton;
use crate::dev::tooltip::DevTooltipContent;
use crate::dev::widgets::{
    DevWidgetActionButton, DevWidgetToggle, spawn_action_button, spawn_toggle_row,
    sync_action_button_styles, sync_toggle_styles_with_marker,
};
use crate::dev::window::{DevWindowBody, DevWindowId, DevWindowRegistry, DevWindowUi};
use crate::units::input::SelectedUnits;
use crate::world::WorldData;

use super::model::{
    SettlementDevSummary, build_settlement_dev_summary, format_ai_line, format_focused_line,
};

#[derive(Component, Debug)]
pub(crate) struct DevSettlementWindowUi;

#[derive(Component, Debug)]
pub(crate) struct DevSettlementFocusedText;

#[derive(Component, Debug)]
pub(crate) struct DevSettlementUnitsText;

#[derive(Component, Debug)]
pub(crate) struct DevSettlementBuildingsText;

#[derive(Component, Debug)]
pub(crate) struct DevSettlementAiText;

#[derive(Component, Debug)]
pub(crate) struct DevSettlementStatusText;

#[derive(Component, Debug, Clone, Copy)]
pub struct DevSettlementAddUnitsButton;

#[derive(Component, Debug, Clone, Copy)]
pub struct DevSettlementAiToggle;

pub fn sync_dev_settlement_panel_visibility(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut visibility: Query<&mut Visibility, With<DevSettlementWindowUi>>,
) {
    let visible = dev_state.enabled && registry.is_visible(DevWindowId::Settlement);
    for mut vis in &mut visibility {
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

pub fn setup_settlement_window_panel(
    mut commands: Commands,
    bodies: Query<(Entity, &DevWindowBody)>,
) {
    for (entity, body) in &bodies {
        if body.id != DevWindowId::Settlement {
            continue;
        }
        commands.entity(entity).with_children(|panel| {
            panel
                .spawn((
                    DevSettlementWindowUi,
                    DevPanelUi,
                    DevWindowUi,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        ..default()
                    },
                ))
                .with_children(|root| {
                    root.spawn((
                        DevPanelUi,
                        Text::new("Settlement"),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.8, 0.88, 0.95, 1.0)),
                    ));
                    root.spawn((
                        DevSettlementFocusedText,
                        DevPanelUi,
                        Text::new("Focused: No focused settlement"),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.75, 0.85, 0.92, 1.0)),
                    ));
                    root.spawn((
                        DevSettlementUnitsText,
                        DevPanelUi,
                        Text::new("Units: —"),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.75, 0.85, 0.92, 1.0)),
                    ));
                    root.spawn((
                        DevSettlementBuildingsText,
                        DevPanelUi,
                        Text::new("Buildings: —"),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.75, 0.85, 0.92, 1.0)),
                    ));
                    root.spawn((
                        DevSettlementAiText,
                        DevPanelUi,
                        Text::new("AI: —"),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.75, 0.85, 0.92, 1.0)),
                    ));
                    spawn_action_button(
                        root,
                        "Place Settlement Anchor",
                        Some(
                            "Arm settlement anchor placement — left-click terrain to create a \
                             settlement; right-click or Escape to cancel",
                        ),
                        DevSettlementPlacementButton,
                    );
                    spawn_action_button(
                        root,
                        "Add Selected Units",
                        Some(
                            "Assign currently selected gameplay units to the camera-focused \
                             settlement",
                        ),
                        DevSettlementAddUnitsButton,
                    );
                    spawn_toggle_row(
                        root,
                        "Settlement AI",
                        DevTooltipContent::new(
                            "Toggles settlement automation_enabled policy on the focused \
                             settlement (authoritative SA1 policy gate)",
                        ),
                        DevSettlementAiToggle,
                    );
                    root.spawn((
                        DevSettlementStatusText,
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

pub fn sync_settlement_dev_panel(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    world: Res<WorldData>,
    context: Res<CameraSettlementContext>,
    mut focused: Query<&mut Text, With<DevSettlementFocusedText>>,
    mut units: Query<
        &mut Text,
        (
            With<DevSettlementUnitsText>,
            Without<DevSettlementFocusedText>,
        ),
    >,
    mut buildings: Query<
        &mut Text,
        (
            With<DevSettlementBuildingsText>,
            Without<DevSettlementFocusedText>,
            Without<DevSettlementUnitsText>,
        ),
    >,
    mut ai: Query<
        &mut Text,
        (
            With<DevSettlementAiText>,
            Without<DevSettlementFocusedText>,
            Without<DevSettlementUnitsText>,
            Without<DevSettlementBuildingsText>,
        ),
    >,
    mut status: Query<
        &mut Text,
        (
            With<DevSettlementStatusText>,
            Without<DevSettlementFocusedText>,
            Without<DevSettlementUnitsText>,
            Without<DevSettlementBuildingsText>,
            Without<DevSettlementAiText>,
        ),
    >,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Settlement) {
        return;
    }
    let summary = build_settlement_dev_summary(&world, &context);
    apply_summary_text(&summary, &mut focused, &mut units, &mut buildings, &mut ai);
    if let Ok(mut text) = status.single_mut() {
        **text = dev_state.settlement_placement_message.clone();
    }
}

fn apply_summary_text(
    summary: &SettlementDevSummary,
    focused: &mut Query<&mut Text, With<DevSettlementFocusedText>>,
    units: &mut Query<
        &mut Text,
        (
            With<DevSettlementUnitsText>,
            Without<DevSettlementFocusedText>,
        ),
    >,
    buildings: &mut Query<
        &mut Text,
        (
            With<DevSettlementBuildingsText>,
            Without<DevSettlementFocusedText>,
            Without<DevSettlementUnitsText>,
        ),
    >,
    ai: &mut Query<
        &mut Text,
        (
            With<DevSettlementAiText>,
            Without<DevSettlementFocusedText>,
            Without<DevSettlementUnitsText>,
            Without<DevSettlementBuildingsText>,
        ),
    >,
) {
    if let Ok(mut text) = focused.single_mut() {
        **text = format_focused_line(summary);
    }
    if let Ok(mut text) = units.single_mut() {
        **text = if summary.focused {
            format!("Units: {}", summary.unit_count)
        } else {
            "Units: —".into()
        };
    }
    if let Ok(mut text) = buildings.single_mut() {
        **text = if summary.focused {
            format!("Buildings: {}", summary.building_count)
        } else {
            "Buildings: —".into()
        };
    }
    if let Ok(mut text) = ai.single_mut() {
        **text = format_ai_line(summary);
    }
}

pub fn sync_settlement_dev_action_availability(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    context: Res<CameraSettlementContext>,
    selected_units: Res<SelectedUnits>,
    mut add_buttons: Query<
        &mut DevWidgetActionButton,
        (
            With<DevSettlementAddUnitsButton>,
            Without<DevSettlementPlacementButton>,
        ),
    >,
    mut ai_toggles: Query<
        &mut DevWidgetToggle,
        (
            With<DevSettlementAiToggle>,
            Without<DevSettlementAddUnitsButton>,
            Without<DevSettlementPlacementButton>,
        ),
    >,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Settlement) {
        return;
    }
    let has_focus = context.focused_settlement_id.is_some();
    let has_units = !selected_units.0.is_empty();
    for mut button in &mut add_buttons {
        button.disabled = !has_focus || !has_units;
    }
    for mut toggle in &mut ai_toggles {
        toggle.disabled = !has_focus;
    }
}

pub fn sync_settlement_dev_button_styles(
    mut add_buttons: Query<
        (
            &DevWidgetActionButton,
            &mut crate::dev::widgets::DevButtonChrome,
        ),
        With<Button>,
    >,
) {
    sync_action_button_styles(add_buttons);
}

pub fn sync_settlement_ai_toggle_styles(
    world: Res<WorldData>,
    context: Res<CameraSettlementContext>,
    mut toggles: Query<
        (
            &Interaction,
            &DevWidgetToggle,
            &DevSettlementAiToggle,
            &mut BackgroundColor,
            &Children,
        ),
        Without<DevSettlementAddUnitsButton>,
    >,
    mut marks: Query<&mut Visibility, With<crate::dev::widgets::DevWidgetToggleMark>>,
) {
    let enabled = context
        .focused_settlement_id
        .and_then(|id| world.settlement_state_store().get(id))
        .map(|state| state.policies.automation_enabled)
        .unwrap_or(false);
    sync_toggle_styles_with_marker(|_: &DevSettlementAiToggle| enabled, toggles, marks);
}
