//! Read-only health bar visibility rules (ADR-062 C9).

use crate::units::input::SelectedUnits;
use crate::world::{UnitId, WorldData, is_unit_alive};

/// Compute normalized HP percent; guards max_hp == 0.
pub fn health_percent(current_hp: u32, max_hp: u32) -> f32 {
    if max_hp == 0 {
        return 1.0;
    }
    (current_hp as f32 / max_hp as f32).clamp(0.0, 1.0)
}

/// Whether a unit should display an overhead health bar this frame.
pub fn should_show_health_bar(
    unit_id: UnitId,
    world: &WorldData,
    selection: &SelectedUnits,
    hovered_unit: Option<UnitId>,
    dev_show_all_health: bool,
) -> bool {
    let Some(record) = world.get_unit(unit_id) else {
        return false;
    };
    if !is_unit_alive(record) {
        return false;
    }
    if dev_show_all_health {
        return true;
    }
    if !matches!(record.combat_state, crate::world::CombatState::Peaceful) {
        return true;
    }
    if selection.contains(unit_id) {
        return true;
    }
    if hovered_unit == Some(unit_id) {
        return true;
    }
    record.vitals.current_hp < record.vitals.max_hp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, UnitCatalog,
        UnitDefinitionId, UnitSource, WorldPosition, create_unit,
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
    fn health_percent_computed_correctly() {
        assert!((health_percent(3, 10) - 0.3).abs() < f32::EPSILON);
        assert!((health_percent(0, 0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn full_health_unselected_unit_hidden_by_default() {
        let catalog = UnitCatalog::default();
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
        let selection = SelectedUnits::default();
        assert!(!should_show_health_bar(
            unit_id, &world, &selection, None, false
        ));
    }

    #[test]
    fn damaged_unit_visible() {
        let catalog = UnitCatalog::default();
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
        world.set_unit_hp(unit_id, 2).unwrap();
        assert!(should_show_health_bar(
            unit_id,
            &world,
            &SelectedUnits::default(),
            None,
            false
        ));
    }

    #[test]
    fn selected_unit_visible() {
        let catalog = UnitCatalog::default();
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
        let mut selection = SelectedUnits::default();
        selection.set_single(unit_id);
        assert!(should_show_health_bar(
            unit_id, &world, &selection, None, false
        ));
    }

    #[test]
    fn dead_unit_health_bar_hidden() {
        let catalog = UnitCatalog::default();
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
            .set_unit_state(unit_id, crate::world::UnitState::Dead)
            .unwrap();
        let mut selection = SelectedUnits::default();
        selection.set_single(unit_id);
        assert!(!should_show_health_bar(
            unit_id, &world, &selection, None, false
        ));
    }

    #[test]
    fn visibility_does_not_mutate_world_data() {
        let catalog = UnitCatalog::default();
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
        world.set_unit_hp(unit_id, 2).unwrap();
        let before = world.get_unit(unit_id).unwrap().clone();
        let _ = should_show_health_bar(unit_id, &world, &SelectedUnits::default(), None, false);
        assert_eq!(world.get_unit(unit_id).unwrap(), &before);
    }
}
