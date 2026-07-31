//! Tests for unified world selection.

use super::*;
use crate::ui::gameplay::GameplayBuildingSelection;
use crate::world::{
    BuildingId, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadId, Heightfield, ItemPileId,
    LocalPosition, UnitId, WorldData, WorldPosition,
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

fn apply_params<'a>(
    world_selection: &'a mut WorldSelectionState,
    selected_units: &'a mut SelectedUnits,
    building_selection: &'a mut GameplayBuildingSelection,
    revision: &'a mut WorldSelectionRevision,
) -> ApplyWorldSelectionParams<'a> {
    ApplyWorldSelectionParams {
        world_selection,
        selected_units,
        building_selection,
        hud: None,
        revision: Some(revision),
    }
}

#[test]
fn select_unit_clears_building_category() {
    let mut world_selection = WorldSelectionState {
        category: WorldSelectionCategory::Building,
        building_id: Some(BuildingId::new(1)),
        ..Default::default()
    };
    let mut selected_units = SelectedUnits::default();
    let mut building_selection = GameplayBuildingSelection::default();
    let mut revision = WorldSelectionRevision::default();

    apply_world_selection(
        WorldSelectionChange::SelectUnit {
            unit_id: UnitId::new(5),
        },
        &mut apply_params(
            &mut world_selection,
            &mut selected_units,
            &mut building_selection,
            &mut revision,
        ),
    );

    assert_eq!(world_selection.category, WorldSelectionCategory::Units);
    assert!(world_selection.building_id.is_none());
    assert!(building_selection.building_id.is_none());
    assert!(selected_units.contains(UnitId::new(5)));
    assert_eq!(revision.0, 1);
}

#[test]
fn select_building_clears_units() {
    let mut world_selection = WorldSelectionState::default();
    let mut selected_units = SelectedUnits::default();
    selected_units.set_single(UnitId::new(2));
    let mut building_selection = GameplayBuildingSelection::default();
    let mut revision = WorldSelectionRevision::default();

    apply_world_selection(
        WorldSelectionChange::SelectBuilding {
            building_id: BuildingId::new(9),
        },
        &mut apply_params(
            &mut world_selection,
            &mut selected_units,
            &mut building_selection,
            &mut revision,
        ),
    );

    assert_eq!(world_selection.category, WorldSelectionCategory::Building);
    assert_eq!(world_selection.building_id, Some(BuildingId::new(9)));
    assert!(selected_units.is_empty());
    assert_eq!(building_selection.building_id, Some(BuildingId::new(9)));
}

#[test]
fn multi_unit_replace_preserves_primary_rule() {
    let mut world_selection = WorldSelectionState::default();
    let mut selected_units = SelectedUnits::default();
    let mut building_selection = GameplayBuildingSelection::default();
    let mut revision = WorldSelectionRevision::default();

    apply_world_selection(
        WorldSelectionChange::ReplaceUnits {
            unit_ids: vec![UnitId::new(9), UnitId::new(2), UnitId::new(7)],
        },
        &mut apply_params(
            &mut world_selection,
            &mut selected_units,
            &mut building_selection,
            &mut revision,
        ),
    );

    assert_eq!(selected_units.0.len(), 3);
    assert_eq!(
        world_selection.primary_unit(&selected_units),
        Some(UnitId::new(2))
    );
}

#[test]
fn clear_all_resets_every_category() {
    let mut world_selection = WorldSelectionState {
        category: WorldSelectionCategory::Doodad,
        doodad_id: Some(DoodadId::new(3)),
        ..Default::default()
    };
    let mut selected_units = SelectedUnits::default();
    selected_units.set_single(UnitId::new(1));
    let mut building_selection = GameplayBuildingSelection::default();
    let mut revision = WorldSelectionRevision::default();

    apply_world_selection(
        WorldSelectionChange::ClearAll,
        &mut apply_params(
            &mut world_selection,
            &mut selected_units,
            &mut building_selection,
            &mut revision,
        ),
    );

    assert_eq!(world_selection.category, WorldSelectionCategory::None);
    assert!(selected_units.is_empty());
    assert!(building_selection.building_id.is_none());
}

#[test]
fn prune_building_selection_when_missing() {
    let world = flat_world();
    let mut world_selection = WorldSelectionState {
        category: WorldSelectionCategory::Building,
        building_id: Some(BuildingId::new(999)),
        ..Default::default()
    };
    let mut selected_units = SelectedUnits::default();
    let mut building_selection = GameplayBuildingSelection {
        building_id: Some(BuildingId::new(999)),
    };
    let mut revision = WorldSelectionRevision::default();

    prune_world_selection(
        &world,
        &mut apply_params(
            &mut world_selection,
            &mut selected_units,
            &mut building_selection,
            &mut revision,
        ),
    );

    assert_eq!(world_selection.category, WorldSelectionCategory::None);
    assert!(building_selection.building_id.is_none());
}

#[test]
fn select_doodad_then_building_leaves_no_stale_doodad() {
    let mut world_selection = WorldSelectionState::default();
    let mut selected_units = SelectedUnits::default();
    let mut building_selection = GameplayBuildingSelection::default();
    let mut revision = WorldSelectionRevision::default();
    let mut params = apply_params(
        &mut world_selection,
        &mut selected_units,
        &mut building_selection,
        &mut revision,
    );

    apply_world_selection(
        WorldSelectionChange::SelectDoodad {
            doodad_id: DoodadId::new(4),
        },
        &mut params,
    );
    apply_world_selection(
        WorldSelectionChange::SelectBuilding {
            building_id: BuildingId::new(1),
        },
        &mut params,
    );

    assert_eq!(world_selection.category, WorldSelectionCategory::Building);
    assert!(world_selection.doodad_id.is_none());
}

#[test]
fn item_pile_selection_clears_units() {
    let mut world_selection = WorldSelectionState::default();
    let mut selected_units = SelectedUnits::default();
    selected_units.set_single(UnitId::new(1));
    let mut building_selection = GameplayBuildingSelection::default();
    let mut revision = WorldSelectionRevision::default();

    apply_world_selection(
        WorldSelectionChange::SelectItemPile {
            pile_id: ItemPileId::new(7),
        },
        &mut apply_params(
            &mut world_selection,
            &mut selected_units,
            &mut building_selection,
            &mut revision,
        ),
    );

    assert_eq!(world_selection.category, WorldSelectionCategory::ItemPile);
    assert_eq!(world_selection.pile_id, Some(ItemPileId::new(7)));
    assert!(selected_units.is_empty());
}
