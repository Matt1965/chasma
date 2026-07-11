//! Bottom-center squad / available-units panel (P-UI1).

use bevy::prelude::*;

use crate::client::{ClientInputModifiers, ClientIntent, ClientIntentQueue};
use crate::units::input::SelectedUnits;
use crate::world::{UnitCatalog, UnitId, WorldData, player_units};

use super::layout::PlayerHudUi;
use super::player_hud_state::{PlayerHudState, SquadFilterMode, primary_selected_unit};
use super::styles::{
    ACCENT_GREEN, PANEL_BG, SQUAD_ENTRY_BG, SQUAD_ENTRY_HOVER, SQUAD_ENTRY_SELECTED, TEXT_MUTED,
    TEXT_PRIMARY, hud_body_font,
};

/// Marker for the squad panel root.
#[derive(Component, Debug)]
pub struct SquadPanelRoot;

#[derive(Component, Debug)]
pub struct SquadEntryButton {
    pub unit_id: UnitId,
}

#[derive(Component, Debug)]
struct SquadPanelHeader;

/// Ordered unit ids shown in the squad bar.
pub fn squad_panel_unit_ids(
    selection: &SelectedUnits,
    world: &WorldData,
    filter: SquadFilterMode,
) -> Vec<UnitId> {
    if !selection.is_empty() {
        let mut ids: Vec<_> = selection.iter().collect();
        ids.sort_by_key(|id| id.raw());
        return ids;
    }

    if filter == SquadFilterMode::SelectedOnly {
        return Vec::new();
    }

    player_units(world)
}

pub fn squad_display_name(unit_id: UnitId, world: &WorldData, catalog: &UnitCatalog) -> String {
    world
        .get_unit(unit_id)
        .and_then(|record| catalog.get(&record.definition_id))
        .map(|def| def.display_name.clone())
        .unwrap_or_else(|| format!("Unit #{}", unit_id.raw()))
}

pub fn spawn_squad_panel(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            SquadPanelRoot,
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: Val::Percent(36.0),
                padding: UiRect::all(Val::Px(super::styles::PANEL_PADDING_PX)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|panel| {
            panel.spawn((
                SquadPanelHeader,
                Text::new("Squad"),
                hud_body_font(),
                TextColor(TEXT_MUTED),
            ));
            panel.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(4.0),
                    align_items: AlignItems::FlexStart,
                    ..default()
                },
                SquadEntryList,
            ));
        });
}

#[derive(Component, Debug)]
pub(crate) struct SquadEntryList;

/// Rebuild squad entry buttons when the visible unit set changes.
pub fn sync_squad_panel(
    mut commands: Commands,
    selection: Res<SelectedUnits>,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    hud: Res<PlayerHudState>,
    list: Query<Entity, With<SquadEntryList>>,
    entries: Query<Entity, With<SquadEntryButton>>,
    mut cache: Local<Vec<UnitId>>,
) {
    let ids = squad_panel_unit_ids(&selection, &world, hud.squad_filter_mode);
    if *cache == ids {
        return;
    }
    *cache = ids.clone();

    for entity in &entries {
        commands.entity(entity).despawn();
    }

    let Ok(list_entity) = list.single() else {
        return;
    };

    let primary = primary_selected_unit(&selection);

    commands.entity(list_entity).with_children(|row| {
        for unit_id in ids {
            let label = squad_display_name(unit_id, &world, &catalog);
            let selected = selection.contains(unit_id);
            let is_primary = primary == Some(unit_id);
            row.spawn((
                SquadEntryButton { unit_id },
                PlayerHudUi,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(if selected {
                    SQUAD_ENTRY_SELECTED
                } else {
                    SQUAD_ENTRY_BG
                }),
                BorderColor::all(if is_primary {
                    ACCENT_GREEN
                } else {
                    Color::srgba(0.3, 0.45, 0.5, 0.6)
                }),
            ))
            .with_children(|btn| {
                btn.spawn((Text::new(label), hud_body_font(), TextColor(TEXT_PRIMARY)));
            });
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

/// Highlight squad entries on hover.
pub fn update_squad_entry_hover(
    selection: Res<SelectedUnits>,
    mut query: Query<(&Interaction, &SquadEntryButton, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, entry, mut bg) in &mut query {
        if selection.contains(entry.unit_id) {
            *bg = BackgroundColor(SQUAD_ENTRY_SELECTED);
            continue;
        }
        *bg = BackgroundColor(match *interaction {
            Interaction::Hovered | Interaction::Pressed => SQUAD_ENTRY_HOVER,
            Interaction::None => SQUAD_ENTRY_BG,
        });
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
    fn squad_lists_selection_when_non_empty() {
        let mut selection = SelectedUnits::default();
        selection.replace_with([UnitId::new(3), UnitId::new(1)]);
        let ids = squad_panel_unit_ids(&selection, &flat_world(), SquadFilterMode::SelectedOnly);
        assert_eq!(ids, vec![UnitId::new(1), UnitId::new(3)]);
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
        let ids = squad_panel_unit_ids(&selection, &world, SquadFilterMode::AvailableUnits);
        assert_eq!(ids, vec![id]);
    }
}
