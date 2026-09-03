//! Presentation lifecycle and unit-ring target tests.

use super::WorldObjectPresentationTarget;
use crate::client::selection::{
    ApplyWorldSelectionParams, WorldSelectionCategory, WorldSelectionChange,
    WorldSelectionRevision, WorldSelectionState, apply_world_selection,
};
use crate::units::input::SelectedUnits;
use crate::world::{BuildingId, DoodadId, ItemPileId, UnitId};

fn apply_params<'a>(
    world_selection: &'a mut WorldSelectionState,
    selected_units: &'a mut SelectedUnits,
    revision: &'a mut WorldSelectionRevision,
) -> ApplyWorldSelectionParams<'a> {
    ApplyWorldSelectionParams {
        world_selection,
        selected_units,
        hud: None,
        revision: Some(revision),
    }
}

#[test]
fn world_object_target_clears_on_category_switch() {
    let building = WorldObjectPresentationTarget::from_selection(
        WorldSelectionCategory::Building,
        Some(BuildingId::new(1)),
        None,
        None,
    );
    assert_eq!(
        building,
        Some(WorldObjectPresentationTarget::Building(BuildingId::new(1)))
    );

    let doodad = WorldObjectPresentationTarget::from_selection(
        WorldSelectionCategory::Doodad,
        None,
        Some(DoodadId::new(2)),
        None,
    );
    assert_eq!(
        doodad,
        Some(WorldObjectPresentationTarget::Doodad(DoodadId::new(2)))
    );

    let pile = WorldObjectPresentationTarget::from_selection(
        WorldSelectionCategory::ItemPile,
        None,
        None,
        Some(ItemPileId::new(3)),
    );
    assert_eq!(
        pile,
        Some(WorldObjectPresentationTarget::ItemPile(ItemPileId::new(3)))
    );

    assert!(
        WorldObjectPresentationTarget::from_selection(
            WorldSelectionCategory::Units,
            None,
            None,
            None,
        )
        .is_none()
    );
}

#[test]
fn category_switching_clears_stale_object_ids() {
    let mut world_selection = WorldSelectionState::default();
    let mut selected_units = SelectedUnits::default();
    let mut revision = WorldSelectionRevision::default();

    apply_world_selection(
        WorldSelectionChange::SelectBuilding {
            building_id: BuildingId::new(1),
        },
        &mut apply_params(&mut world_selection, &mut selected_units, &mut revision),
    );
    apply_world_selection(
        WorldSelectionChange::SelectDoodad {
            doodad_id: DoodadId::new(2),
        },
        &mut apply_params(&mut world_selection, &mut selected_units, &mut revision),
    );
    assert_eq!(world_selection.category, WorldSelectionCategory::Doodad);
    assert!(world_selection.building_id.is_none());

    apply_world_selection(
        WorldSelectionChange::SelectItemPile {
            pile_id: ItemPileId::new(3),
        },
        &mut apply_params(&mut world_selection, &mut selected_units, &mut revision),
    );
    assert_eq!(world_selection.category, WorldSelectionCategory::ItemPile);
    assert!(world_selection.doodad_id.is_none());

    apply_world_selection(
        WorldSelectionChange::ClearAll,
        &mut apply_params(&mut world_selection, &mut selected_units, &mut revision),
    );
    assert_eq!(world_selection.category, WorldSelectionCategory::None);
    assert!(
        WorldObjectPresentationTarget::from_selection(
            world_selection.category,
            world_selection.building_id,
            world_selection.doodad_id,
            world_selection.pile_id,
        )
        .is_none()
    );
}

#[test]
fn unit_selection_primary_remains_deterministic() {
    let mut world_selection = WorldSelectionState::default();
    let mut selected_units = SelectedUnits::default();
    let mut revision = WorldSelectionRevision::default();

    apply_world_selection(
        WorldSelectionChange::ReplaceUnits {
            unit_ids: vec![UnitId::new(9), UnitId::new(2)],
        },
        &mut apply_params(&mut world_selection, &mut selected_units, &mut revision),
    );

    assert_eq!(
        world_selection.primary_unit(&selected_units),
        Some(UnitId::new(2))
    );
}

#[test]
fn select_units_clears_world_object_presentation_target() {
    let mut world_selection = WorldSelectionState {
        category: WorldSelectionCategory::Building,
        building_id: Some(BuildingId::new(1)),
        ..Default::default()
    };
    let mut selected_units = SelectedUnits::default();
    let mut revision = WorldSelectionRevision::default();

    apply_world_selection(
        WorldSelectionChange::ReplaceUnits {
            unit_ids: vec![UnitId::new(1)],
        },
        &mut apply_params(&mut world_selection, &mut selected_units, &mut revision),
    );

    assert!(
        WorldObjectPresentationTarget::from_selection(
            world_selection.category,
            world_selection.building_id,
            world_selection.doodad_id,
            world_selection.pile_id,
        )
        .is_none()
    );
    assert_eq!(world_selection.category, WorldSelectionCategory::Units);
}
