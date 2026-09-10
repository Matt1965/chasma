//! Settlement Workforce floating panel (BP5).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::ui::{RelativeCursorPosition, ScrollPosition};

use crate::client::CameraSettlementContext;
use crate::ui::gameplay::floating_window::{
    FloatingGameplayWindowId, FloatingGameplayWindowRegistry, FloatingGameplayWindowRoot,
    floating_row_separator_border, floating_window_shell_colors, floating_window_shell_node,
    spawn_floating_raised_button, spawn_floating_title_rail, spawn_floating_window_inner_frame,
};
use crate::ui::gameplay::layout::PlayerHudUi;
use crate::ui::gameplay::styles::{TEXT_MUTED, TEXT_PRIMARY, panel_body_font, panel_title_font};
use crate::world::{
    SettlementId, UnitCatalog, UnitId, WorkPermissionDomain, WorkSkillCatalog, WorldData,
    allow_all_unit_work_permissions, deny_all_unit_work_permissions, set_unit_work_permission,
};

use super::content::{
    SettlementWorkforceSnapshot, build_settlement_workforce_snapshot, permission_column_labels,
};
use super::layout::{
    CLOSE_BUTTON_LABEL, COL_CONTROLS_WIDTH, COL_GAP, COL_UNIT_WIDTH, PANEL_MIN_WIDTH_PX,
    PANEL_WIDTH_PX, matrix_min_width, permission_checkbox_label, permission_col_width,
};
use super::scroll::{
    WORKFORCE_SCROLLBAR_MIN_THUMB_PX, WORKFORCE_SCROLLBAR_TRACK_WIDTH_PX,
    reset_settlement_workforce_scroll,
};
use super::state::SettlementWorkforcePanelState;

const PANEL_HEIGHT_VIEWPORT_FRACTION: f32 = 0.62;
const PANEL_MIN_HEIGHT_PX: f32 = 240.0;
const PANEL_MAX_HEIGHT_VIEWPORT_FRACTION: f32 = 0.75;

#[derive(Component, Debug)]
pub struct SettlementWorkforcePanelRoot;

#[derive(Component, Debug)]
pub struct SettlementWorkforcePanelCloseButton;

#[derive(Component, Debug)]
pub struct SettlementWorkforcePanelTitleText;

/// Matrix region below the title bar (contains horizontal + vertical scroll viewports).
#[derive(Component, Debug)]
pub struct SettlementWorkforceMatrixBody;

/// Horizontal scroll viewport — header and row body share one horizontal scroll position.
#[derive(Component, Debug)]
pub struct SettlementWorkforceMatrixHorizontalScroll;

/// Grid frame holding matrix content and the vertical scrollbar.
#[derive(Component, Debug)]
pub struct SettlementWorkforceMatrixFrame;

/// Fixed column header row host (does not scroll vertically with worker rows).
#[derive(Component, Debug)]
pub struct SettlementWorkforceMatrixHeaderHost;

/// Vertical scroll viewport for worker rows only.
#[derive(Component, Debug)]
pub struct SettlementWorkforceMatrixRowsScroll;

/// Visible vertical scrollbar track for the worker viewport.
#[derive(Component, Debug)]
pub struct SettlementWorkforceVerticalScrollbar;

/// Draggable thumb inside the workforce vertical scrollbar track.
#[derive(Component, Debug)]
pub struct SettlementWorkforceVerticalScrollbarThumb;

/// Stable host for worker data rows — content is rebuilt here without replacing scroll viewports.
#[derive(Component, Debug)]
pub struct SettlementWorkforceMatrixContentHost;

#[derive(Component, Debug)]
pub struct SettlementWorkforceEmptyText;

/// Marker on the matrix column header row.
#[derive(Component, Debug)]
pub struct SettlementWorkforceMatrixHeaderRow;

/// Marker on each worker data row (not the header row).
#[derive(Component, Debug, Clone, Copy)]
pub struct SettlementWorkforceMatrixDataRow {
    pub unit_id: UnitId,
}

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
    let (shell_bg, shell_border) = floating_window_shell_colors();
    let mut shell_node = floating_window_shell_node();
    shell_node.width = Val::Px(PANEL_WIDTH_PX);
    shell_node.min_width = Val::Px(PANEL_MIN_WIDTH_PX);
    shell_node.max_width = Val::Percent(92.0);
    shell_node.height = Val::Px(480.0);
    shell_node.min_height = Val::Px(PANEL_MIN_HEIGHT_PX);
    shell_node.max_height = Val::Px(720.0);

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
            shell_node,
            shell_bg,
            shell_border,
            ZIndex(411),
        ))
        .with_children(|root| {
            spawn_floating_window_inner_frame(root, |frame| {
                spawn_floating_title_rail(
                    frame,
                    FloatingGameplayWindowId::SettlementWorkforce,
                    |title| {
                        title.spawn((
                            SettlementWorkforcePanelTitleText,
                            Text::new("Settlement Workforce"),
                            panel_title_font(),
                            TextColor(TEXT_PRIMARY),
                        ));
                    },
                    Some((SettlementWorkforcePanelCloseButton, CLOSE_BUTTON_LABEL)),
                );
                frame
                    .spawn((
                        SettlementWorkforceMatrixBody,
                        Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            min_height: Val::Px(0.0),
                            width: Val::Percent(100.0),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                    ))
                    .with_children(|body| {
                        body.spawn((
                            SettlementWorkforceMatrixHorizontalScroll,
                            Node {
                                flex_direction: FlexDirection::Column,
                                overflow: Overflow::scroll_x(),
                                flex_grow: 1.0,
                                min_height: Val::Px(0.0),
                                width: Val::Percent(100.0),
                                ..default()
                            },
                        ))
                        .with_children(|horizontal| {
                            horizontal
                                .spawn((
                                    SettlementWorkforceMatrixFrame,
                                    Node {
                                        display: Display::Grid,
                                        flex_grow: 1.0,
                                        min_height: Val::Px(0.0),
                                        min_width: Val::Px(matrix_min_width()),
                                        width: Val::Percent(100.0),
                                        grid_template_columns: vec![
                                            RepeatedGridTrack::flex(1, 1.0),
                                            RepeatedGridTrack::auto(1),
                                        ],
                                        grid_template_rows: vec![RepeatedGridTrack::flex(1, 1.0)],
                                        column_gap: Val::Px(4.0),
                                        ..default()
                                    },
                                ))
                                .with_children(|matrix_frame| {
                                    matrix_frame
                                        .spawn((Node {
                                            display: Display::Grid,
                                            grid_column: GridPlacement::start(1),
                                            grid_row: GridPlacement::start(1),
                                            grid_template_columns: vec![RepeatedGridTrack::flex(
                                                1, 1.0,
                                            )],
                                            grid_template_rows: vec![
                                                RepeatedGridTrack::auto(1),
                                                RepeatedGridTrack::flex(1, 1.0),
                                            ],
                                            min_height: Val::Px(0.0),
                                            row_gap: Val::Px(2.0),
                                            ..default()
                                        },))
                                        .with_children(|rows_column| {
                                            rows_column.spawn((
                                                SettlementWorkforceMatrixHeaderHost,
                                                Node {
                                                    flex_direction: FlexDirection::Column,
                                                    flex_shrink: 0.0,
                                                    ..default()
                                                },
                                            ));
                                            rows_column
                                                .spawn((
                                                    SettlementWorkforceMatrixRowsScroll,
                                                    Button,
                                                    FocusPolicy::Block,
                                                    ScrollPosition::default(),
                                                    Node {
                                                        flex_direction: FlexDirection::Column,
                                                        overflow: Overflow::scroll_y(),
                                                        flex_grow: 1.0,
                                                        min_height: Val::Px(0.0),
                                                        width: Val::Percent(100.0),
                                                        ..default()
                                                    },
                                                ))
                                                .with_children(|rows_scroll| {
                                                    rows_scroll.spawn((
                                                        SettlementWorkforceMatrixContentHost,
                                                        Node {
                                                            flex_direction: FlexDirection::Column,
                                                            row_gap: Val::Px(2.0),
                                                            width: Val::Percent(100.0),
                                                            ..default()
                                                        },
                                                    ));
                                                });
                                        });

                                    matrix_frame
                                        .spawn((
                                            SettlementWorkforceVerticalScrollbar,
                                            Button,
                                            RelativeCursorPosition::default(),
                                            Visibility::Hidden,
                                            Node {
                                                grid_column: GridPlacement::start(2),
                                                grid_row: GridPlacement::start(1),
                                                min_width: Val::Px(
                                                    WORKFORCE_SCROLLBAR_TRACK_WIDTH_PX,
                                                ),
                                                width: Val::Px(WORKFORCE_SCROLLBAR_TRACK_WIDTH_PX),
                                                min_height: Val::Px(0.0),
                                                height: Val::Percent(100.0),
                                                position_type: PositionType::Relative,
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgba(0.35, 0.35, 0.38, 0.85)),
                                        ))
                                        .with_children(|track| {
                                            track.spawn((
                                                SettlementWorkforceVerticalScrollbarThumb,
                                                Node {
                                                    position_type: PositionType::Absolute,
                                                    left: Val::Px(0.0),
                                                    right: Val::Px(0.0),
                                                    border_radius: BorderRadius::all(Val::Px(3.0)),
                                                    ..default()
                                                },
                                                BackgroundColor(Color::srgba(
                                                    0.72, 0.72, 0.76, 1.0,
                                                )),
                                            ));
                                        });
                                });
                        });
                    });
            });
        });
}

pub fn sync_settlement_workforce_panel_dimensions(
    panel: Res<SettlementWorkforcePanelState>,
    registry: Res<FloatingGameplayWindowRegistry>,
    mut roots: Query<&mut Node, With<SettlementWorkforcePanelRoot>>,
) {
    if !panel.open {
        return;
    }
    let viewport_h = registry.viewport.y.max(1.0);
    let height_px = (viewport_h * PANEL_HEIGHT_VIEWPORT_FRACTION).clamp(
        PANEL_MIN_HEIGHT_PX,
        viewport_h * PANEL_MAX_HEIGHT_VIEWPORT_FRACTION,
    );
    for mut node in &mut roots {
        node.height = Val::Px(height_px);
        node.max_height = Val::Px(viewport_h * PANEL_MAX_HEIGHT_VIEWPORT_FRACTION);
    }
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
    mut scroll_state: ResMut<super::scroll::SettlementWorkforceScrollState>,
    mut cache: Local<Option<(SettlementWorkforceSnapshot, u64)>>,
    mut title: Query<
        &mut Text,
        (
            With<SettlementWorkforcePanelTitleText>,
            Without<SettlementWorkforceEmptyText>,
        ),
    >,
    header_hosts: Query<Entity, With<SettlementWorkforceMatrixHeaderHost>>,
    content_hosts: Query<Entity, With<SettlementWorkforceMatrixContentHost>>,
    empty_text: Query<(), With<SettlementWorkforceEmptyText>>,
    header_rows: Query<(), With<SettlementWorkforceMatrixHeaderRow>>,
    data_rows: Query<(), With<SettlementWorkforceMatrixDataRow>>,
) {
    if !panel.open {
        *cache = None;
        reset_settlement_workforce_scroll(&mut scroll_state);
        return;
    }

    let snapshot =
        build_settlement_workforce_snapshot(&context, &world, &unit_catalog, &work_skill_catalog);

    let materialized_rows = data_rows.iter().count();
    let host_ready = if content_hosts.single().is_ok() && header_hosts.single().is_ok() {
        if snapshot.empty_message.is_some() {
            empty_text.iter().count() == 1 && header_rows.is_empty()
        } else if snapshot.rows.is_empty() {
            materialized_rows == 0 && empty_text.is_empty() && header_rows.iter().count() == 1
        } else {
            materialized_rows == snapshot.rows.len() && header_rows.iter().count() == 1
        }
    } else {
        false
    };

    let cache_hit = cache
        .as_ref()
        .is_some_and(|(cached_snapshot, cached_revision)| {
            *cached_snapshot == snapshot
                && *cached_revision == panel.presentation_revision
                && host_ready
        });
    if cache_hit {
        return;
    }

    if let Ok(mut text) = title.single_mut() {
        **text = snapshot.title.clone();
    }

    let Ok(content_host) = content_hosts.single() else {
        return;
    };
    let Ok(header_host) = header_hosts.single() else {
        return;
    };

    commands.entity(content_host).despawn_children();
    commands.entity(header_host).despawn_children();
    reset_settlement_workforce_scroll(&mut scroll_state);

    if let Some(message) = &snapshot.empty_message {
        commands.entity(content_host).with_children(|body| {
            body.spawn((
                SettlementWorkforceEmptyText,
                Text::new(message.clone()),
                panel_body_font(),
                TextColor(TEXT_MUTED),
            ));
        });
        *cache = Some((snapshot, panel.presentation_revision));
        return;
    }

    let Some(settlement_id) = snapshot.settlement_id else {
        return;
    };

    commands
        .entity(header_host)
        .with_children(|header| spawn_matrix_header_row(header, &snapshot));
    commands.entity(content_host).with_children(|body| {
        for row in &snapshot.rows {
            spawn_matrix_data_row(body, settlement_id, row);
        }
    });

    *cache = Some((snapshot, panel.presentation_revision));
}

fn spawn_matrix_header_row(
    parent: &mut ChildSpawnerCommands<'_>,
    snapshot: &SettlementWorkforceSnapshot,
) {
    let (_, separator_bg, separator_border) = floating_row_separator_border();
    parent
        .spawn((
            SettlementWorkforceMatrixHeaderRow,
            separator_bg,
            separator_border,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(COL_GAP),
                padding: UiRect::vertical(Val::Px(4.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|row| {
            spawn_header_cell(row, "Unit", COL_UNIT_WIDTH);
            for (label, domain) in permission_column_labels(snapshot)
                .iter()
                .zip(snapshot.permission_columns.iter())
            {
                spawn_header_cell(row, label, permission_col_width(*domain));
            }
            spawn_header_cell(row, "Controls", COL_CONTROLS_WIDTH);
        });
}

fn spawn_header_cell(parent: &mut ChildSpawnerCommands<'_>, label: &str, width: f32) {
    parent.spawn((
        Text::new(label),
        panel_body_font(),
        TextColor(TEXT_MUTED),
        Node {
            width: Val::Px(width),
            flex_shrink: 0.0,
            ..default()
        },
    ));
}

fn spawn_matrix_data_row(
    parent: &mut ChildSpawnerCommands<'_>,
    settlement_id: SettlementId,
    row: &super::content::WorkforceMatrixRow,
) {
    let (_, separator_bg, separator_border) = floating_row_separator_border();
    parent
        .spawn((
            SettlementWorkforceMatrixDataRow {
                unit_id: row.unit_id,
            },
            separator_bg,
            separator_border,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(COL_GAP),
                align_items: AlignItems::Center,
                padding: UiRect::vertical(Val::Px(2.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|line| {
            line.spawn((
                Text::new(row.display_name.clone()),
                panel_body_font(),
                TextColor(TEXT_PRIMARY),
                Node {
                    width: Val::Px(COL_UNIT_WIDTH),
                    flex_shrink: 0.0,
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
            width: Val::Px(permission_col_width(cell.domain)),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|cell_row| {
            cell_row.spawn((
                Text::new(cell.skill_value.to_string()),
                panel_body_font(),
                TextColor(TEXT_PRIMARY),
            ));
            if cell.physically_capable == Some(false) {
                cell_row.spawn((
                    Text::new("Incapable"),
                    panel_body_font(),
                    TextColor(TEXT_MUTED),
                ));
                return;
            }
            let label = permission_checkbox_label(cell.permission_allowed);
            spawn_floating_raised_button(
                cell_row,
                WorkforcePermissionCheckbox {
                    settlement_id,
                    unit_id,
                    domain: cell.domain,
                    target_allowed: !cell.permission_allowed,
                },
                label,
                Node {
                    min_width: Val::Px(30.0),
                    min_height: Val::Px(24.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    ..default()
                },
            );
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
            width: Val::Px(COL_CONTROLS_WIDTH),
            flex_shrink: 0.0,
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
    spawn_floating_raised_button(
        parent,
        marker,
        label,
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
            ..default()
        },
    );
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
