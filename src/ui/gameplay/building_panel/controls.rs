//! Player production controls for the owned Building Menu (BP3).

use bevy::prelude::*;

use crate::client::{ClientIntent, ClientIntentQueue};
use crate::world::OperationDefinitionId;

use super::content::{BuildingPanelProduction, BuildingPanelWorkPriority};
use super::state::BuildingPanelState;

#[derive(Component, Debug, Clone, Copy)]
pub struct BuildingWorkPriorityButton {
    pub increase: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct BuildingProductionToggleButton {
    pub target_enabled: bool,
}

#[derive(Component, Debug, Clone)]
pub struct BuildingProductionOperationButton {
    pub operation: OperationDefinitionId,
}

pub fn spawn_work_priority_controls(
    parent: &mut ChildSpawnerCommands<'_>,
    work_priority: &BuildingPanelWorkPriority,
) {
    parent
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(4.0),
            ..default()
        },))
        .with_children(|row| {
            row.spawn((
                Text::new(format!("Priority: {}", work_priority.label)),
                super::super::styles::hud_body_font(),
                TextColor(super::super::styles::TEXT_PRIMARY),
            ));
            spawn_priority_button(row, "-", false);
            spawn_priority_button(row, "+", true);
        });
}

fn spawn_priority_button(parent: &mut ChildSpawnerCommands<'_>, label: &str, increase: bool) {
    parent
        .spawn((
            BuildingWorkPriorityButton { increase },
            Button,
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(super::super::styles::CMD_BTN_ENABLED_BG),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                super::super::styles::hud_body_font(),
                TextColor(super::super::styles::TEXT_PRIMARY),
            ));
        });
}

pub fn spawn_production_controls(
    parent: &mut ChildSpawnerCommands<'_>,
    production: &BuildingPanelProduction,
) {
    parent
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },))
        .with_children(|section| {
            section.spawn((
                Text::new("Production"),
                super::super::styles::hud_body_font(),
                TextColor(super::super::styles::TEXT_PRIMARY),
            ));
            spawn_production_toggle(section, production.enabled);
            if production.show_operation_selector {
                spawn_operation_selector(section, production);
            } else if let Some(progress) = production.progress_percent {
                section.spawn((
                    Text::new(format!("{} — {}%", production.operation_name, progress)),
                    super::super::styles::hud_body_font(),
                    TextColor(super::super::styles::TEXT_PRIMARY),
                ));
            } else {
                section.spawn((
                    Text::new(&production.operation_name),
                    super::super::styles::hud_body_font(),
                    TextColor(super::super::styles::TEXT_PRIMARY),
                ));
            }
            if production.show_operation_selector {
                if let Some(progress) = production.progress_percent {
                    section.spawn((
                        Text::new(format!("Progress: {progress}%")),
                        super::super::styles::hud_body_font(),
                        TextColor(super::super::styles::TEXT_MUTED),
                    ));
                }
            }
            if let Some(efficiency) = &production.efficiency_display {
                section.spawn((
                    Text::new(format!("Efficiency: {efficiency}")),
                    super::super::styles::hud_body_font(),
                    TextColor(super::super::styles::TEXT_MUTED),
                ));
            }
            if let Some(blocked) = &production.blocking_label {
                section.spawn((
                    Text::new(format!("Blocked: {blocked}")),
                    super::super::styles::hud_body_font(),
                    TextColor(super::super::styles::TEXT_MUTED),
                ));
            }
        });
}

fn spawn_production_toggle(parent: &mut ChildSpawnerCommands<'_>, enabled: bool) {
    let label = if enabled {
        "Production: Enabled"
    } else {
        "Production: Disabled"
    };
    parent
        .spawn((
            BuildingProductionToggleButton {
                target_enabled: !enabled,
            },
            Button,
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                align_self: AlignSelf::FlexStart,
                ..default()
            },
            BackgroundColor(super::super::styles::CMD_BTN_ENABLED_BG),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                super::super::styles::hud_body_font(),
                TextColor(super::super::styles::TEXT_PRIMARY),
            ));
        });
}

fn spawn_operation_selector(
    parent: &mut ChildSpawnerCommands<'_>,
    production: &BuildingPanelProduction,
) {
    parent
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        },))
        .with_children(|row| {
            for option in &production.operation_options {
                let armed = option.selected;
                row.spawn((
                    BuildingProductionOperationButton {
                        operation: option.operation_id.clone(),
                    },
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(if armed {
                        super::super::styles::CMD_BTN_ARMED_BG
                    } else {
                        super::super::styles::CMD_BTN_ENABLED_BG
                    }),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new(&option.display_name),
                        super::super::styles::hud_body_font(),
                        TextColor(super::super::styles::TEXT_PRIMARY),
                    ));
                });
            }
        });
}

pub fn handle_building_production_controls(
    panel: Res<BuildingPanelState>,
    mut queue: ResMut<ClientIntentQueue>,
    toggle_buttons: Query<
        (&Interaction, &BuildingProductionToggleButton),
        (Changed<Interaction>, With<BuildingProductionToggleButton>),
    >,
    operation_buttons: Query<
        (&Interaction, &BuildingProductionOperationButton),
        Changed<Interaction>,
    >,
    priority_buttons: Query<
        (&Interaction, &BuildingWorkPriorityButton),
        (Changed<Interaction>, With<BuildingWorkPriorityButton>),
    >,
) {
    let Some(building_id) = panel.open_building_id else {
        return;
    };

    for (interaction, button) in &priority_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        queue.push(ClientIntent::AdjustBuildingWorkPriority {
            building_id,
            increase: button.increase,
        });
        return;
    }

    for (interaction, button) in &toggle_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        queue.push(ClientIntent::SetBuildingProductionEnabled {
            building_id,
            enabled: button.target_enabled,
        });
        return;
    }

    for (interaction, button) in &operation_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        queue.push(ClientIntent::SetBuildingProductionOperation {
            building_id,
            operation: button.operation.clone(),
        });
        return;
    }
}
