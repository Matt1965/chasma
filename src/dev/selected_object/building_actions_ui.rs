//! Building dev action UI in Selected Object (Slice 12).

use bevy::prelude::*;

use crate::dev::input::DevPanelUi;
use crate::dev::inspector::{BuildingDevAction, DevBuildingActionButton};
use crate::dev::widgets::{
    DevCollapsibleSectionId, spawn_action_button, spawn_collapsible_section,
};
use crate::dev::window::DevWindowUi;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildingDevSectionKind {
    Construction,
    Lifecycle,
    Production,
    Doors,
    Terrain,
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct DevBuildingActionSection {
    pub kind: BuildingDevSectionKind,
}

#[derive(Component, Debug)]
pub(crate) struct DevBuildingActionsRoot;

#[derive(Component, Debug)]
pub(crate) struct DevProductionOperationSelector;

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
            spawn_action_section(
                root,
                BuildingDevSectionKind::Construction,
                DevCollapsibleSectionId::SelectedBuildingConstruction,
                "Construction",
                BuildingDevAction::CONSTRUCTION,
                false,
            );
            spawn_action_section(
                root,
                BuildingDevSectionKind::Lifecycle,
                DevCollapsibleSectionId::SelectedBuildingLifecycle,
                "Lifecycle",
                BuildingDevAction::LIFECYCLE,
                false,
            );
            spawn_action_section(
                root,
                BuildingDevSectionKind::Production,
                DevCollapsibleSectionId::SelectedBuildingProduction,
                "Production",
                BuildingDevAction::PRODUCTION_ACTIONS,
                true,
            );
            spawn_action_section(
                root,
                BuildingDevSectionKind::Doors,
                DevCollapsibleSectionId::SelectedBuildingDoors,
                "Doors",
                BuildingDevAction::DOORS,
                false,
            );
            spawn_action_section(
                root,
                BuildingDevSectionKind::Terrain,
                DevCollapsibleSectionId::SelectedBuildingTerrain,
                "Terrain",
                BuildingDevAction::TERRAIN,
                false,
            );
        });
}

fn spawn_action_section(
    parent: &mut ChildSpawnerCommands<'_>,
    kind: BuildingDevSectionKind,
    section_id: DevCollapsibleSectionId,
    title: &str,
    actions: &[BuildingDevAction],
    include_operation_selector: bool,
) {
    parent
        .spawn((
            DevBuildingActionSection { kind },
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|section| {
            spawn_collapsible_section(section, section_id, title, None, |body| {
                if include_operation_selector {
                    body.spawn((
                        DevProductionOperationSelector,
                        DevPanelUi,
                        Node {
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: Val::Px(4.0),
                            row_gap: Val::Px(4.0),
                            display: Display::None,
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        },
                    ));
                }
                if !actions.is_empty() {
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
                }
            });
        });
}
