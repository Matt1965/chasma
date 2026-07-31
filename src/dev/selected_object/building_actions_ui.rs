//! Building dev action UI in Selected Object (Slice 12).

use bevy::prelude::*;

use crate::dev::input::DevPanelUi;
use crate::dev::inspector::{BuildingDevAction, DevBuildingActionButton};
use crate::dev::tooltip::DevTooltipTarget;
use crate::dev::widgets::{
    DevCollapsibleSectionId, spawn_action_button, spawn_collapsible_section,
};
use crate::dev::window::DevWindowUi;

#[derive(Component, Debug)]
pub(crate) struct DevBuildingActionsRoot;

pub fn spawn_building_dev_actions(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevBuildingActionsRoot,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_action_group(root, "Construction", BuildingDevAction::CONSTRUCTION);
            spawn_action_group(root, "Lifecycle", BuildingDevAction::LIFECYCLE);
            spawn_action_group(root, "Production", BuildingDevAction::PRODUCTION);
            spawn_action_group(root, "Inventory", BuildingDevAction::INVENTORY);
            spawn_action_group(root, "Logistics", BuildingDevAction::LOGISTICS);
            spawn_action_group(root, "Doors", BuildingDevAction::DOORS);
            spawn_action_group(root, "Terrain", BuildingDevAction::TERRAIN);
        });
}

fn spawn_action_group(
    parent: &mut ChildSpawnerCommands<'_>,
    title: &str,
    actions: &[BuildingDevAction],
) {
    let section_id = match title {
        "Construction" => DevCollapsibleSectionId::SelectedBuildingConstruction,
        "Lifecycle" => DevCollapsibleSectionId::SelectedBuildingLifecycle,
        "Production" => DevCollapsibleSectionId::SelectedBuildingProduction,
        "Inventory" => DevCollapsibleSectionId::SelectedBuildingInventory,
        "Logistics" => DevCollapsibleSectionId::SelectedBuildingLogistics,
        _ => DevCollapsibleSectionId::SelectedBuildingDiagnostics,
    };
    spawn_collapsible_section(parent, section_id, title, None, |body| {
        body.spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .with_children(|row| {
            for action in actions {
                spawn_action_button(
                    row,
                    action.label(),
                    Some(action.tooltip()),
                    DevBuildingActionButton { action: *action },
                );
            }
        });
    });
}
