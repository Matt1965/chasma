//! Player selection/command filtering via ownership (ADR-051 O1).

use crate::units::input::SelectedUnits;
use crate::world::{
    SelectionControllabilityPolicy, WorldData, filter_commandable_unit_ids,
    filter_selectable_unit_ids,
};

/// Remove units the local player cannot command from the selection set.
pub fn prune_non_commandable_from_selection(world: &WorldData, selection: &mut SelectedUnits) {
    let commandable: std::collections::HashSet<_> =
        filter_commandable_unit_ids(world, selection.iter())
            .into_iter()
            .collect();
    selection.0.retain(|id| commandable.contains(id));
}

/// Replace selection with selectable ids from a pick/box-select result.
pub fn apply_selectable_filter(
    world: &WorldData,
    policy: SelectionControllabilityPolicy,
    picked: impl IntoIterator<Item = crate::world::UnitId>,
) -> Vec<crate::world::UnitId> {
    filter_selectable_unit_ids(world, picked, policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition,
        UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource, WorldPosition, create_unit,
        create_unit_with_ownership,
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

    #[test]
    fn prune_selection_drops_non_commandable_units() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let hostile = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(2.0, 2.0),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Hostile),
        )
        .unwrap()
        .id;

        let mut selection = SelectedUnits::default();
        selection.replace_with([player, hostile]);
        prune_non_commandable_from_selection(&world, &mut selection);
        assert_eq!(selection.0.len(), 1);
        assert!(selection.contains(player));
    }
}
