//! Player production controls for the owned Building Menu (BP3).

use bevy::prelude::*;

use crate::client::{ClientIntent, ClientIntentQueue};
use crate::ui::gameplay::floating_window::{
    spawn_floating_raised_button, spawn_floating_raised_button_armed, spawn_floating_section_well,
};
use crate::ui::gameplay::text::format_ui_status;
use crate::world::OperationDefinitionId;

use super::content::{
    BuildingPanelProduction, BuildingPanelStorageSettings, BuildingPanelWorkPriority,
};
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

#[derive(Component, Debug, Clone)]
pub struct BuildingStorageCategoryButton {
    pub category_id: crate::world::ItemCategoryId,
    pub target_accepted: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct BuildingStorageAcceptAllButton;

#[derive(Component, Debug, Clone, Copy)]
pub struct BuildingStorageClearAllButton;

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
                super::super::styles::panel_body_font(),
                TextColor(super::super::styles::TEXT_PRIMARY),
            ));
            spawn_priority_button(row, "-", false);
            spawn_priority_button(row, "+", true);
        });
}

fn spawn_priority_button(parent: &mut ChildSpawnerCommands<'_>, label: &str, increase: bool) {
    spawn_floating_raised_button(
        parent,
        BuildingWorkPriorityButton { increase },
        label,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
            ..default()
        },
    );
}

pub fn spawn_storage_controls(
    parent: &mut ChildSpawnerCommands<'_>,
    storage: &BuildingPanelStorageSettings,
) {
    parent
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },))
        .with_children(|section| {
            section.spawn((
                Text::new("Accepted Items"),
                super::super::styles::panel_body_font(),
                TextColor(super::super::styles::TEXT_PRIMARY),
            ));
            spawn_floating_section_well(section, |well| {
                well.spawn((Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                },))
                    .with_children(|row| {
                        spawn_storage_bulk_button(
                            row,
                            "Accept All",
                            BuildingStorageAcceptAllButton,
                        );
                        spawn_storage_bulk_button(row, "Clear All", BuildingStorageClearAllButton);
                    });
                for category in &storage.categories {
                    let label = if category.accepted {
                        format!("[X] {}", category.display_name)
                    } else {
                        format!("[ ] {}", category.display_name)
                    };
                    spawn_floating_raised_button(
                        well,
                        BuildingStorageCategoryButton {
                            category_id: category.category_id.clone(),
                            target_accepted: !category.accepted,
                        },
                        &label,
                        Node {
                            padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                            align_self: AlignSelf::FlexStart,
                            ..default()
                        },
                    );
                }
            });
        });
}

fn spawn_storage_bulk_button<T: Component>(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &str,
    marker: T,
) {
    spawn_floating_raised_button(
        parent,
        marker,
        label,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
            ..default()
        },
    );
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
                super::super::styles::panel_body_font(),
                TextColor(super::super::styles::TEXT_PRIMARY),
            ));
            spawn_production_toggle(section, production.enabled);
            if production.show_operation_selector {
                spawn_operation_selector(section, production);
            } else if let Some(progress) = production.progress_percent {
                section.spawn((
                    Text::new(format_ui_status(
                        &production.operation_name,
                        &format!("{}%", progress),
                    )),
                    super::super::styles::panel_body_font(),
                    TextColor(super::super::styles::TEXT_PRIMARY),
                ));
            } else {
                section.spawn((
                    Text::new(&production.operation_name),
                    super::super::styles::panel_body_font(),
                    TextColor(super::super::styles::TEXT_PRIMARY),
                ));
            }
            if production.show_operation_selector {
                if let Some(progress) = production.progress_percent {
                    section.spawn((
                        Text::new(format!("Progress: {progress}%")),
                        super::super::styles::panel_body_font(),
                        TextColor(super::super::styles::TEXT_MUTED),
                    ));
                }
            }
            if let Some(efficiency) = &production.efficiency_display {
                section.spawn((
                    Text::new(format!("Efficiency: {efficiency}")),
                    super::super::styles::panel_body_font(),
                    TextColor(super::super::styles::TEXT_MUTED),
                ));
            }
            for line in &production.terrain_field_lines {
                section.spawn((
                    Text::new(line),
                    super::super::styles::panel_body_font(),
                    TextColor(super::super::styles::TEXT_MUTED),
                ));
            }
            if let Some(blocked) = &production.blocking_label {
                section.spawn((
                    Text::new(format!("Blocked: {blocked}")),
                    super::super::styles::panel_body_font(),
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
    spawn_floating_raised_button(
        parent,
        BuildingProductionToggleButton {
            target_enabled: !enabled,
        },
        label,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
            align_self: AlignSelf::FlexStart,
            ..default()
        },
    );
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
                let node = Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                    ..default()
                };
                if option.selected {
                    spawn_floating_raised_button_armed(
                        row,
                        BuildingProductionOperationButton {
                            operation: option.operation_id.clone(),
                        },
                        &option.display_name,
                        node,
                    );
                } else {
                    spawn_floating_raised_button(
                        row,
                        BuildingProductionOperationButton {
                            operation: option.operation_id.clone(),
                        },
                        &option.display_name,
                        node,
                    );
                }
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
    storage_category_buttons: Query<
        (&Interaction, &BuildingStorageCategoryButton),
        (Changed<Interaction>, With<BuildingStorageCategoryButton>),
    >,
    storage_accept_all_buttons: Query<
        (&Interaction, &BuildingStorageAcceptAllButton),
        (Changed<Interaction>, With<BuildingStorageAcceptAllButton>),
    >,
    storage_clear_all_buttons: Query<
        (&Interaction, &BuildingStorageClearAllButton),
        (Changed<Interaction>, With<BuildingStorageClearAllButton>),
    >,
) {
    let Some(building_id) = panel.open_building_id else {
        return;
    };

    for (interaction, button) in &storage_accept_all_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        queue.push(ClientIntent::AcceptAllBuildingStorageCategories { building_id });
        return;
    }

    for (interaction, button) in &storage_clear_all_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        queue.push(ClientIntent::ClearAllBuildingStorageCategories { building_id });
        return;
    }

    for (interaction, button) in &storage_category_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        queue.push(ClientIntent::SetBuildingStorageCategoryAccepted {
            building_id,
            category_id: button.category_id.clone(),
            accepted: button.target_accepted,
        });
        return;
    }

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
