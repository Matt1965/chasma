//! Edge-to-edge weapon range checks (ADR-057 C4).

use crate::world::navigation::xz_distance;
use crate::world::unit::{UnitCatalog, UnitId, UnitOrderError, UnitRecord};
use crate::world::{WeaponCatalog, WeaponDefinition, WorldData, WorldPosition};

/// Extra meters before a unit resumes chasing after leaving attack range.
pub const RANGE_HYSTERESIS_METERS: f32 = 0.5;

/// Measured distances for one attacker/target pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeCheck {
    pub center_distance_meters: f32,
    pub edge_distance_meters: f32,
    pub weapon_range_meters: f32,
    pub attacker_radius_meters: f32,
    pub target_radius_meters: f32,
}

/// Whether the attacker can strike using edge-to-edge reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeStatus {
    InRange,
    OutOfRange,
}

pub fn collision_radius_for_record(record: &UnitRecord, unit_catalog: &UnitCatalog) -> f32 {
    unit_catalog
        .get(&record.definition_id)
        .map(|def| def.collision_radius_meters)
        .unwrap_or(0.5)
}

pub fn center_distance_meters(
    world: &WorldData,
    attacker_pos: WorldPosition,
    target_pos: WorldPosition,
) -> f32 {
    xz_distance(attacker_pos, target_pos, world.layout())
}

pub fn edge_distance_meters(
    center_distance_meters: f32,
    attacker_radius_meters: f32,
    target_radius_meters: f32,
) -> f32 {
    center_distance_meters - attacker_radius_meters - target_radius_meters
}

pub fn measure_weapon_range(
    world: &WorldData,
    attacker: &UnitRecord,
    target: &UnitRecord,
    weapon: &WeaponDefinition,
    unit_catalog: &UnitCatalog,
) -> RangeCheck {
    let center_distance_meters =
        center_distance_meters(world, attacker.placement.position, target.placement.position);
    let attacker_radius_meters = collision_radius_for_record(attacker, unit_catalog);
    let target_radius_meters = collision_radius_for_record(target, unit_catalog);
    RangeCheck {
        center_distance_meters,
        edge_distance_meters: edge_distance_meters(
            center_distance_meters,
            attacker_radius_meters,
            target_radius_meters,
        ),
        weapon_range_meters: weapon.range_meters,
        attacker_radius_meters,
        target_radius_meters,
    }
}

pub fn range_status_from_check(check: &RangeCheck) -> RangeStatus {
    if check.edge_distance_meters <= check.weapon_range_meters {
        RangeStatus::InRange
    } else {
        RangeStatus::OutOfRange
    }
}

pub fn is_in_weapon_range(
    world: &WorldData,
    attacker: &UnitRecord,
    target: &UnitRecord,
    unit_catalog: &UnitCatalog,
    weapon: &WeaponDefinition,
) -> bool {
    matches!(
        range_status_from_check(&measure_weapon_range(
            world, attacker, target, weapon, unit_catalog
        )),
        RangeStatus::InRange
    )
}

/// True when edge distance exceeds weapon range plus hysteresis (resume chase).
pub fn is_outside_weapon_range_with_hysteresis(
    world: &WorldData,
    attacker: &UnitRecord,
    target: &UnitRecord,
    unit_catalog: &UnitCatalog,
    weapon: &WeaponDefinition,
) -> bool {
    let check = measure_weapon_range(world, attacker, target, weapon, unit_catalog);
    check.edge_distance_meters > check.weapon_range_meters + RANGE_HYSTERESIS_METERS
}

pub fn weapon_for_unit_record<'a>(
    attacker: &UnitRecord,
    unit_catalog: &'a UnitCatalog,
    weapon_catalog: &'a WeaponCatalog,
) -> Result<&'a WeaponDefinition, UnitOrderError> {
    let definition = unit_catalog
        .get(&attacker.definition_id)
        .ok_or(UnitOrderError::DefinitionNotFound)?;
    let weapon_id = &definition.default_weapon_id;
    let weapon = weapon_catalog
        .get(weapon_id)
        .ok_or(UnitOrderError::MissingWeapon)?;
    if !weapon.enabled {
        return Err(UnitOrderError::MissingWeapon);
    }
    Ok(weapon)
}

pub fn range_check_for_units(
    world: &WorldData,
    attacker_id: UnitId,
    target_id: UnitId,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) -> Result<RangeCheck, UnitOrderError> {
    let attacker = world
        .get_unit(attacker_id)
        .ok_or(UnitOrderError::AttackerNotFound)?;
    let target = world
        .get_unit(target_id)
        .ok_or(UnitOrderError::TargetNotFound)?;
    let weapon = weapon_for_unit_record(attacker, unit_catalog, weapon_catalog)?;
    Ok(measure_weapon_range(
        world,
        attacker,
        target,
        weapon,
        unit_catalog,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_unit_with_ownership, ChunkCoord, ChunkLayout, LocalPosition, UnitDefinition,
        UnitDefinitionId, UnitOwnership, UnitRenderKey, WeaponDefinition, WeaponDefinitionId,
        WorldData,
    };
    use bevy::prelude::Vec3;

    fn layout_world() -> WorldData {
        WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        })
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn large_radius_catalog() -> UnitCatalog {
        UnitCatalog::from_definitions(vec![
            UnitDefinition::new(
                UnitDefinitionId::new("big_a"),
                "Big A",
                "Test",
                1,
                10,
                10,
                1,
                1,
                1,
                1,
                1,
                1,
                1.0,
                "T1",
                4.0,
                2.0,
                45.0,
                WeaponDefinitionId::new("weapon_short"),
                true,
                UnitRenderKey::reserved("big_a"),
            ),
            UnitDefinition::new(
                UnitDefinitionId::new("big_b"),
                "Big B",
                "Test",
                1,
                10,
                10,
                1,
                1,
                1,
                1,
                1,
                1,
                1.0,
                "T1",
                4.0,
                2.0,
                45.0,
                WeaponDefinitionId::new("weapon_short"),
                true,
                UnitRenderKey::reserved("big_b"),
            ),
        ])
        .unwrap()
    }

    fn short_weapon_catalog() -> WeaponCatalog {
        WeaponCatalog::from_definitions(vec![WeaponDefinition::new(
            WeaponDefinitionId::new("weapon_short"),
            "Short",
            "Test",
            1.0,
            crate::world::DamageType::Blunt,
            1.0,
            1.0,
            0.1,
            0.1,
            crate::world::HitMode::Melee,
            None,
            "attack",
            vec![crate::world::TargetFilter::Enemies],
            None,
            true,
        )])
        .unwrap()
    }

    #[test]
    fn edge_to_edge_uses_collision_radii() {
        let check = RangeCheck {
            center_distance_meters: 4.0,
            edge_distance_meters: edge_distance_meters(4.0, 2.0, 2.0),
            weapon_range_meters: 1.0,
            attacker_radius_meters: 2.0,
            target_radius_meters: 2.0,
        };
        assert_eq!(check.edge_distance_meters, 0.0);
        assert_eq!(range_status_from_check(&check), RangeStatus::InRange);
        assert!(check.center_distance_meters > check.weapon_range_meters);
    }

    #[test]
    fn same_center_large_radii_count_as_in_range() {
        let catalog = large_radius_catalog();
        let weapons = short_weapon_catalog();
        let mut world = layout_world();
        let player = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("big_a"),
            pos(10.0, 10.0),
            crate::world::UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let hostile = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("big_b"),
            pos(10.0, 10.0),
            crate::world::UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap()
        .id;
        let check = range_check_for_units(&world, player, hostile, &catalog, &weapons).unwrap();
        assert!(check.center_distance_meters < f32::EPSILON);
        assert!(check.edge_distance_meters <= 0.0);
        assert_eq!(range_status_from_check(&check), RangeStatus::InRange);
    }

    #[test]
    fn center_distance_alone_would_fail_but_edge_to_edge_passes() {
        let check = RangeCheck {
            center_distance_meters: 4.0,
            edge_distance_meters: 0.0,
            weapon_range_meters: 1.0,
            attacker_radius_meters: 2.0,
            target_radius_meters: 2.0,
        };
        assert!(check.center_distance_meters > check.weapon_range_meters);
        assert_eq!(range_status_from_check(&check), RangeStatus::InRange);
    }

    #[test]
    fn hysteresis_prevents_reentry_oscillation() {
        let check = RangeCheck {
            center_distance_meters: 3.05,
            edge_distance_meters: 1.05,
            weapon_range_meters: 1.0,
            attacker_radius_meters: 1.0,
            target_radius_meters: 1.0,
        };
        assert_eq!(range_status_from_check(&check), RangeStatus::OutOfRange);
        assert!(check.edge_distance_meters <= check.weapon_range_meters + RANGE_HYSTERESIS_METERS);
    }
}
