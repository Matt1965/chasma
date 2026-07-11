//! Read-only combat presentation helpers for the gameplay HUD (ADR-061 C8).

use crate::units::input::SelectedUnits;
use crate::world::{
    AttackCycle, CombatState, DamageType, HitMode, UnitCatalog, UnitId, UnitRecord, WeaponCatalog,
    WeaponDefinition, WorldData, weapon_for_unit_record,
};

/// Weapon stats shown on the selected-unit panel (from [`WeaponCatalog`]).
#[derive(Debug, Clone, PartialEq)]
pub struct CombatWeaponDisplay {
    pub name: String,
    pub damage: f32,
    pub damage_type: String,
    pub range_meters: f32,
    pub attacks_per_second: f32,
    pub windup_seconds: f32,
    pub recovery_seconds: f32,
    pub hit_mode: String,
}

pub fn hit_mode_label(mode: HitMode) -> &'static str {
    match mode {
        HitMode::Melee => "Melee",
        HitMode::RangedInstant => "RangedInstant",
        HitMode::Projectile => "Projectile",
    }
}

pub fn damage_type_label(damage_type: DamageType) -> &'static str {
    match damage_type {
        DamageType::Physical => "Physical",
        DamageType::Piercing => "Piercing",
        DamageType::Blunt => "Blunt",
        DamageType::Slashing => "Slashing",
        DamageType::Fire => "Fire",
        DamageType::Acid => "Acid",
        DamageType::Energy => "Energy",
        DamageType::True => "True",
    }
}

pub fn weapon_display_from_definition(weapon: &WeaponDefinition) -> CombatWeaponDisplay {
    CombatWeaponDisplay {
        name: weapon.display_name.clone(),
        damage: weapon.damage,
        damage_type: damage_type_label(weapon.damage_type).to_string(),
        range_meters: weapon.range_meters,
        attacks_per_second: weapon.attacks_per_second,
        windup_seconds: weapon.windup_seconds,
        recovery_seconds: weapon.recovery_seconds,
        hit_mode: hit_mode_label(weapon.hit_mode).to_string(),
    }
}

pub fn weapon_display_for_unit(
    record: &UnitRecord,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) -> Option<CombatWeaponDisplay> {
    let weapon = weapon_for_unit_record(record, unit_catalog, weapon_catalog).ok()?;
    Some(weapon_display_from_definition(weapon))
}

pub fn combat_target_id(combat_state: &CombatState) -> Option<UnitId> {
    match combat_state {
        CombatState::Attacking { target } | CombatState::Chasing { target } => Some(*target),
        CombatState::AttackMoving {
            target: Some(target),
            ..
        } => Some(*target),
        _ => None,
    }
}

pub fn attack_cycle_summary(cycle: &AttackCycle) -> String {
    format!(
        "{:?} ({:.2}s remaining)",
        cycle.phase, cycle.phase_remaining_seconds
    )
}

/// Deterministic average HP percent across selected living units.
pub fn average_hp_percent(selection: &SelectedUnits, world: &WorldData) -> Option<f32> {
    if selection.is_empty() {
        return None;
    }
    let mut ids: Vec<_> = selection.iter().collect();
    ids.sort_by_key(|id| id.raw());
    let mut total_percent = 0.0_f32;
    let mut count = 0_u32;
    for unit_id in ids {
        let Some(record) = world.get_unit(unit_id) else {
            continue;
        };
        if record.vitals.max_hp == 0 {
            continue;
        }
        total_percent += record.vitals.current_hp as f32 / record.vitals.max_hp as f32;
        count += 1;
    }
    if count == 0 {
        return None;
    }
    Some((total_percent / count as f32) * 100.0)
}

pub fn append_weapon_hud_lines(lines: &mut Vec<String>, weapon: &CombatWeaponDisplay) {
    lines.push(format!("Weapon: {}", weapon.name));
    lines.push(format!(
        "Damage: {:.0} ({})",
        weapon.damage, weapon.damage_type
    ));
    lines.push(format!("Range: {:.1} m", weapon.range_meters));
    lines.push(format!("APS: {:.2}", weapon.attacks_per_second));
    lines.push(format!(
        "Windup: {:.2}s  Recovery: {:.2}s",
        weapon.windup_seconds, weapon.recovery_seconds
    ));
    lines.push(format!("Hit mode: {}", weapon.hit_mode));
}

pub fn append_combat_state_lines(
    lines: &mut Vec<String>,
    record: &UnitRecord,
    target: Option<UnitId>,
) {
    lines.push(format!("Combat state: {}", record.combat_state.label()));
    if let Some(target_id) = target {
        lines.push(format!("Target: Unit #{}", target_id.raw()));
    }
    if let Some(cycle) = record.attack_cycle.as_ref() {
        lines.push(format!("Attack phase: {}", attack_cycle_summary(cycle)));
    }
}

/// Edge-to-edge weapon reach from attacker center (meters) for range overlay.
pub fn attack_range_circle_radius_meters(
    weapon_range_meters: f32,
    attacker_collision_radius_meters: f32,
) -> f32 {
    (weapon_range_meters + attacker_collision_radius_meters).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, UnitDefinitionId,
        UnitSource, WeaponCatalog, WorldPosition, create_unit, starter_weapon_definitions,
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
    fn hud_reads_weapon_catalog_for_default_weapon() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap();
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
        let record = world.get_unit(unit_id).unwrap();
        let display = weapon_display_for_unit(record, &catalog, &weapons).unwrap();
        assert_eq!(display.name, "Wolf Bite");
        assert!((display.damage - 8.0).abs() < f32::EPSILON);
        assert_eq!(display.hit_mode, "Melee");
    }

    #[test]
    fn average_hp_percent_is_deterministic() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let a = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let b = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(2.0, 2.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        world.set_unit_hp(a, 2).unwrap();
        world.set_unit_hp(b, 4).unwrap();
        let mut selection = SelectedUnits::default();
        selection.replace_with([b, a]);
        let avg = average_hp_percent(&selection, &world).unwrap();
        assert!((avg - 60.0).abs() < 0.01);
    }

    #[test]
    fn attack_range_overlay_uses_edge_to_edge_reach() {
        let radius = attack_range_circle_radius_meters(8.0, 0.5);
        assert!((radius - 8.5).abs() < f32::EPSILON);
    }

    #[test]
    fn combat_target_reads_unit_record_only() {
        let target = UnitId::new(9);
        let state = CombatState::Attacking { target };
        assert_eq!(combat_target_id(&state), Some(target));
        assert_eq!(combat_target_id(&CombatState::Peaceful), None);
    }
}
