//! Dedicated Brain dev window — layout, sync, and interaction.

use bevy::prelude::*;

use super::document::{
    BrainCardModel, BrainFieldRow, BrainHistoryBlock, BrainInspectorModel, BrainStatusTone,
    SettlementBrainDocument, UnitBrainDocument, build_settlement_document, build_unit_document,
    resolve_inspected_unit_id,
};
use super::history::BrainDecisionHistory;
use super::state::{BrainView, BrainWindowState};
use crate::client::selection::WorldSelectionState;
use crate::client::CameraSettlementContext;
use crate::dev::dev_mode::DevModeState;
use crate::dev::input::DevPanelUi;
use crate::dev::window::{DevWindowBody, DevWindowId, DevWindowRegistry, DevWindowUi};
use crate::dev::widgets::theme::{
    BTN_BG_ACTIVE, BTN_BG_IDLE, BTN_BORDER_ACTIVE, BTN_BORDER_IDLE, CARD_BG, CARD_BORDER,
    FIELD_BG_IDLE, FONT_SIZE_LABEL, SPACE_CONTROL, SPACE_SECTION, SPACE_BUTTON_PAD_X,
    SPACE_BUTTON_PAD_Y, STATUS_SUCCESS, STATUS_WARNING, TEXT_LABEL, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECTION, label_text_font, small_text_font,
};
use crate::units::input::SelectedUnits;
use crate::world::{UnitId, WorldData};

#[derive(Component, Debug)]
pub struct DevBrainWindowRoot;

#[derive(Component, Debug)]
pub struct DevBrainDynamicRoot;

#[derive(Component, Debug)]
pub struct DevBrainViewTab {
    pub view: BrainView,
}

#[derive(Component, Debug)]
pub struct DevBrainCardButton {
    pub card_id: String,
}

#[derive(Component, Debug)]
pub struct DevBrainNeedButton {
    pub need_id: String,
}

#[derive(Component, Debug)]
pub struct DevBrainWorkerButton {
    pub unit_id: UnitId,
}

#[derive(Component, Debug)]
pub struct DevBrainSourceRecordsToggle;

const SPINE_CONNECTOR_HEIGHT_PX: f32 = 6.0;

pub fn setup_brain_window_panel(mut commands: Commands, bodies: Query<(Entity, &DevWindowBody)>) {
    for (entity, body) in &bodies {
        if body.id != DevWindowId::Brain {
            continue;
        }
        commands.entity(entity).with_children(|panel| {
            panel.spawn((
                DevBrainWindowRoot,
                DevPanelUi,
                DevWindowUi,
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(SPACE_SECTION),
                    ..default()
                },
            ))
            .with_children(|root| {
                spawn_tabs(root);
                root.spawn((
                    DevBrainDynamicRoot,
                    DevPanelUi,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(SPACE_SECTION),
                        ..default()
                    },
                ));
            });
        });
        return;
    }
}

fn spawn_tabs(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(SPACE_CONTROL),
                flex_wrap: FlexWrap::Wrap,
                width: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|row| {
            spawn_tab(row, "Unit", BrainView::Unit);
            spawn_tab(row, "Settlement", BrainView::Settlement);
        });
}

fn spawn_tab(parent: &mut ChildSpawnerCommands<'_>, label: &str, view: BrainView) {
    parent
        .spawn((
            DevBrainViewTab { view },
            DevPanelUi,
            Button,
            Node {
                padding: UiRect::axes(Val::Px(SPACE_BUTTON_PAD_X), Val::Px(SPACE_BUTTON_PAD_Y)),
                ..default()
            },
            BackgroundColor(BTN_BG_IDLE),
            BorderColor::all(BTN_BORDER_IDLE),
        ))
        .with_children(|b| {
            b.spawn((
                DevPanelUi,
                Text::new(label),
                label_text_font(),
                TextColor(TEXT_PRIMARY),
            ));
        });
}

pub fn sync_brain_window(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut brain_state: ResMut<BrainWindowState>,
    world_selection: Res<WorldSelectionState>,
    selected_units: Res<SelectedUnits>,
    settlement_context: Res<CameraSettlementContext>,
    world: Res<WorldData>,
    history: Res<BrainDecisionHistory>,
    mut dynamic: Query<Entity, With<DevBrainDynamicRoot>>,
    mut commands: Commands,
    mut tabs: Query<(&DevBrainViewTab, &mut BackgroundColor, &mut BorderColor)>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Brain) {
        return;
    }

    for (tab, mut bg, mut border) in &mut tabs {
        let active = tab.view == brain_state.view;
        *bg = BackgroundColor(if active { BTN_BG_ACTIVE } else { BTN_BG_IDLE });
        *border = BorderColor::all(if active {
            BTN_BORDER_ACTIVE
        } else {
            BTN_BORDER_IDLE
        });
    }

    let unit_id = resolve_inspected_unit_id(&brain_state, &world_selection, &selected_units);
    brain_state.on_context_changed(unit_id, settlement_context.focused_settlement_id);

    let Ok(root) = dynamic.single_mut() else {
        return;
    };
    commands.entity(root).despawn_children();

    let selected_card = brain_state.selected_card_id.clone();
    let show_sources = brain_state.source_records_expanded;

    commands.entity(root).with_children(|parent| {
        match brain_state.view {
            BrainView::Unit => {
                let doc = build_unit_document(
                    &brain_state,
                    &world_selection,
                    &selected_units,
                    &world,
                    &history,
                );
                spawn_unit_view(parent, &doc, &selected_card, show_sources);
            }
            BrainView::Settlement => {
                let doc = build_settlement_document(&brain_state, &settlement_context, &world);
                spawn_settlement_view(parent, &doc, &selected_card, show_sources);
            }
        }
    });
}

fn spawn_unit_view(
    parent: &mut ChildSpawnerCommands<'_>,
    doc: &UnitBrainDocument,
    selected_card: &Option<String>,
    show_sources: bool,
) {
    if let Some(msg) = &doc.empty_message {
        spawn_muted(parent, msg);
        return;
    }
    spawn_context_header(parent, &doc.header_action, &doc.header_subtitle, doc.authority_label.as_deref());
    let card_id = selected_card
        .clone()
        .or_else(|| doc.cards.last().map(|c| c.id.clone()));
    spawn_main_columns(parent, &doc.cards, card_id.as_deref(), show_sources);
    spawn_history(parent, &doc.history);
}

fn spawn_settlement_view(
    parent: &mut ChildSpawnerCommands<'_>,
    doc: &SettlementBrainDocument,
    selected_card: &Option<String>,
    show_sources: bool,
) {
    if let Some(msg) = &doc.empty_message {
        spawn_muted(parent, msg);
        return;
    }
    spawn_context_header(parent, &doc.settlement_name, &doc.header_subtitle, Some("Settlement"));
    spawn_need_pressures(parent, &doc.needs);
    spawn_section_title(parent, &doc.chain_title);
    let card_id = selected_card
        .clone()
        .or_else(|| doc.cards.first().map(|c| c.id.clone()));
    spawn_main_columns(parent, &doc.cards, card_id.as_deref(), show_sources);
}

fn spawn_context_header(
    parent: &mut ChildSpawnerCommands<'_>,
    title: &str,
    subtitle: &str,
    authority: Option<&str>,
) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    flex_grow: 1.0,
                    ..default()
                },
            ))
            .with_children(|col| {
                col.spawn(primary_text(title));
                col.spawn(small_muted_text(subtitle));
            });
            if let Some(auth) = authority {
                row.spawn((
                    DevPanelUi,
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::FlexEnd,
                        row_gap: Val::Px(2.0),
                        ..default()
                    },
                ))
                .with_children(|col| {
                    col.spawn(authority_badge(format!("Authority: {}", auth)));
                    col.spawn(small_muted_text("Current unit state"));
                });
            }
        });
}

fn spawn_need_pressures(parent: &mut ChildSpawnerCommands<'_>, needs: &[super::document::SettlementNeedRow]) {
    spawn_section_title(parent, "Need pressures");
    spawn_muted(parent, "Latest need snapshot");
    for need in needs {
        parent
            .spawn((
                DevBrainNeedButton {
                    need_id: need.need_id.clone(),
                },
                DevPanelUi,
                Button,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    margin: UiRect::bottom(Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(if need.selected { BTN_BG_ACTIVE } else { CARD_BG }),
                BorderColor::all(if need.selected { BTN_BORDER_ACTIVE } else { CARD_BORDER }),
            ))
            .with_children(|card| {
                card.spawn((
                    DevPanelUi,
                    Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        width: Val::Percent(100.0),
                        ..default()
                    },
                ))
                .with_children(|r| {
                    r.spawn(label_text(&need.need_id));
                    r.spawn(label_text(&need.pressure.to_string()));
                });
                spawn_bar(card, need.pressure as f32, 100.0, STATUS_SUCCESS);
            });
    }
}

fn spawn_main_columns(
    parent: &mut ChildSpawnerCommands<'_>,
    cards: &[BrainCardModel],
    selected_id: Option<&str>,
    show_sources: bool,
) {
    let selected = cards
        .iter()
        .find(|c| Some(c.id.as_str()) == selected_id)
        .or_else(|| cards.last());
    parent
        .spawn((
            DevPanelUi,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(SPACE_SECTION),
                row_gap: Val::Px(SPACE_SECTION),
                align_items: AlignItems::FlexStart,
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(SPACE_CONTROL),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    flex_basis: Val::Percent(45.0),
                    min_width: Val::Px(180.0),
                    width: Val::Percent(100.0),
                    ..default()
                },
            ))
            .with_children(|spine| {
                spawn_section_title(spine, "Decision spine");
                spawn_muted(spine, "Cause to execution");
                for (index, card) in cards.iter().enumerate() {
                    if index > 0 {
                        spawn_spine_connector(spine);
                    }
                    if let Some(raw) = card.id.strip_prefix("worker:") {
                        if let Ok(raw_id) = raw.parse::<u64>() {
                            spawn_worker_spine_card(spine, card, selected_id, UnitId::new(raw_id));
                            continue;
                        }
                    }
                    spawn_spine_card(spine, card, selected_id);
                }
            });
            row.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(SPACE_SECTION)),
                    border: UiRect::all(Val::Px(1.0)),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    flex_basis: Val::Percent(55.0),
                    min_width: Val::Px(200.0),
                    width: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(CARD_BG),
                BorderColor::all(CARD_BORDER),
            ))
            .with_children(|inspector| {
                if let Some(card) = selected {
                    spawn_inspector(inspector, &card.inspector, show_sources);
                } else {
                    spawn_muted(inspector, "Select a spine card to inspect.");
                }
            });
        });
}

fn spawn_worker_spine_card(
    parent: &mut ChildSpawnerCommands<'_>,
    card: &BrainCardModel,
    selected_id: Option<&str>,
    unit_id: UnitId,
) {
    let selected = selected_id == Some(card.id.as_str());
    parent
        .spawn((
            DevBrainWorkerButton { unit_id },
            DevBrainCardButton {
                card_id: card.id.clone(),
            },
            DevPanelUi,
            Button,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(if selected { BTN_BG_ACTIVE } else { CARD_BG }),
            BorderColor::all(if selected { BTN_BORDER_ACTIVE } else { CARD_BORDER }),
        ))
        .with_children(|row| {
            row.spawn(small_muted_text(format!("{:02}", card.number)));
            row.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
            ))
            .with_children(|col| {
                col.spawn(label_text(&card.title));
                col.spawn(small_muted_text(&card.subtitle));
            });
            row.spawn(label_text_colored("Inspect unit", STATUS_SUCCESS));
        });
}

fn spawn_spine_card(
    parent: &mut ChildSpawnerCommands<'_>,
    card: &BrainCardModel,
    selected_id: Option<&str>,
) {
    let selected = selected_id == Some(card.id.as_str());
    parent
        .spawn((
            DevBrainCardButton {
                card_id: card.id.clone(),
            },
            DevPanelUi,
            Button,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(if selected { BTN_BG_ACTIVE } else { CARD_BG }),
            BorderColor::all(if selected { BTN_BORDER_ACTIVE } else { CARD_BORDER }),
        ))
        .with_children(|row| {
            row.spawn(small_muted_text(format!("{:02}", card.number)));
            row.spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
            ))
            .with_children(|col| {
                col.spawn(label_text(&card.title));
                col.spawn(small_muted_text(&card.subtitle));
                if let Some(note) = &card.link_note {
                    col.spawn(small_muted_text(note));
                }
            });
            row.spawn(label_text_colored(&card.status_label, tone_color(card.tone)));
        });
}

fn spawn_spine_connector(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                width: Val::Px(2.0),
                height: Val::Px(SPINE_CONNECTOR_HEIGHT_PX),
                margin: UiRect::left(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(CARD_BORDER),
        ));
}

fn spawn_inspector(
    parent: &mut ChildSpawnerCommands<'_>,
    model: &BrainInspectorModel,
    show_sources: bool,
) {
    parent.spawn(section_text(&model.system_tag));
    parent.spawn(primary_text(&model.title));
    if let Some(note) = &model.candidate_stale_note {
        spawn_muted(parent, note);
    }
    for row in &model.rows {
        spawn_field_row(parent, row);
    }
    if !model.candidate_race.is_empty() {
        spawn_section_title(parent, "Candidate race");
        let max_score = model
            .candidate_race
            .iter()
            .map(|c| c.total_score)
            .fold(1.0f32, f32::max);
        for cand in &model.candidate_race {
            spawn_candidate(parent, cand, max_score);
        }
        if model.candidate_extra > 0 {
            spawn_muted(parent, &format!("+{} more eligible candidates", model.candidate_extra));
        }
    }
    parent
        .spawn((
            DevBrainSourceRecordsToggle,
            DevPanelUi,
            Button,
            Node {
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
        ))
        .with_children(|b| {
            let label = if show_sources {
                "Source records (hide)"
            } else {
                "Source records (show)"
            };
            b.spawn(small_muted_text(label));
        });
    if show_sources {
        parent
            .spawn((
                DevPanelUi,
                Node {
                    flex_direction: FlexDirection::Column,
                    margin: UiRect::top(Val::Px(SPACE_CONTROL)),
                    padding: UiRect::all(Val::Px(SPACE_CONTROL)),
                    border: UiRect::all(Val::Px(1.0)),
                    row_gap: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(FIELD_BG_IDLE),
                BorderColor::all(CARD_BORDER),
            ))
            .with_children(|block| {
                for row in &model.source_records {
                    spawn_field_row(block, row);
                }
            });
    }
}

fn spawn_candidate(parent: &mut ChildSpawnerCommands<'_>, cand: &super::model::BrainCandidateRaceRow, max: f32) {
    let prefix = if cand.chosen { "[chosen] " } else { "" };
    parent.spawn(label_text_colored(
        format!(
            "{}{} score={:.1} (pri={:.0} dist=-{:.2})",
            prefix,
            cand.label,
            cand.total_score,
            cand.priority_component,
            cand.distance_component
        ),
        if cand.chosen {
            STATUS_SUCCESS
        } else {
            TEXT_PRIMARY
        },
    ));
    spawn_bar(
        parent,
        cand.total_score,
        max,
        if cand.chosen {
            STATUS_SUCCESS
        } else {
            TEXT_MUTED
        },
    );
}

fn spawn_history(parent: &mut ChildSpawnerCommands<'_>, blocks: &[BrainHistoryBlock]) {
    if blocks.is_empty() {
        return;
    }
    spawn_section_title(parent, "Recent changes");
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                width: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|row| {
            for block in blocks {
                row.spawn((
                    DevPanelUi,
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::all(Val::Px(6.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(CARD_BG),
                    BorderColor::all(CARD_BORDER),
                ))
                .with_children(|card| {
                    if block.is_latest {
                        card.spawn((
                            DevPanelUi,
                            Node {
                                width: Val::Px(3.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(STATUS_WARNING),
                        ));
                    }
                    card.spawn((
                        DevPanelUi,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(2.0),
                            ..default()
                        },
                    ))
                    .with_children(|col| {
                        col.spawn(small_muted_text(if block.is_latest {
                            format!("{} - latest", block.title)
                        } else {
                            block.title.clone()
                        }));
                        col.spawn(label_text(&block.detail));
                    });
                });
            }
        });
}

fn spawn_field_row(parent: &mut ChildSpawnerCommands<'_>, row: &BrainFieldRow) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
        ))
        .with_children(|r| {
            r.spawn(small_muted_text(&row.label));
            r.spawn(label_text(&row.value));
        });
}

fn spawn_bar(parent: &mut ChildSpawnerCommands<'_>, value: f32, max: f32, color: Color) {
    let ratio = (value / max.max(1.0)).clamp(0.0, 1.0);
    parent
        .spawn((
            DevPanelUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(6.0),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(FIELD_BG_IDLE),
        ))
        .with_children(|track| {
            track.spawn((
                DevPanelUi,
                Node {
                    width: Val::Percent(ratio * 100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(color),
            ));
        });
}

fn spawn_section_title(parent: &mut ChildSpawnerCommands<'_>, title: &str) {
    parent.spawn(section_text(title));
}

fn spawn_muted(parent: &mut ChildSpawnerCommands<'_>, text: &str) {
    parent.spawn(small_muted_text(text));
}

fn primary_text(text: impl Into<String>) -> (DevPanelUi, Text, TextFont, TextColor) {
    (
        DevPanelUi,
        Text::new(text),
        TextFont {
            font_size: FONT_SIZE_LABEL + 1.0,
            ..default()
        },
        TextColor(TEXT_PRIMARY),
    )
}

fn label_text(text: impl Into<String>) -> (DevPanelUi, Text, TextFont, TextColor) {
    (
        DevPanelUi,
        Text::new(text),
        label_text_font(),
        TextColor(TEXT_PRIMARY),
    )
}

fn label_text_colored(text: impl Into<String>, color: Color) -> (DevPanelUi, Text, TextFont, TextColor) {
    (
        DevPanelUi,
        Text::new(text),
        label_text_font(),
        TextColor(color),
    )
}

fn section_text(text: impl Into<String>) -> (DevPanelUi, Text, TextFont, TextColor) {
    (
        DevPanelUi,
        Text::new(text),
        small_text_font(),
        TextColor(TEXT_SECTION),
    )
}

fn small_muted_text(text: impl Into<String>) -> (DevPanelUi, Text, TextFont, TextColor) {
    (
        DevPanelUi,
        Text::new(text),
        small_text_font(),
        TextColor(TEXT_MUTED),
    )
}

fn authority_badge(text: String) -> impl Bundle {
    (
        DevPanelUi,
        Text::new(text),
        small_text_font(),
        TextColor(TEXT_LABEL),
        Node {
            padding: UiRect::axes(Val::Px(SPACE_BUTTON_PAD_X), Val::Px(3.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(CARD_BG),
        BorderColor::all(BTN_BORDER_ACTIVE),
    )
}

fn tone_color(tone: BrainStatusTone) -> Color {
    match tone {
        BrainStatusTone::Executing => STATUS_WARNING,
        BrainStatusTone::Rejected | BrainStatusTone::Unavailable => TEXT_MUTED,
        BrainStatusTone::Neutral | BrainStatusTone::Score => TEXT_LABEL,
        _ => STATUS_SUCCESS,
    }
}

pub fn handle_brain_window_input(
    registry: Res<DevWindowRegistry>,
    dev_state: Res<DevModeState>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut brain_state: ResMut<BrainWindowState>,
    view_tabs: Query<(&Interaction, &DevBrainViewTab), Changed<Interaction>>,
    cards: Query<(&Interaction, &DevBrainCardButton), Changed<Interaction>>,
    needs: Query<(&Interaction, &DevBrainNeedButton), Changed<Interaction>>,
    workers: Query<(&Interaction, &DevBrainWorkerButton), Changed<Interaction>>,
    sources: Query<&Interaction, (Changed<Interaction>, With<DevBrainSourceRecordsToggle>)>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Brain) {
        return;
    }
    let mut block = false;
    for (interaction, tab) in &view_tabs {
        if *interaction == Interaction::Pressed {
            brain_state.view = tab.view;
            brain_state.selected_card_id = None;
            block = true;
        }
    }
    for (interaction, card) in &cards {
        if *interaction == Interaction::Pressed {
            brain_state.selected_card_id = Some(card.card_id.clone());
            block = true;
        }
    }
    for (interaction, need) in &needs {
        if *interaction == Interaction::Pressed {
            brain_state.selected_need_id = Some(need.need_id.clone());
            brain_state.selected_card_id = None;
            block = true;
        }
    }
    for (interaction, worker) in &workers {
        if *interaction == Interaction::Pressed {
            brain_state.open_unit_inspect(worker.unit_id);
            block = true;
        }
    }
    for interaction in &sources {
        if *interaction == Interaction::Pressed {
            brain_state.source_records_expanded = !brain_state.source_records_expanded;
            block = true;
        }
    }
    if block {
        gate.block_gameplay_mouse = true;
    }
}
