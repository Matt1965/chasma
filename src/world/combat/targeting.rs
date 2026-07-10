//! Attack target validation (ADR-056 C3).

use crate::world::interaction::InteractionType;
use crate::world::ownership::{Affiliation, OwnerId, TeamId};
use crate::world::unit::{UnitOrderError, UnitRecord, UnitState};
use crate::world::{
    TargetFilter, UnitCatalog, UnitId, WeaponCatalog, WeaponDefinition, WorldData,
};

/// Frozen attacker ownership and weapon filter state at projectile launch (ADR-060, REVIEW-A3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectileLaunchSnapshot {
    pub source_unit_id: UnitId,
    pub source_owner_id: Option<OwnerId>,
    pub source_team_id: Option<TeamId>,
    pub source_affiliation: Affiliation,
    pub weapon_target_filters: Vec<TargetFilter>,
    pub dev_allow_all_targets: bool,
}

impl ProjectileLaunchSnapshot {
    pub fn capture(
        attacker: &UnitRecord,
        weapon: &WeaponDefinition,
        policy: AttackTargetingPolicy,
    ) -> Self {
        Self {
            source_unit_id: attacker.id,
            source_owner_id: attacker.owner_id,
            source_team_id: attacker.team_id,
            source_affiliation: attacker.affiliation,
            weapon_target_filters: weapon.target_filters.clone(),
            dev_allow_all_targets: policy.dev_allow_all_targets,
        }
    }

    /// Render-only tests that never resolve impact against live unit rules.
    pub fn render_test_placeholder(source_unit_id: UnitId) -> Self {
        Self {
            source_unit_id,
            source_owner_id: None,
            source_team_id: None,
            source_affiliation: Affiliation::Dev,
            weapon_target_filters: vec![TargetFilter::All],
            dev_allow_all_targets: true,
        }
    }
}

impl Default for ProjectileLaunchSnapshot {
    fn default() -> Self {
        Self {
            source_unit_id: UnitId::new(0),
            source_owner_id: None,
            source_team_id: None,
            source_affiliation: Affiliation::Unknown,
            weapon_target_filters: Vec::new(),
            dev_allow_all_targets: false,
        }
    }
}

/// Why a projectile impact was rejected (REVIEW-A3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileImpactRejection {
    TargetMissing,
    TargetDead,
    TargetNowFriendly,
    TargetFilterRejected,
    OwnershipUnavailable,
}

/// Revalidate projectile target legality at impact using launch-time snapshot (REVIEW-A3).
///
/// Does not require the source unit to exist or be alive. Does not recheck weapon range.
pub fn validate_projectile_impact_target(
    world: &WorldData,
    target_id: UnitId,
    snapshot: &ProjectileLaunchSnapshot,
) -> Result<(), ProjectileImpactRejection> {
    if snapshot.source_unit_id == target_id {
        return Err(ProjectileImpactRejection::TargetNowFriendly);
    }
    if snapshot.source_affiliation == Affiliation::Unknown && !snapshot.dev_allow_all_targets {
        return Err(ProjectileImpactRejection::OwnershipUnavailable);
    }
    let Some(target) = world.get_unit(target_id) else {
        return Err(ProjectileImpactRejection::TargetMissing);
    };
    if !is_unit_alive(target) {
        return Err(ProjectileImpactRejection::TargetDead);
    }
    let ownership_ok = ownership_allows_attack_parts(
        snapshot.source_unit_id,
        snapshot.source_team_id,
        snapshot.source_affiliation,
        target,
        snapshot.dev_allow_all_targets,
    );
    if !ownership_ok && !snapshot.weapon_target_filters.contains(&TargetFilter::Neutral) {
        return Err(ProjectileImpactRejection::TargetNowFriendly);
    }
    if !weapon_allows_target_filters(&snapshot.weapon_target_filters, target, ownership_ok) {
        return Err(ProjectileImpactRejection::TargetFilterRejected);
    }
    Ok(())
}

/// Policy hooks for dev/debug targeting overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AttackTargetingPolicy {
    /// When true, ownership hostility checks are skipped (dev inspect mode).
    pub dev_allow_all_targets: bool,
}

impl AttackTargetingPolicy {
    pub fn from_dev_selection_override(allow_non_player_selection: bool) -> Self {
        Self {
            dev_allow_all_targets: allow_non_player_selection,
        }
    }
}

pub fn is_unit_alive(record: &UnitRecord) -> bool {
    record.vitals.current_hp > 0 && !matches!(record.state, UnitState::Dead)
}

/// Validate an attack target; returns a typed error on failure.
pub fn validate_attack_target(
    world: &WorldData,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    policy: AttackTargetingPolicy,
) -> Result<(), UnitOrderError> {
    if attacker_id == target_id {
        return Err(UnitOrderError::SelfTarget);
    }

    let attacker = world
        .get_unit(attacker_id)
        .ok_or(UnitOrderError::AttackerNotFound)?;
    let target = world
        .get_unit(target_id)
        .ok_or(UnitOrderError::TargetNotFound)?;

    if !is_unit_alive(attacker) {
        return Err(UnitOrderError::AttackerDead);
    }
    if !is_unit_alive(target) {
        return Err(UnitOrderError::TargetDead);
    }

    let weapon = weapon_for_unit(attacker, unit_catalog, weapon_catalog)?;
    let ownership_ok =
        ownership_allows_attack(attacker, target, policy.dev_allow_all_targets);
    if !ownership_ok && !weapon.target_filters.contains(&TargetFilter::Neutral) {
        return Err(UnitOrderError::InvalidOwnershipTarget);
    }
    if !weapon_allows_target(weapon, target, ownership_ok) {
        return Err(UnitOrderError::WeaponCannotTarget);
    }

    Ok(())
}

pub fn is_valid_attack_target(
    world: &WorldData,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    policy: AttackTargetingPolicy,
) -> bool {
    validate_attack_target(
        world,
        attacker_id,
        target_id,
        weapon_catalog,
        unit_catalog,
        policy,
    )
    .is_ok()
}

/// Classify a unit-under-cursor relative to an attacker (interaction layer).
pub fn classify_unit_target(
    world: &WorldData,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    policy: AttackTargetingPolicy,
) -> InteractionType {
    if is_valid_attack_target(
        world,
        attacker_id,
        target_id,
        weapon_catalog,
        unit_catalog,
        policy,
    ) {
        return InteractionType::AttackableUnit;
    }

    let Some(target) = world.get_unit(target_id) else {
        return InteractionType::MoveTarget;
    };

    match target.affiliation {
        Affiliation::Neutral => InteractionType::NeutralUnit,
        _ => InteractionType::FriendlyUnit,
    }
}

fn weapon_for_unit<'a>(
    attacker: &UnitRecord,
    unit_catalog: &'a UnitCatalog,
    weapon_catalog: &'a WeaponCatalog,
) -> Result<&'a WeaponDefinition, UnitOrderError> {
    let definition = unit_catalog
        .get(&attacker.definition_id)
        .ok_or(UnitOrderError::MissingWeapon)?;
    let weapon_id = &definition.default_weapon_id;
    let weapon = weapon_catalog
        .get(weapon_id)
        .ok_or(UnitOrderError::MissingWeapon)?;
    if !weapon.enabled {
        return Err(UnitOrderError::MissingWeapon);
    }
    Ok(weapon)
}

/// Runtime ownership hostility (ADR-051). Never uses catalog `faction_tag`.
fn ownership_allows_attack(
    attacker: &UnitRecord,
    target: &UnitRecord,
    dev_allow_all: bool,
) -> bool {
    ownership_allows_attack_parts(
        attacker.id,
        attacker.team_id,
        attacker.affiliation,
        target,
        dev_allow_all,
    )
}

fn ownership_allows_attack_parts(
    attacker_id: UnitId,
    attacker_team_id: Option<TeamId>,
    attacker_affiliation: Affiliation,
    target: &UnitRecord,
    dev_allow_all: bool,
) -> bool {
    if dev_allow_all || attacker_affiliation == Affiliation::Dev {
        return attacker_id != target.id;
    }

    if attacker_team_id.is_some() && attacker_team_id == target.team_id {
        return false;
    }

    match attacker_affiliation {
        Affiliation::Player => matches!(
            target.affiliation,
            Affiliation::Hostile | Affiliation::Wildlife
        ),
        Affiliation::Hostile => target.affiliation == Affiliation::Player,
        Affiliation::Dev => true,
        _ => false,
    }
}

fn weapon_allows_target(
    weapon: &WeaponDefinition,
    target: &UnitRecord,
    ownership_ok: bool,
) -> bool {
    weapon_allows_target_filters(&weapon.target_filters, target, ownership_ok)
}

fn weapon_allows_target_filters(
    filters: &[TargetFilter],
    target: &UnitRecord,
    ownership_ok: bool,
) -> bool {
    if filters.contains(&TargetFilter::All) {
        return true;
    }

    for filter in filters {
        match filter {
            TargetFilter::All => return true,
            TargetFilter::Enemies if ownership_ok => return true,
            TargetFilter::Wildlife if target.affiliation == Affiliation::Wildlife => return true,
            TargetFilter::Neutral if target.affiliation == Affiliation::Neutral => return true,
            TargetFilter::Structures => {}
            TargetFilter::Enemies | TargetFilter::Wildlife | TargetFilter::Neutral => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_unit, create_unit_with_ownership, ChunkCoord, ChunkLayout, LocalPosition,
        UnitDefinitionId, UnitOwnership, UnitSource, WorldPosition,
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

    fn policy() -> AttackTargetingPolicy {
        AttackTargetingPolicy::default()
    }

    fn spawn_player(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        id_key: &str,
        position: WorldPosition,
    ) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new(id_key),
            position,
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id
    }

    fn spawn_hostile(world: &mut WorldData, catalog: &UnitCatalog) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("bandit"),
            pos(5.0, 5.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap()
        .id
    }

    fn spawn_neutral(world: &mut WorldData, catalog: &UnitCatalog) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("deer"),
            pos(6.0, 6.0),
            UnitSource::Authored,
            UnitOwnership::neutral(),
        )
        .unwrap()
        .id
    }

    #[test]
    fn player_can_attack_hostile() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let hostile = spawn_hostile(&mut world, &catalog);
        assert!(is_valid_attack_target(
            &world,
            player,
            hostile,
            &weapons,
            &catalog,
            policy(),
        ));
    }

    #[test]
    fn hostile_can_attack_player() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let hostile = spawn_hostile(&mut world, &catalog);
        assert!(is_valid_attack_target(
            &world,
            hostile,
            player,
            &weapons,
            &catalog,
            policy(),
        ));
    }

    #[test]
    fn player_cannot_attack_same_team() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let a = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let b = spawn_player(&mut world, &catalog, "bandit", pos(2.0, 2.0));
        assert_eq!(
            validate_attack_target(&world, a, b, &weapons, &catalog, policy()),
            Err(UnitOrderError::InvalidOwnershipTarget)
        );
    }

    #[test]
    fn player_cannot_attack_neutral_by_default() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let neutral = spawn_neutral(&mut world, &catalog);
        assert_eq!(
            validate_attack_target(&world, player, neutral, &weapons, &catalog, policy()),
            Err(UnitOrderError::InvalidOwnershipTarget)
        );
    }

    #[test]
    fn self_target_rejected() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        assert_eq!(
            validate_attack_target(&world, player, player, &weapons, &catalog, policy()),
            Err(UnitOrderError::SelfTarget)
        );
    }

    #[test]
    fn dead_attacker_rejected() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let hostile = spawn_hostile(&mut world, &catalog);
        world.damage_unit(player, 999).unwrap();
        assert_eq!(
            validate_attack_target(&world, player, hostile, &weapons, &catalog, policy()),
            Err(UnitOrderError::AttackerDead)
        );
    }

    #[test]
    fn dead_target_rejected() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let hostile = spawn_hostile(&mut world, &catalog);
        world.damage_unit(hostile, 999).unwrap();
        assert_eq!(
            validate_attack_target(&world, player, hostile, &weapons, &catalog, policy()),
            Err(UnitOrderError::TargetDead)
        );
    }

    #[test]
    fn weapon_target_filter_blocks_wildlife_only_weapon_vs_hostile() {
        let catalog = UnitCatalog::default();
        let mut weapons = WeaponCatalog::default();
        let wolf_bite = weapons
            .get(&crate::world::WeaponDefinitionId::new("weapon_wolf_bite"))
            .unwrap()
            .clone();
        let mut wildlife_only = wolf_bite.clone();
        wildlife_only.target_filters = vec![TargetFilter::Wildlife];
        wildlife_only.id = crate::world::WeaponDefinitionId::new("weapon_test_wildlife");
        let weapon_catalog =
            WeaponCatalog::from_definitions(vec![wildlife_only]).unwrap();

        let mut unit_catalog = catalog.clone();
        let mut bandit = unit_catalog
            .get(&UnitDefinitionId::new("bandit"))
            .unwrap()
            .clone();
        bandit.default_weapon_id = crate::world::WeaponDefinitionId::new("weapon_test_wildlife");
        unit_catalog = UnitCatalog::from_definitions(vec![bandit]).unwrap();

        let mut world = layout_world();
        let player = spawn_player(&mut world, &unit_catalog, "bandit", pos(1.0, 1.0));
        let hostile = spawn_hostile(&mut world, &unit_catalog);
        assert_eq!(
            validate_attack_target(
                &world,
                player,
                hostile,
                &weapon_catalog,
                &unit_catalog,
                policy(),
            ),
            Err(UnitOrderError::WeaponCannotTarget)
        );
    }
}
