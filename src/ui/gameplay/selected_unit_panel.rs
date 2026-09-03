//! Bottom-left selected-object stats panel (P-UI1, BP1).

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::units::input::SelectedUnits;
use crate::world::{
    BuildingCatalog, UnitCatalog, UnitDefinition, UnitId, UnitRecord, UnitState, WeaponCatalog,
    WorldData,
};

use super::building_panel::format_building_shell;

use super::combat_display::{
    append_combat_state_lines, append_weapon_hud_lines, average_hp_percent, combat_target_id,
    weapon_display_for_unit,
};

use super::player_hud_state::primary_selected_unit;
use super::styles::{PANEL_BG, TEXT_PRIMARY, hud_title_font};

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

const NO_SELECTION_LABEL: &str = "No selection";

/// Bottom-bar content from [`WorldSelectionState`] (units, buildings, or none).
pub fn build_selected_panel_snapshot(
    world_selection: &WorldSelectionState,
    selected_units: &SelectedUnits,
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    building_catalog: &BuildingCatalog,
    weapon_catalog: &WeaponCatalog,
) -> SelectedUnitPanelSnapshot {
    match world_selection.category {
        WorldSelectionCategory::Building => {
            let Some(building_id) = world_selection.building_id else {
                return no_selection_snapshot();
            };
            let Some(record) = world.get_building(building_id) else {
                return no_selection_snapshot();
            };
            let display_name = building_catalog
                .get(&record.definition_id)
                .map(|def| def.display_name.as_str())
                .unwrap_or(record.definition_id.as_str());
            let body = format_building_shell(
                display_name,
                record.lifecycle_state,
                record.vitals.current_hp,
                record.vitals.max_hp,
            );
            SelectedUnitPanelSnapshot {
                selection_count: 0,
                primary_unit: None,
                lines: body.lines().map(str::to_string).collect(),
            }
        }
        WorldSelectionCategory::Units => {
            build_selected_unit_snapshot(selected_units, world, unit_catalog, weapon_catalog)
        }
        _ => no_selection_snapshot(),
    }
}

fn no_selection_snapshot() -> SelectedUnitPanelSnapshot {
    SelectedUnitPanelSnapshot {
        selection_count: 0,
        primary_unit: None,
        lines: vec![NO_SELECTION_LABEL.to_string()],
    }
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
        return no_selection_snapshot();
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
            format!(
                "Definition: {} (not in catalog)",
                record.definition_id.as_str()
            ),
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
        format!(
            "STR: {}  DEX: {}  CON: {}",
            def.strength, def.dexterity, def.constitution
        ),
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
        UnitState::Working { .. } => "Working",
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
                Text::new(NO_SELECTION_LABEL),
                hud_title_font(),
                TextColor(TEXT_PRIMARY),
            ));
        });
}

/// Refresh stat text when the derived snapshot changes.
pub fn sync_selected_unit_panel(
    world_selection: Res<WorldSelectionState>,
    selection: Res<SelectedUnits>,
    world: Res<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    building_catalog: Res<BuildingCatalog>,
    weapon_catalog: Res<WeaponCatalog>,
    mut cache: Local<Option<SelectedUnitPanelSnapshot>>,
    mut text: Query<&mut Text, With<SelectedUnitPanelText>>,
) {
    let snapshot = build_selected_panel_snapshot(
        &world_selection,
        &selection,
        &world,
        &unit_catalog,
        &building_catalog,
        &weapon_catalog,
    );
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
    use crate::client::selection::{
        ApplyWorldSelectionParams, WorldSelectionCategory, WorldSelectionChange,
        WorldSelectionRevision, WorldSelectionState, apply_world_selection,
    };
    use crate::ui::gameplay::building_panel::BuildingPanelState;
    use crate::world::{
        Affiliation, BuildingDefinitionId, BuildingId, BuildingOwnership, BuildingPlacement,
        BuildingRecord, BuildingSource, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield,
        LocalPosition, UnitDefinitionId, UnitId, UnitSource, WeaponCatalog, WorldData,
        WorldPosition, create_unit, starter_weapon_definitions,
    };
    use bevy::prelude::{Quat, Vec3};

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

    fn building_catalog() -> BuildingCatalog {
        BuildingCatalog::default()
    }

    fn snapshot(
        world_selection: &WorldSelectionState,
        selected_units: &SelectedUnits,
        world: &WorldData,
    ) -> SelectedUnitPanelSnapshot {
        build_selected_panel_snapshot(
            world_selection,
            selected_units,
            world,
            &wolf_catalog(),
            &building_catalog(),
            &default_weapons(),
        )
    }

    fn insert_building(world: &mut WorldData, id: u64, ownership: BuildingOwnership) -> BuildingId {
        let building_id = BuildingId::new(id);
        let record = BuildingRecord::new(
            building_id,
            BuildingDefinitionId::new("prispod_farm"),
            BuildingPlacement::new(pos(10.0, 10.0), Quat::IDENTITY),
            ownership,
            300,
            BuildingSource::Authored,
        );
        let chunk = ChunkId::new(record.placement.position.chunk);
        world.insert_building(chunk, record).unwrap();
        building_id
    }

    fn select_building(
        world_selection: &mut WorldSelectionState,
        selected_units: &mut SelectedUnits,
        building_id: BuildingId,
    ) {
        let mut revision = WorldSelectionRevision::default();
        apply_world_selection(
            WorldSelectionChange::SelectBuilding { building_id },
            &mut ApplyWorldSelectionParams {
                world_selection,
                selected_units,
                hud: None,
                revision: Some(&mut revision),
            },
        );
    }

    #[test]
    fn empty_selection_shows_no_selection_state() {
        let snapshot = snapshot(
            &WorldSelectionState::default(),
            &SelectedUnits::default(),
            &flat_world(),
        );
        assert_eq!(snapshot.selection_count, 0);
        assert_eq!(snapshot.lines[0], NO_SELECTION_LABEL);
    }

    #[test]
    fn selected_unit_shows_existing_unit_info() {
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
        let mut selected_units = SelectedUnits::default();
        selected_units.set_single(unit_id);
        let world_selection = WorldSelectionState {
            category: WorldSelectionCategory::Units,
            ..Default::default()
        };
        let joined = snapshot(&world_selection, &selected_units, &world)
            .lines
            .join("\n");
        assert!(joined.contains("Wolf"));
        assert!(joined.contains("HP: 5/5"));
    }

    #[test]
    fn selected_owned_building_shows_public_stats() {
        let mut world = flat_world();
        let farm = insert_building(
            &mut world,
            1,
            BuildingOwnership::with_affiliation(Affiliation::Player),
        );
        let mut world_selection = WorldSelectionState::default();
        let mut selected_units = SelectedUnits::default();
        select_building(&mut world_selection, &mut selected_units, farm);
        let joined = snapshot(&world_selection, &selected_units, &world)
            .lines
            .join("\n");
        assert!(joined.contains("Prispod Farm"));
        assert!(joined.contains("Complete"));
        assert!(joined.contains("HP 300 / 300"));
    }

    #[test]
    fn selected_foreign_building_shows_public_stats() {
        let mut world = flat_world();
        let smelter = insert_building(
            &mut world,
            2,
            BuildingOwnership::with_affiliation(Affiliation::Hostile),
        );
        let mut world_selection = WorldSelectionState::default();
        let mut selected_units = SelectedUnits::default();
        select_building(&mut world_selection, &mut selected_units, smelter);
        let joined = snapshot(&world_selection, &selected_units, &world)
            .lines
            .join("\n");
        assert!(joined.contains("Prispod Farm"));
        assert!(joined.contains("HP 300 / 300"));
    }

    #[test]
    fn building_stats_visible_when_menu_targets_same_building() {
        let mut world = flat_world();
        let farm = insert_building(
            &mut world,
            3,
            BuildingOwnership::with_affiliation(Affiliation::Player),
        );
        let mut world_selection = WorldSelectionState::default();
        let mut selected_units = SelectedUnits::default();
        select_building(&mut world_selection, &mut selected_units, farm);
        let _menu_open = BuildingPanelState {
            open_building_id: Some(farm),
        };
        let joined = snapshot(&world_selection, &selected_units, &world)
            .lines
            .join("\n");
        assert!(joined.contains("Prispod Farm"));
        assert!(joined.contains("HP 300 / 300"));
    }

    #[test]
    fn open_farm_menu_and_select_unit_shows_unit_stats() {
        let catalog = wolf_catalog();
        let mut world = flat_world();
        let farm = insert_building(
            &mut world,
            4,
            BuildingOwnership::with_affiliation(Affiliation::Player),
        );
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let mut world_selection = WorldSelectionState::default();
        let mut selected_units = SelectedUnits::default();
        select_building(&mut world_selection, &mut selected_units, farm);
        let _menu_open = BuildingPanelState {
            open_building_id: Some(farm),
        };
        let mut revision = WorldSelectionRevision::default();
        apply_world_selection(
            WorldSelectionChange::SelectUnit { unit_id },
            &mut ApplyWorldSelectionParams {
                world_selection: &mut world_selection,
                selected_units: &mut selected_units,
                hud: None,
                revision: Some(&mut revision),
            },
        );
        let joined = snapshot(&world_selection, &selected_units, &world)
            .lines
            .join("\n");
        assert!(joined.contains("Wolf"));
        assert!(_menu_open.open_building_id == Some(farm));
    }

    #[test]
    fn open_farm_menu_and_select_foreign_building_shows_foreign_stats() {
        let mut world = flat_world();
        let farm = insert_building(
            &mut world,
            5,
            BuildingOwnership::with_affiliation(Affiliation::Player),
        );
        let foreign = insert_building(
            &mut world,
            6,
            BuildingOwnership::with_affiliation(Affiliation::Hostile),
        );
        let mut world_selection = WorldSelectionState::default();
        let mut selected_units = SelectedUnits::default();
        select_building(&mut world_selection, &mut selected_units, foreign);
        let menu = BuildingPanelState {
            open_building_id: Some(farm),
        };
        let joined = snapshot(&world_selection, &selected_units, &world)
            .lines
            .join("\n");
        assert!(joined.contains("Prispod Farm"));
        assert_eq!(menu.open_building_id, Some(farm));
    }

    #[test]
    fn clear_selection_shows_no_selection_while_menu_stays_open() {
        let mut world = flat_world();
        let farm = insert_building(
            &mut world,
            7,
            BuildingOwnership::with_affiliation(Affiliation::Player),
        );
        let mut world_selection = WorldSelectionState::default();
        let mut selected_units = SelectedUnits::default();
        select_building(&mut world_selection, &mut selected_units, farm);
        let menu = BuildingPanelState {
            open_building_id: Some(farm),
        };
        let mut revision = WorldSelectionRevision::default();
        apply_world_selection(
            WorldSelectionChange::ClearAll,
            &mut ApplyWorldSelectionParams {
                world_selection: &mut world_selection,
                selected_units: &mut selected_units,
                hud: None,
                revision: Some(&mut revision),
            },
        );
        let snapshot = snapshot(&world_selection, &selected_units, &world);
        assert_eq!(snapshot.lines[0], NO_SELECTION_LABEL);
        assert_eq!(menu.open_building_id, Some(farm));
    }

    #[test]
    fn multi_selection_shows_count() {
        let mut selection = SelectedUnits::default();
        selection.replace_with([UnitId::new(1), UnitId::new(2)]);
        let world_selection = WorldSelectionState {
            category: WorldSelectionCategory::Units,
            ..Default::default()
        };
        let snapshot = snapshot(&world_selection, &selection, &flat_world());
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
        let world_selection = WorldSelectionState {
            category: WorldSelectionCategory::Units,
            ..Default::default()
        };
        let joined = snapshot(&world_selection, &selection, &world)
            .lines
            .join("\n");
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
        let world_selection = WorldSelectionState {
            category: WorldSelectionCategory::Units,
            ..Default::default()
        };
        let _ = snapshot(&world_selection, &selection, &world);
        assert_eq!(world.get_unit(unit_id).unwrap(), &before);
    }

    #[test]
    fn dead_unit_selection_handled_gracefully() {
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
        world
            .set_unit_state(unit_id, UnitState::Dead)
            .expect("set dead");
        let mut selection = SelectedUnits::default();
        selection.set_single(unit_id);
        let world_selection = WorldSelectionState {
            category: WorldSelectionCategory::Units,
            ..Default::default()
        };
        let joined = snapshot(&world_selection, &selection, &world)
            .lines
            .join("\n");
        assert!(joined.contains("Dead"));
        assert!(joined.contains("HP:"));
    }
}
