//! Selection and command controllability rules (ADR-051 O1).

use crate::world::{UnitId, UnitRecord, WorldData};

use super::query::is_player_controllable;

/// Policy for whether non-player units may be selected (dev/debug override).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectionControllabilityPolicy {
    /// When true, any unit may be selected (dev mode inspect/debug).
    pub allow_non_player_selection: bool,
}

impl SelectionControllabilityPolicy {
    pub fn gameplay_default() -> Self {
        Self {
            allow_non_player_selection: false,
        }
    }

    pub fn dev_inspect() -> Self {
        Self {
            allow_non_player_selection: true,
        }
    }
}

/// Whether the local player may select this unit.
pub fn unit_is_selectable(
    record: &UnitRecord,
    policy: SelectionControllabilityPolicy,
) -> bool {
    if is_player_controllable(record) {
        return true;
    }
    if policy.allow_non_player_selection {
        return true;
    }
    false
}

/// Whether the local player may issue orders to this unit.
pub fn unit_is_commandable(record: &UnitRecord) -> bool {
    is_player_controllable(record)
}

/// Filter a candidate set to locally selectable units.
pub fn filter_selectable_unit_ids(
    world: &WorldData,
    unit_ids: impl IntoIterator<Item = UnitId>,
    policy: SelectionControllabilityPolicy,
) -> Vec<UnitId> {
    let mut ids: Vec<_> = unit_ids
        .into_iter()
        .filter(|id| {
            world
                .get_unit(*id)
                .is_some_and(|record| unit_is_selectable(record, policy))
        })
        .collect();
    ids.sort_by_key(|id| id.raw());
    ids
}

/// Filter selection to units that accept player commands.
pub fn filter_commandable_unit_ids(
    world: &WorldData,
    unit_ids: impl IntoIterator<Item = UnitId>,
) -> Vec<UnitId> {
    let mut ids: Vec<_> = unit_ids
        .into_iter()
        .filter(|id| {
            world
                .get_unit(*id)
                .is_some_and(unit_is_commandable)
        })
        .collect();
    ids.sort_by_key(|id| id.raw());
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_unit, create_unit_with_ownership, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
        Heightfield, LocalPosition, UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource,
        WorldPosition,
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
    fn selection_filters_non_controllable_units() {
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
        let neutral = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(2.0, 2.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;

        let policy = SelectionControllabilityPolicy::gameplay_default();
        let filtered = filter_selectable_unit_ids(&world, [player, neutral], policy);
        assert_eq!(filtered, vec![player]);
    }

    #[test]
    fn dev_policy_allows_neutral_selection() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let neutral = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let filtered = filter_selectable_unit_ids(
            &world,
            [neutral],
            SelectionControllabilityPolicy::dev_inspect(),
        );
        assert_eq!(filtered, vec![neutral]);
    }

    #[test]
    fn commands_ignore_non_controllable_units() {
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

        let commandable = filter_commandable_unit_ids(&world, [player, hostile]);
        assert_eq!(commandable, vec![player]);
    }
}
