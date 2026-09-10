//! Bottom-center squad / available-units panel (P-UI1).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::client::{ClientInputModifiers, ClientIntent, ClientIntentQueue};
use crate::units::input::SelectedUnits;
use crate::world::{UnitCatalog, UnitId, WorldData, player_units};

use super::hud::{
    HudViewportGeometry, hud_flex_section_node, hud_inset_fill_style, hud_inset_rim_style,
    hud_roster_card_style, hud_roster_ghost_slot_style, hud_stat_track_style,
};
use super::layout::PlayerHudUi;
use super::player_hud_state::primary_selected_unit;
use super::roster_scroll::{
    SquadRosterNavLeft, SquadRosterNavRight, SquadRosterScrollState, spawn_roster_nav_button,
};
use super::styles::{
    HUD_HP_FILL, HUD_ROSTER_MIN_WIDTH_PX, TEXT_MUTED, TEXT_PRIMARY, hud_caption_font,
    hud_title_font,
};

/// Fixed rows inside a roster card; the portrait plate absorbs the remainder.
pub const HUD_ROSTER_CARD_HP_HEIGHT_PX: f32 = 8.0;
pub const HUD_ROSTER_CARD_LABEL_HEIGHT_PX: f32 = 15.0;

/// Marker for the squad panel root.
#[derive(Component, Debug)]
pub struct SquadPanelRoot;

#[derive(Component, Debug)]
pub struct SquadEntryButton {
    pub unit_id: UnitId,
}

#[derive(Component, Debug)]
struct SquadPanelHeader;

/// Non-interactive placeholder matching roster card geometry.
#[derive(Component, Debug)]
pub struct SquadRosterGhostSlot;

/// Middle roster viewport (horizontal scroll).
#[derive(Component, Debug)]
pub struct SquadRosterViewport;

#[derive(Component, Debug)]
struct SquadEntryHpFill {
    unit_id: UnitId,
}

/// Ordered player-owned unit ids for the roster strip.
pub fn owned_roster_unit_ids(world: &WorldData) -> Vec<UnitId> {
    player_units(world)
}

/// Back-compat helper retained for tests.
pub fn squad_panel_unit_ids(
    _selection: &SelectedUnits,
    world: &WorldData,
    _filter: super::player_hud_state::SquadFilterMode,
) -> Vec<UnitId> {
    owned_roster_unit_ids(world)
}

pub fn squad_display_name(unit_id: UnitId, world: &WorldData, catalog: &UnitCatalog) -> String {
    world
        .get_unit(unit_id)
        .and_then(|record| catalog.get(&record.definition_id))
        .map(|def| def.display_name.clone())
        .unwrap_or_else(|| format!("Unit #{}", unit_id.raw()))
}

/// Header strip above the roster cards ("UNITS  n" in the mockup).
pub const HUD_ROSTER_HEADER_HEIGHT_PX: f32 = 24.0;

pub fn spawn_squad_panel(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            SquadPanelRoot,
            PlayerHudUi,
            hud_flex_section_node(HUD_ROSTER_MIN_WIDTH_PX),
        ))
        .with_children(|panel| {
            panel.spawn((
                SquadPanelHeader,
                Text::new("UNITS"),
                hud_caption_font(),
                TextColor(TEXT_MUTED),
                Node {
                    height: Val::Px(HUD_ROSTER_HEADER_HEIGHT_PX),
                    flex_shrink: 0.0,
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                },
            ));
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Stretch,
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    margin: UiRect::bottom(Val::Px(6.0)),
                    ..default()
                })
                .with_children(|row| {
                    spawn_roster_nav_button(row, "<", SquadRosterNavLeft);
                    row.spawn((
                        SquadRosterViewport,
                        PlayerHudUi,
                        Button,
                        Interaction::None,
                        Node {
                            flex_grow: 1.0,
                            min_height: Val::Px(0.0),
                            overflow: Overflow::scroll_x(),
                            ..default()
                        },
                    ))
                    .with_children(|viewport| {
                        viewport.spawn((
                            Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(HudViewportGeometry::default().card_gap),
                                align_items: AlignItems::Stretch,
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            SquadEntryList,
                        ));
                    });
                    spawn_roster_nav_button(row, ">", SquadRosterNavRight);
                });
        });
}

#[derive(Component, Debug)]
pub struct SquadEntryList;

fn roster_ghost_count(unit_count: usize, visible_slots: u32) -> usize {
    if unit_count == 0 {
        return visible_slots as usize;
    }
    visible_slots
        .saturating_sub(unit_count as u32)
        .min(visible_slots) as usize
}

fn spawn_roster_ghost_slot(parent: &mut ChildSpawnerCommands<'_>, card_width: f32) {
    let card_radius = BorderRadius::all(Val::Px(6.0));
    let portrait_radius = BorderRadius::all(Val::Px(4.0));
    let (face, border) = hud_roster_ghost_slot_style();
    parent
        .spawn((
            SquadRosterGhostSlot,
            PlayerHudUi,
            FocusPolicy::Pass,
            Node {
                width: Val::Px(card_width),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(4.0)),
                row_gap: Val::Px(4.0),
                align_items: AlignItems::Stretch,
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: card_radius,
                ..default()
            },
            face,
            border,
        ))
        .with_children(|slot| {
            slot.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    padding: UiRect::all(Val::Px(1.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: portrait_radius,
                    ..default()
                },
                hud_inset_rim_style(),
            ))
            .with_children(|rim| {
                rim.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        border_radius: portrait_radius,
                        ..default()
                    },
                    hud_inset_fill_style(),
                ));
            });
            slot.spawn((
                Node {
                    height: Val::Px(HUD_ROSTER_CARD_HP_HEIGHT_PX),
                    flex_shrink: 0.0,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                hud_stat_track_style(),
            ));
            slot.spawn(Node {
                height: Val::Px(HUD_ROSTER_CARD_LABEL_HEIGHT_PX),
                flex_shrink: 0.0,
                ..default()
            });
        });
}

/// Rebuild squad entry buttons when the visible unit set or roster geometry changes.
pub fn sync_squad_panel(
    mut commands: Commands,
    selection: Res<SelectedUnits>,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    geometry: Res<HudViewportGeometry>,
    mut roster_scroll: ResMut<SquadRosterScrollState>,
    list: Query<Entity, With<SquadEntryList>>,
    entries: Query<Entity, With<SquadEntryButton>>,
    ghosts: Query<Entity, With<SquadRosterGhostSlot>>,
    mut cache: Local<Option<(Vec<UnitId>, f32, u32)>>,
) {
    let ids = owned_roster_unit_ids(&world);
    let ghost_count = roster_ghost_count(ids.len(), geometry.visible_roster_slots);
    let fingerprint = (ids.clone(), geometry.card_width, ghost_count as u32);
    if cache.as_ref() == Some(&fingerprint) {
        return;
    }
    *cache = Some(fingerprint);
    roster_scroll.reset();

    for entity in entries.iter().chain(ghosts.iter()) {
        commands.entity(entity).despawn();
    }

    let Ok(list_entity) = list.single() else {
        return;
    };

    let primary = primary_selected_unit(&selection);
    let card_width = geometry.card_width;
    let card_gap = geometry.card_gap;

    commands.entity(list_entity).with_children(|row| {
        for unit_id in ids {
            let label = squad_display_name(unit_id, &world, &catalog);
            let selected = selection.contains(unit_id);
            let is_primary = primary == Some(unit_id);
            let hp_percent = world
                .get_unit(unit_id)
                .map(|record| {
                    if record.vitals.max_hp == 0 {
                        0.0
                    } else {
                        (record.vitals.current_hp as f32 / record.vitals.max_hp as f32) * 100.0
                    }
                })
                .unwrap_or(0.0);
            let card_radius = BorderRadius::all(Val::Px(6.0));
            let (card_face, card_border) =
                hud_roster_card_style(selected, is_primary, &Interaction::None);
            row.spawn((
                SquadEntryButton { unit_id },
                PlayerHudUi,
                Button,
                Node {
                    width: Val::Px(card_width),
                    height: Val::Percent(100.0),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(4.0)),
                    row_gap: Val::Px(4.0),
                    align_items: AlignItems::Stretch,
                    overflow: Overflow::clip(),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: card_radius,
                    ..default()
                },
                BackgroundColor(card_face),
                card_border,
            ))
            .with_children(|btn| {
                let portrait_radius = BorderRadius::all(Val::Px(4.0));
                btn.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        min_height: Val::Px(0.0),
                        padding: UiRect::all(Val::Px(1.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: portrait_radius,
                        ..default()
                    },
                    hud_inset_rim_style(),
                ))
                .with_children(|rim| {
                    rim.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: portrait_radius,
                            ..default()
                        },
                        hud_inset_fill_style(),
                    ))
                    .with_children(|portrait| {
                        let initial = label
                            .chars()
                            .next()
                            .map(|c| c.to_ascii_uppercase().to_string())
                            .unwrap_or_else(|| "?".to_string());
                        portrait.spawn((
                            Text::new(initial),
                            hud_title_font(),
                            TextColor(TEXT_MUTED),
                        ));
                    });
                });
                btn.spawn((
                    Node {
                        height: Val::Px(HUD_ROSTER_CARD_HP_HEIGHT_PX),
                        flex_shrink: 0.0,
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    hud_stat_track_style(),
                ))
                .with_children(|track| {
                    track.spawn((
                        SquadEntryHpFill { unit_id },
                        Node {
                            width: Val::Percent(hp_percent.clamp(0.0, 100.0)),
                            height: Val::Percent(100.0),
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(HUD_HP_FILL),
                    ));
                });
                btn.spawn((
                    Text::new(label),
                    hud_caption_font(),
                    TextColor(TEXT_PRIMARY),
                    Node {
                        max_width: Val::Px(card_width - 12.0),
                        height: Val::Px(HUD_ROSTER_CARD_LABEL_HEIGHT_PX),
                        flex_shrink: 0.0,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ));
            });
        }
        for _ in 0..ghost_count {
            spawn_roster_ghost_slot(row, card_width);
        }
    });
}

/// Route squad entry clicks through the client intent pipeline.
pub fn handle_squad_entry_clicks(
    mut queue: ResMut<ClientIntentQueue>,
    modifiers: Res<ClientInputModifiers>,
    interaction: Query<(&Interaction, &SquadEntryButton), Changed<Interaction>>,
) {
    for (state, entry) in &interaction {
        if *state != Interaction::Pressed {
            continue;
        }
        if modifiers.shift {
            queue.push(ClientIntent::ToggleUnitSelection {
                unit_id: entry.unit_id,
            });
        } else {
            queue.push(ClientIntent::SelectUnit {
                unit_id: entry.unit_id,
            });
        }
    }
}

fn apply_squad_entry_presentation(
    selection: &SelectedUnits,
    primary: Option<UnitId>,
    interaction: &Interaction,
    entry: &SquadEntryButton,
    bg: &mut BackgroundColor,
    border: &mut BorderColor,
) {
    let selected = selection.contains(entry.unit_id);
    let is_primary = primary == Some(entry.unit_id);
    let (face, next_border) = hud_roster_card_style(selected, is_primary, interaction);
    *bg = BackgroundColor(face);
    *border = next_border;
}

/// Keep roster card visuals in sync when selection changes.
pub fn sync_squad_entry_presentation(
    selection: Res<SelectedUnits>,
    mut query: Query<(
        &SquadEntryButton,
        &Interaction,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    if !selection.is_changed() {
        return;
    }
    let primary = primary_selected_unit(&selection);
    for (entry, interaction, mut bg, mut border) in &mut query {
        apply_squad_entry_presentation(
            &selection,
            primary,
            interaction,
            entry,
            &mut bg,
            &mut border,
        );
    }
}

/// Highlight squad entries on hover.
pub fn update_squad_entry_hover(
    selection: Res<SelectedUnits>,
    mut query: Query<
        (
            &Interaction,
            &SquadEntryButton,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Changed<Interaction>,
    >,
) {
    let primary = primary_selected_unit(&selection);
    for (interaction, entry, mut bg, mut border) in &mut query {
        apply_squad_entry_presentation(
            &selection,
            primary,
            interaction,
            entry,
            &mut bg,
            &mut border,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, UnitDefinitionId,
        UnitOwnership, UnitSource, WorldPosition, create_unit_with_ownership,
    };
    use bevy::prelude::Vec3;

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn wolf_catalog() -> UnitCatalog {
        UnitCatalog::default()
    }

    #[test]
    fn roster_lists_owned_units_deterministically() {
        let catalog = wolf_catalog();
        let mut world = flat_world();
        let id = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(2.0, 2.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let ids = owned_roster_unit_ids(&world);
        assert_eq!(ids, vec![id]);
    }

    #[test]
    fn roster_has_no_hard_cap() {
        let catalog = wolf_catalog();
        let mut world = flat_world();
        for i in 0..15 {
            create_unit_with_ownership(
                &catalog,
                &mut world,
                &UnitDefinitionId::new("wolf"),
                pos(i as f32, 0.0),
                UnitSource::Authored,
                UnitOwnership::player_default(),
            )
            .unwrap();
        }
        assert_eq!(owned_roster_unit_ids(&world).len(), 15);
    }

    #[test]
    fn roster_ghost_count_fills_visible_capacity_without_units() {
        assert_eq!(roster_ghost_count(0, 6), 6);
        assert_eq!(roster_ghost_count(2, 6), 4);
        assert_eq!(roster_ghost_count(8, 6), 0);
    }

    #[test]
    fn squad_lists_authored_units_when_selection_empty() {
        let catalog = wolf_catalog();
        let mut world = flat_world();
        let id = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(2.0, 2.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let selection = SelectedUnits::default();
        let ids = squad_panel_unit_ids(
            &selection,
            &world,
            super::super::player_hud_state::SquadFilterMode::AvailableUnits,
        );
        assert_eq!(ids, vec![id]);
    }
}
