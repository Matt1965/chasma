//! Bottom-left selected unit stats panel (P-UI1).

use bevy::prelude::*;

use crate::units::input::SelectedUnits;
use crate::world::{UnitCatalog, UnitDefinition, UnitId, UnitRecord, UnitState, WeaponCatalog, WorldData};

use super::combat_display::{
    append_combat_state_lines, append_weapon_hud_lines, average_hp_percent, combat_target_id,
    weapon_display_for_unit,
};

use super::player_hud_state::primary_selected_unit;
use super::styles::{hud_title_font, PANEL_BG, TEXT_PRIMARY};

/// Marker for the selected-unit panel root.
#[derive(Component, Debug)]
pub struct SelectedUnitPanelRoot;

#[derive(Component, Debug)]
pub(crate) struct SelectedUnitPanelText;

/// Read-only snapshot for HUD stat display and change detection.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedUnitPanelSnapshot {
    pub selection_count: u32,
    pub primary_unit: Option<UnitId>,
    pub lines: Vec<String>,
}

pub fn build_selected_unit_snapshot(
    selection: &SelectedUnits,
    world: &WorldData,
    catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) -> SelectedUnitPanelSnapshot {
    let count = selection.0.len() as u32;
    let primary = primary_selected_unit(selection);

    if count == 0 {
        return SelectedUnitPanelSnapshot {
            selection_count: 0,
            primary_unit: None,
            lines: vec!["No unit selected".to_string()],
        };
    }

    if count > 1 {
        let mut lines = vec![format!("Selected: {count} units")];
        if let Some(avg) = average_hp_percent(selection, world) {
            lines.push(format!("Average HP: {avg:.0}%"));
        }
        if let Some(id) = primary {
            lines.push(format!("Primary: Unit #{}", id.raw()));
            if let Some(summary) = primary_unit_summary(id, world, catalog, weapon_catalog) {
                lines.push(summary);
            }
        }
        return SelectedUnitPanelSnapshot {
            selection_count: count,
            primary_unit: primary,
            lines,
        };
    }

    let unit_id = primary.expect("single selection implies primary");
    SelectedUnitPanelSnapshot {
        selection_count: 1,
        primary_unit: Some(unit_id),
        lines: format_single_unit_lines(unit_id, world, catalog, weapon_catalog),
    }
}

fn primary_unit_summary(
    unit_id: UnitId,
    world: &WorldData,
    catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) -> Option<String> {
    let record = world.get_unit(unit_id)?;
    let def = catalog.get(&record.definition_id)?;
    let weapon = weapon_display_for_unit(record, catalog, weapon_catalog)
        .map(|w| w.name)
        .unwrap_or_else(|| "unknown".to_string());
    Some(format!(
        "{} — {} / weapon: {}",
        def.display_name,
        unit_state_label(&record.state),
        weapon
    ))
}

pub fn format_single_unit_lines(
    unit_id: UnitId,
    world: &WorldData,
    catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) -> Vec<String> {
    let Some(record) = world.get_unit(unit_id) else {
        return vec![format!("Unit #{} (missing from world)", unit_id.raw())];
    };
    let Some(def) = catalog.get(&record.definition_id) else {
        return vec![
            format!("Unit #{}", unit_id.raw()),
            format!("Definition: {} (not in catalog)", record.definition_id.as_str()),
        ];
    };
    format_unit_detail_lines(unit_id, record, def, catalog, weapon_catalog)
}

pub fn format_unit_detail_lines(
    unit_id: UnitId,
    record: &UnitRecord,
    def: &UnitDefinition,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) -> Vec<String> {
    let mut lines = vec![
        def.display_name.clone(),
        format!("Unit ID: {}", unit_id.raw()),
        format!("Definition: {}", def.id.as_str()),
        format!("Faction: {}", def.faction_tag),
        format!("Level: {}", def.level),
        format!("HP: {}/{}", record.vitals.current_hp, record.vitals.max_hp),
        format!("Base HP: {}", def.base_hp),
        format!("STR: {}  DEX: {}  CON: {}", def.strength, def.dexterity, def.constitution),
        format!(
            "AGI: {}  CHA: {}  INT: {}",
            def.agility, def.charisma, def.intelligence
        ),
        format!("Move speed: {:.1} m/s", def.move_speed_mps),
        format!("Collision radius: {:.2} m", def.collision_radius_meters),
        format!("State: {}", unit_state_label(&record.state)),
        format!("Combat: {}", record.combat_state.label()),
    ];
    if let Some(weapon) = weapon_display_for_unit(record, unit_catalog, weapon_catalog) {
        append_weapon_hud_lines(&mut lines, &weapon);
    }
    append_combat_state_lines(&mut lines, record, combat_target_id(&record.combat_state));
    lines
}

pub fn unit_state_label(state: &UnitState) -> &'static str {
    match state {
        UnitState::Idle => "Idle",
        UnitState::Moving { .. } => "Moving",
        UnitState::Dead => "Dead",
    }
}

pub fn spawn_selected_unit_panel(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            SelectedUnitPanelRoot,
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: Val::Percent(32.0),
                padding: UiRect::all(Val::Px(super::styles::PANEL_PADDING_PX)),
                row_gap: Val::Px(2.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|panel| {
            panel.spawn((
                SelectedUnitPanelText,
                Text::new("No unit selected"),
                hud_title_font(),
                TextColor(TEXT_PRIMARY),
            ));
        });
}

/// Refresh stat text when the derived snapshot changes.
pub fn sync_selected_unit_panel(
    selection: Res<SelectedUnits>,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    weapon_catalog: Res<WeaponCatalog>,
    mut cache: Local<Option<SelectedUnitPanelSnapshot>>,
    mut text: Query<&mut Text, With<SelectedUnitPanelText>>,
) {
    let snapshot = build_selected_unit_snapshot(&selection, &world, &catalog, &weapon_catalog);
    if cache.as_ref() == Some(&snapshot) {
        return;
    }
    *cache = Some(snapshot.clone());

    let Ok(mut text) = text.single_mut() else {
        return;
    };
    **text = snapshot.lines.join("\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_unit, starter_weapon_definitions, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
        Heightfield, LocalPosition, UnitDefinitionId, UnitSource, WeaponCatalog, WorldPosition,
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

    fn default_weapons() -> WeaponCatalog {
        WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap()
    }

    #[test]
    fn empty_selection_shows_empty_state() {
        let snapshot = build_selected_unit_snapshot(
            &SelectedUnits::default(),
            &flat_world(),
            &wolf_catalog(),
            &default_weapons(),
        );
        assert_eq!(snapshot.selection_count, 0);
        assert_eq!(snapshot.lines[0], "No unit selected");
    }

    #[test]
    fn multi_selection_shows_count() {
        let mut selection = SelectedUnits::default();
        selection.replace_with([UnitId::new(1), UnitId::new(2)]);
        let snapshot = build_selected_unit_snapshot(&selection, &flat_world(), &wolf_catalog(), &default_weapons());
        assert_eq!(snapshot.selection_count, 2);
        assert!(snapshot.lines[0].contains("2 units"));
    }

    #[test]
    fn single_selection_reads_unit_definition_stats() {
        let catalog = wolf_catalog();
        let mut world = flat_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(4.0, 4.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let mut selection = SelectedUnits::default();
        selection.set_single(unit_id);
        let snapshot = build_selected_unit_snapshot(&selection, &world, &catalog, &default_weapons());
        let joined = snapshot.lines.join("\n");
        assert!(joined.contains("Wolf"));
        assert!(joined.contains("HP: 5/5"));
        assert!(joined.contains("Base HP: 5"));
        assert!(joined.contains("STR: 4"));
        assert!(joined.contains("Move speed: 4.5"));
        assert!(joined.contains("State: Idle"));
    }

    #[test]
    fn panel_snapshot_does_not_mutate_world_data() {
        let catalog = wolf_catalog();
        let mut world = flat_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let before = world.get_unit(unit_id).unwrap().clone();
        let mut selection = SelectedUnits::default();
        selection.set_single(unit_id);
        let _ = build_selected_unit_snapshot(&selection, &world, &catalog, &default_weapons());
        assert_eq!(world.get_unit(unit_id).unwrap(), &before);
    }

    #[test]
    fn dead_unit_selection_handled_gracefully() {
        let catalog = wolf_catalog();
        let weapons = default_weapons();
        let mut world = flat_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        world
            .set_unit_state(unit_id, UnitState::Dead)
            .expect("set dead");
        let mut selection = SelectedUnits::default();
        selection.set_single(unit_id);
        let snapshot = build_selected_unit_snapshot(&selection, &world, &catalog, &weapons);
        let joined = snapshot.lines.join("\n");
        assert!(joined.contains("Dead"));
        assert!(joined.contains("HP:"));
    }
}
