//! Settlement Workforce floating panel (BP5).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::client::CameraSettlementContext;
use crate::ui::gameplay::floating_window::{
    FloatingGameplayWindowId, FloatingGameplayWindowRoot, FloatingWindowTitleBarDragRegion,
    TITLE_BAR_HEIGHT_PX,
};
use crate::ui::gameplay::layout::PlayerHudUi;
use crate::ui::gameplay::styles::{
    BAR_BG, CMD_BTN_ENABLED_BG, TEXT_MUTED, TEXT_PRIMARY, hud_body_font, hud_title_font,
};
use crate::world::{
    SettlementId, UnitCatalog, UnitId, WorkPermissionDomain, WorkSkillCatalog, WorldData,
    allow_all_unit_work_permissions, deny_all_unit_work_permissions, set_unit_work_permission,
};

use super::content::{
    SettlementWorkforceSnapshot, build_settlement_workforce_snapshot, permission_column_labels,
};
use super::state::SettlementWorkforcePanelState;

#[derive(Component, Debug)]
pub struct SettlementWorkforcePanelRoot;

#[derive(Component, Debug)]
pub struct SettlementWorkforcePanelCloseButton;

#[derive(Component, Debug)]
pub struct SettlementWorkforcePanelTitleText;

#[derive(Component, Debug)]
pub struct SettlementWorkforceMatrixBody;

#[derive(Component, Debug)]
pub struct SettlementWorkforceEmptyText;

#[derive(Component, Debug, Clone, Copy)]
pub struct WorkforcePermissionCheckbox {
    pub settlement_id: SettlementId,
    pub unit_id: UnitId,
    pub domain: WorkPermissionDomain,
    pub target_allowed: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct WorkforceClearAllButton {
    pub settlement_id: SettlementId,
    pub unit_id: UnitId,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct WorkforceAllowAllButton {
    pub settlement_id: SettlementId,
    pub unit_id: UnitId,
}

pub fn spawn_settlement_workforce_panel(mut commands: Commands) {
    commands
        .spawn((
            SettlementWorkforcePanelRoot,
            FloatingGameplayWindowRoot {
                id: FloatingGameplayWindowId::SettlementWorkforce,
            },
            PlayerHudUi,
            Button,
            Interaction::None,
            FocusPolicy::Block,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(640.0),
                height: Val::Percent(62.0),
                max_width: Val::Percent(92.0),
                max_height: Val::Percent(75.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(6.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(BAR_BG),
            ZIndex(411),
        ))
        .with_children(|root| {
            root.spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|header| {
                header
                    .spawn((
                        FloatingWindowTitleBarDragRegion {
                            id: FloatingGameplayWindowId::SettlementWorkforce,
                        },
                        Button,
                        Node {
                            flex_grow: 1.0,
                            min_height: Val::Px(TITLE_BAR_HEIGHT_PX),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                    ))
                    .with_children(|title| {
                        title.spawn((
                            SettlementWorkforcePanelTitleText,
                            Text::new("Settlement Workforce"),
                            hud_title_font(),
                            TextColor(TEXT_PRIMARY),
                        ));
                    });
                header.spawn((
                    SettlementWorkforcePanelCloseButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                        ..default()
                    },
                    Text::new("×"),
                    hud_title_font(),
                    TextColor(TEXT_MUTED),
                ));
            });
            root.spawn((
                SettlementWorkforceMatrixBody,
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    overflow: Overflow::scroll(),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
            ));
        });
}

pub fn sync_settlement_workforce_panel_visibility(
    panel: Res<SettlementWorkforcePanelState>,
    mut roots: Query<&mut Node, With<SettlementWorkforcePanelRoot>>,
) {
    let display = if panel.open {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut roots {
        node.display = display;
    }
}

pub fn sync_settlement_workforce_panel(
    panel: Res<SettlementWorkforcePanelState>,
    context: Res<CameraSettlementContext>,
    world: Res<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    work_skill_catalog: Res<WorkSkillCatalog>,
    mut commands: Commands,
    mut cache: Local<Option<SettlementWorkforceSnapshot>>,
    mut title: Query<
        &mut Text,
        (
            With<SettlementWorkforcePanelTitleText>,
            Without<SettlementWorkforceEmptyText>,
        ),
    >,
    matrix_body: Query<(Entity, &Children), With<SettlementWorkforceMatrixBody>>,
) {
    if !panel.open {
        *cache = None;
        return;
    }

    let snapshot =
        build_settlement_workforce_snapshot(&context, &world, &unit_catalog, &work_skill_catalog);
    if cache.as_ref() == Some(&snapshot) {
        return;
    }
    *cache = Some(snapshot.clone());

    if let Ok(mut text) = title.single_mut() {
        **text = snapshot.title.clone();
    }

    let Ok((body_entity, _children)) = matrix_body.single() else {
        return;
    };
    commands.entity(body_entity).despawn_related::<Children>();

    commands.entity(body_entity).with_children(|body| {
        if let Some(message) = &snapshot.empty_message {
            body.spawn((
                SettlementWorkforceEmptyText,
                Text::new(message.clone()),
                hud_body_font(),
                TextColor(TEXT_MUTED),
            ));
            return;
        }
        spawn_matrix_table(body, &snapshot);
    });
}

fn spawn_matrix_table(
    parent: &mut ChildSpawnerCommands<'_>,
    snapshot: &SettlementWorkforceSnapshot,
) {
    let Some(settlement_id) = snapshot.settlement_id else {
        return;
    };

    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            min_width: Val::Px(720.0),
            ..default()
        })
        .with_children(|table| {
            spawn_matrix_header_row(table, snapshot);
            for row in &snapshot.rows {
                spawn_matrix_data_row(table, settlement_id, row);
            }
        });
}

fn spawn_matrix_header_row(
    parent: &mut ChildSpawnerCommands<'_>,
    snapshot: &SettlementWorkforceSnapshot,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            padding: UiRect::vertical(Val::Px(4.0)),
            border: UiRect::bottom(Val::Px(1.0)),
            ..default()
        })
        .with_children(|row| {
            spawn_header_cell(row, "Unit", 120.0);
            for label in permission_column_labels(snapshot) {
                spawn_header_cell(row, label, 84.0);
            }
            spawn_header_cell(row, "Controls", 120.0);
        });
}

fn spawn_header_cell(parent: &mut ChildSpawnerCommands<'_>, label: &str, width: f32) {
    parent.spawn((
        Text::new(label),
        hud_body_font(),
        TextColor(TEXT_MUTED),
        Node {
            width: Val::Px(width),
            ..default()
        },
    ));
}

fn spawn_matrix_data_row(
    parent: &mut ChildSpawnerCommands<'_>,
    settlement_id: SettlementId,
    row: &super::content::WorkforceMatrixRow,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            align_items: AlignItems::Center,
            padding: UiRect::vertical(Val::Px(2.0)),
            ..default()
        })
        .with_children(|line| {
            line.spawn((
                Text::new(row.display_name.clone()),
                hud_body_font(),
                TextColor(TEXT_PRIMARY),
                Node {
                    width: Val::Px(120.0),
                    ..default()
                },
            ));
            for cell in &row.cells {
                spawn_permission_cell(line, settlement_id, row.unit_id, cell);
            }
            spawn_row_controls(line, settlement_id, row.unit_id);
        });
}

fn spawn_permission_cell(
    parent: &mut ChildSpawnerCommands<'_>,
    settlement_id: SettlementId,
    unit_id: UnitId,
    cell: &super::content::WorkforceMatrixCell,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            align_items: AlignItems::Center,
            width: Val::Px(84.0),
            ..default()
        })
        .with_children(|cell_row| {
            cell_row.spawn((
                Text::new(cell.skill_value.to_string()),
                hud_body_font(),
                TextColor(TEXT_PRIMARY),
            ));
            if cell.physically_capable == Some(false) {
                cell_row.spawn((
                    Text::new("Incapable"),
                    hud_body_font(),
                    TextColor(TEXT_MUTED),
                ));
                return;
            }
            let label = if cell.permission_allowed {
                "[✓]"
            } else {
                "[ ]"
            };
            cell_row
                .spawn((
                    WorkforcePermissionCheckbox {
                        settlement_id,
                        unit_id,
                        domain: cell.domain,
                        target_allowed: !cell.permission_allowed,
                    },
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(CMD_BTN_ENABLED_BG),
                ))
                .with_children(|button| {
                    button.spawn((Text::new(label), hud_body_font(), TextColor(TEXT_PRIMARY)));
                });
        });
}

fn spawn_row_controls(
    parent: &mut ChildSpawnerCommands<'_>,
    settlement_id: SettlementId,
    unit_id: UnitId,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            width: Val::Px(120.0),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|controls| {
            spawn_row_action_button(
                controls,
                "Clear All",
                WorkforceClearAllButton {
                    settlement_id,
                    unit_id,
                },
            );
            spawn_row_action_button(
                controls,
                "Allow All",
                WorkforceAllowAllButton {
                    settlement_id,
                    unit_id,
                },
            );
        });
}

fn spawn_row_action_button<C: Component>(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &str,
    marker: C,
) {
    parent
        .spawn((
            marker,
            Button,
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(CMD_BTN_ENABLED_BG),
        ))
        .with_children(|button| {
            button.spawn((Text::new(label), hud_body_font(), TextColor(TEXT_PRIMARY)));
        });
}

pub fn handle_settlement_workforce_close_button(
    mut panel: ResMut<SettlementWorkforcePanelState>,
    buttons: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<SettlementWorkforcePanelCloseButton>,
        ),
    >,
) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            panel.close();
        }
    }
}

pub fn handle_settlement_workforce_controls(
    context: Res<CameraSettlementContext>,
    mut world: ResMut<WorldData>,
    checkboxes: Query<
        (&Interaction, &WorkforcePermissionCheckbox),
        (Changed<Interaction>, With<WorkforcePermissionCheckbox>),
    >,
    clear_buttons: Query<
        (&Interaction, &WorkforceClearAllButton),
        (Changed<Interaction>, With<WorkforceClearAllButton>),
    >,
    allow_buttons: Query<
        (&Interaction, &WorkforceAllowAllButton),
        (Changed<Interaction>, With<WorkforceAllowAllButton>),
    >,
) {
    let focused = context.focused_settlement_id;
    for (interaction, checkbox) in &checkboxes {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if focused != Some(checkbox.settlement_id) {
            continue;
        }
        let _ = set_unit_work_permission(
            &mut world,
            checkbox.settlement_id,
            checkbox.unit_id,
            checkbox.domain,
            checkbox.target_allowed,
        );
    }

    for (interaction, button) in &clear_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if focused != Some(button.settlement_id) {
            continue;
        }
        let _ = deny_all_unit_work_permissions(&mut world, button.settlement_id, button.unit_id);
    }

    for (interaction, button) in &allow_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if focused != Some(button.settlement_id) {
            continue;
        }
        let _ = allow_all_unit_work_permissions(&mut world, button.settlement_id, button.unit_id);
    }
}
