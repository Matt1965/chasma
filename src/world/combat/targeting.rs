//! Attack target validation (ADR-056 C3, relationship Phase 5).
//!
//! Four authorities live here and must not be conflated:
//! - **Mechanical targetability** — can combat operate on this pair?
//! - **Explicit player attack** — player-issued `UnitOrder::Attack` (mechanical + same-team only)
//! - **Default interaction intent** — conservative right-click classification via autonomous desire
//! - **Autonomous desire** — relationship-driven proactive hostility (Phase 6)

use crate::world::interaction::InteractionType;
use crate::world::ownership::{Affiliation, OwnerId, TeamId};
use crate::world::relationship::AuthoredRelationshipCatalog;
use crate::world::unit::{UnitOrderError, UnitRecord, UnitState};
use crate::world::{TargetFilter, UnitCatalog, UnitId, WeaponCatalog, WeaponDefinition, WorldData};

use super::autonomous_desire::{evaluate_autonomous_desire, trace_autonomous_desire_decision};

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

/// Policy hooks for dev/debug targeting overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AttackTargetingPolicy {
    /// When true, team and affiliation hostility checks are skipped (dev inspect mode).
    pub dev_allow_all_targets: bool,
}

pub fn is_unit_alive(record: &UnitRecord) -> bool {
    record.vitals.current_hp > 0 && !matches!(record.state, UnitState::Dead)
}

/// Whether dev overrides bypass mechanical team restrictions.
pub fn dev_bypasses_team_restriction(
    policy: AttackTargetingPolicy,
    affiliation: Affiliation,
) -> bool {
    policy.dev_allow_all_targets || affiliation == Affiliation::Dev
}

/// Mechanical same-team veto (friendly-fire semantics remain deferred).
pub fn same_team_blocks_attack(
    attacker_team_id: Option<TeamId>,
    target_team_id: Option<TeamId>,
    policy: AttackTargetingPolicy,
    attacker_affiliation: Affiliation,
) -> bool {
    if dev_bypasses_team_restriction(policy, attacker_affiliation) {
        return false;
    }
    attacker_team_id.is_some() && attacker_team_id == target_team_id
}

/// Autonomous acquisition / AttackMove — mechanical validity plus relationship-driven desire.
pub fn validate_autonomous_attack_target(
    world: &WorldData,
    authored: &AuthoredRelationshipCatalog,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    policy: AttackTargetingPolicy,
) -> Result<(), UnitOrderError> {
    validate_mechanical_attack_target(
        world,
        attacker_id,
        target_id,
        weapon_catalog,
        unit_catalog,
        policy,
    )?;

    let attacker = world
        .get_unit(attacker_id)
        .ok_or(UnitOrderError::AttackerNotFound)?;
    let target = world
        .get_unit(target_id)
        .ok_or(UnitOrderError::TargetNotFound)?;

    let decision = evaluate_autonomous_desire(world, authored, attacker, target, policy);
    if !decision.wants_attack {
        trace_autonomous_desire_decision(attacker_id, target_id, decision);
        return Err(UnitOrderError::InvalidOwnershipTarget);
    }

    Ok(())
}

pub fn is_valid_autonomous_attack_target(
    world: &WorldData,
    authored: &AuthoredRelationshipCatalog,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    policy: AttackTargetingPolicy,
) -> bool {
    validate_autonomous_attack_target(
        world,
        authored,
        attacker_id,
        target_id,
        weapon_catalog,
        unit_catalog,
        policy,
    )
    .is_ok()
}
pub fn validate_mechanical_attack_target(
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

    if snapshot_ownership_unavailable(attacker, policy) {
        return Err(UnitOrderError::InvalidOwnershipTarget);
    }

    if same_team_blocks_attack(
        attacker.team_id,
        target.team_id,
        policy,
        attacker.affiliation,
    ) {
        return Err(UnitOrderError::InvalidOwnershipTarget);
    }

    let weapon = weapon_for_unit(attacker, unit_catalog, weapon_catalog)?;
    if !weapon_allows_target(weapon, target) {
        return Err(UnitOrderError::WeaponCannotTarget);
    }

    Ok(())
}

pub fn is_valid_mechanical_attack_target(
    world: &WorldData,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    policy: AttackTargetingPolicy,
) -> bool {
    validate_mechanical_attack_target(
        world,
        attacker_id,
        target_id,
        weapon_catalog,
        unit_catalog,
        policy,
    )
    .is_ok()
}

/// Explicit player-issued attack — mechanical validity only (relationship is not a shield).
pub fn validate_explicit_attack_target(
    world: &WorldData,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    policy: AttackTargetingPolicy,
) -> Result<(), UnitOrderError> {
    validate_mechanical_attack_target(
        world,
        attacker_id,
        target_id,
        weapon_catalog,
        unit_catalog,
        policy,
    )
}

pub fn is_valid_explicit_attack_target(
    world: &WorldData,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    policy: AttackTargetingPolicy,
) -> bool {
    validate_explicit_attack_target(
        world,
        attacker_id,
        target_id,
        weapon_catalog,
        unit_catalog,
        policy,
    )
    .is_ok()
}

/// Validate a reactive self-defense target after confirmed attributed combat damage.
pub fn validate_reactive_retaliation_target(
    world: &WorldData,
    victim_id: UnitId,
    attacker_id: UnitId,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    policy: AttackTargetingPolicy,
) -> Result<(), UnitOrderError> {
    if victim_id == attacker_id {
        return Err(UnitOrderError::SelfTarget);
    }

    let victim = world
        .get_unit(victim_id)
        .ok_or(UnitOrderError::AttackerNotFound)?;
    let aggressor = world
        .get_unit(attacker_id)
        .ok_or(UnitOrderError::TargetNotFound)?;

    if !is_unit_alive(victim) {
        return Err(UnitOrderError::AttackerDead);
    }
    if !is_unit_alive(aggressor) {
        return Err(UnitOrderError::TargetDead);
    }

    if !reactive_retaliation_ownership_allows(victim, aggressor, policy) {
        return Err(UnitOrderError::InvalidOwnershipTarget);
    }

    let weapon = weapon_for_unit(victim, unit_catalog, weapon_catalog)?;
    if !weapon_allows_target(weapon, aggressor) {
        return Err(UnitOrderError::WeaponCannotTarget);
    }

    Ok(())
}

/// Validate an already-established combat target for continuation.
pub fn validate_active_combat_target(
    world: &WorldData,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    policy: AttackTargetingPolicy,
) -> Result<(), UnitOrderError> {
    if validate_explicit_attack_target(
        world,
        attacker_id,
        target_id,
        weapon_catalog,
        unit_catalog,
        policy,
    )
    .is_ok()
    {
        return Ok(());
    }

    let attacker = world
        .get_unit(attacker_id)
        .ok_or(UnitOrderError::AttackerNotFound)?;
    if attacker.reactive_combat_target != Some(target_id) {
        return Err(UnitOrderError::InvalidOwnershipTarget);
    }

    validate_reactive_retaliation_target(
        world,
        attacker_id,
        target_id,
        weapon_catalog,
        unit_catalog,
        policy,
    )
}

pub fn is_valid_active_combat_target(
    world: &WorldData,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    policy: AttackTargetingPolicy,
) -> bool {
    validate_active_combat_target(
        world,
        attacker_id,
        target_id,
        weapon_catalog,
        unit_catalog,
        policy,
    )
    .is_ok()
}

/// Default right-click interaction classification — uses autonomous desire, not explicit permissiveness.
pub fn classify_unit_target(
    world: &WorldData,
    authored: &AuthoredRelationshipCatalog,
    attacker_id: UnitId,
    target_id: UnitId,
    weapon_catalog: &WeaponCatalog,
    unit_catalog: &UnitCatalog,
    policy: AttackTargetingPolicy,
) -> InteractionType {
    if is_valid_autonomous_attack_target(
        world,
        authored,
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

/// Revalidate projectile target legality at impact using launch-time snapshot (REVIEW-A3).
///
/// Does not require the source unit to exist or be alive. Does not recheck weapon range.
/// Social hostility changes after launch do not invalidate impact.
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
    if same_team_blocks_attack(
        snapshot.source_team_id,
        target.team_id,
        AttackTargetingPolicy {
            dev_allow_all_targets: snapshot.dev_allow_all_targets,
        },
        snapshot.source_affiliation,
    ) {
        return Err(ProjectileImpactRejection::TargetNowFriendly);
    }
    if !weapon_allows_target_filters(&snapshot.weapon_target_filters, target) {
        return Err(ProjectileImpactRejection::TargetFilterRejected);
    }
    Ok(())
}

/// Mechanical weapon target class only — no affiliation, relationship, or desire input.
pub fn weapon_allows_target(weapon: &WeaponDefinition, target: &UnitRecord) -> bool {
    weapon_allows_target_filters(&weapon.target_filters, target)
}

pub fn weapon_allows_target_filters(filters: &[TargetFilter], target: &UnitRecord) -> bool {
    if filters.is_empty() {
        return true;
    }
    if filters.contains(&TargetFilter::All) {
        return true;
    }

    for filter in filters {
        match filter {
            TargetFilter::All => return true,
            TargetFilter::Units
            | TargetFilter::Enemies
            | TargetFilter::Wildlife
            | TargetFilter::Neutral => return true,
            TargetFilter::Structures => {
                let _ = target;
            }
        }
    }
    false
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

fn snapshot_ownership_unavailable(attacker: &UnitRecord, policy: AttackTargetingPolicy) -> bool {
    attacker.affiliation == Affiliation::Unknown && !policy.dev_allow_all_targets
}

fn reactive_retaliation_ownership_allows(
    victim: &UnitRecord,
    aggressor: &UnitRecord,
    policy: AttackTargetingPolicy,
) -> bool {
    if policy.dev_allow_all_targets || victim.affiliation == Affiliation::Dev {
        return victim.id != aggressor.id;
    }
    if victim.id == aggressor.id {
        return false;
    }
    if victim.team_id.is_some() && victim.team_id == aggressor.team_id {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::super::test_support::phase6_authored_catalog;
    use super::*;
    use crate::world::{
        AuthoredRelationshipCatalog, ChunkCoord, ChunkLayout, CombatState, LocalPosition,
        UnitCatalog, UnitDefinitionId, UnitId, UnitOwnership, UnitSource, WeaponCatalog, WorldData,
        WorldPosition, create_unit, create_unit_with_ownership,
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

    fn authored() -> AuthoredRelationshipCatalog {
        super::super::test_support::phase6_authored_catalog()
    }

    fn empty_authored() -> AuthoredRelationshipCatalog {
        AuthoredRelationshipCatalog::default()
    }

    fn spawn_player(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        id_key: &str,
        position: WorldPosition,
    ) -> UnitId {
        let id = create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new(id_key),
            position,
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let mut record = world.remove_unit_by_id(id).expect("unit exists");
        record.faction_id = crate::world::FactionId::new("player");
        let chunk = crate::world::ChunkId::new(record.placement.position.chunk);
        world.insert_unit(chunk, record).unwrap();
        id
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

    fn spawn_wildlife(world: &mut WorldData, catalog: &UnitCatalog) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("bandit"),
            pos(6.0, 6.0),
            UnitSource::Authored,
            UnitOwnership::wildlife(),
        )
        .unwrap()
        .id
    }

    fn spawn_wild_wolf(world: &mut WorldData, catalog: &UnitCatalog) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            pos(5.0, 5.0),
            UnitSource::Authored,
            UnitOwnership::wildlife(),
        )
        .unwrap()
        .id
    }

    #[test]
    fn explicit_attack_can_target_neutral_when_mechanically_valid() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let neutral = spawn_neutral(&mut world, &catalog);
        assert!(
            validate_explicit_attack_target(&world, player, neutral, &weapons, &catalog, policy(),)
                .is_ok()
        );
        assert_eq!(
            validate_autonomous_attack_target(
                &world,
                &empty_authored(),
                player,
                neutral,
                &weapons,
                &catalog,
                policy(),
            ),
            Err(UnitOrderError::InvalidOwnershipTarget)
        );
    }

    #[test]
    fn player_does_not_autonomously_attack_wild_at_zero_relationship() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let wild = spawn_wild_wolf(&mut world, &catalog);
        assert!(!is_valid_autonomous_attack_target(
            &world,
            &authored(),
            player,
            wild,
            &weapons,
            &catalog,
            policy(),
        ));
    }

    #[test]
    fn wild_autonomously_attacks_player_via_authored_relationship() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let wild = spawn_wild_wolf(&mut world, &catalog);
        assert!(is_valid_autonomous_attack_target(
            &world,
            &authored(),
            wild,
            player,
            &weapons,
            &catalog,
            policy(),
        ));
    }

    #[test]
    fn explicit_and_autonomous_reject_same_team() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let a = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let b = spawn_player(&mut world, &catalog, "bandit", pos(2.0, 2.0));
        assert_eq!(
            validate_explicit_attack_target(&world, a, b, &weapons, &catalog, policy()),
            Err(UnitOrderError::InvalidOwnershipTarget)
        );
        assert_eq!(
            validate_autonomous_attack_target(
                &world,
                &authored(),
                a,
                b,
                &weapons,
                &catalog,
                policy()
            ),
            Err(UnitOrderError::InvalidOwnershipTarget)
        );
    }

    #[test]
    fn default_interaction_classifies_neutral_as_non_attackable() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let neutral = spawn_neutral(&mut world, &catalog);
        assert_eq!(
            classify_unit_target(
                &world,
                &empty_authored(),
                player,
                neutral,
                &weapons,
                &catalog,
                policy(),
            ),
            InteractionType::NeutralUnit
        );
    }

    #[test]
    fn default_interaction_uses_relationship_not_affiliation() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let wild = spawn_wild_wolf(&mut world, &catalog);
        assert_eq!(
            classify_unit_target(
                &world,
                &authored(),
                player,
                wild,
                &weapons,
                &catalog,
                policy(),
            ),
            InteractionType::FriendlyUnit
        );
        assert_eq!(
            classify_unit_target(
                &world,
                &authored(),
                wild,
                player,
                &weapons,
                &catalog,
                policy(),
            ),
            InteractionType::AttackableUnit
        );
    }

    #[test]
    fn self_target_rejected() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        assert_eq!(
            validate_explicit_attack_target(&world, player, player, &weapons, &catalog, policy()),
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
            validate_explicit_attack_target(&world, player, hostile, &weapons, &catalog, policy()),
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
            validate_explicit_attack_target(&world, player, hostile, &weapons, &catalog, policy()),
            Err(UnitOrderError::TargetDead)
        );
    }

    #[test]
    fn structures_only_weapon_blocks_unit_targets() {
        let catalog = UnitCatalog::default();
        let mut weapons = WeaponCatalog::default();
        let wolf_bite = weapons
            .get(&crate::world::WeaponDefinitionId::new("weapon_wolf_bite"))
            .unwrap()
            .clone();
        let mut structures_only = wolf_bite.clone();
        structures_only.target_filters = vec![TargetFilter::Structures];
        structures_only.id = crate::world::WeaponDefinitionId::new("weapon_test_structures");
        let weapon_catalog = WeaponCatalog::from_definitions(vec![structures_only]).unwrap();

        let mut unit_catalog = catalog.clone();
        let mut bandit = unit_catalog
            .get(&UnitDefinitionId::new("bandit"))
            .unwrap()
            .clone();
        bandit.default_weapon_id = crate::world::WeaponDefinitionId::new("weapon_test_structures");
        unit_catalog = UnitCatalog::from_definitions(vec![bandit]).unwrap();

        let mut world = layout_world();
        let player = spawn_player(&mut world, &unit_catalog, "bandit", pos(1.0, 1.0));
        let hostile = spawn_hostile(&mut world, &unit_catalog);
        assert_eq!(
            validate_mechanical_attack_target(
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

    #[test]
    fn legacy_enemies_filter_matches_units_mechanically() {
        let catalog = UnitCatalog::default();
        let mut world = layout_world();
        let neutral = spawn_neutral(&mut world, &catalog);
        let target = world.get_unit(neutral).unwrap();
        assert!(weapon_allows_target_filters(
            &[TargetFilter::Enemies],
            target
        ));
        assert!(weapon_allows_target_filters(
            &[TargetFilter::Wildlife],
            target
        ));
        assert!(weapon_allows_target_filters(
            &[TargetFilter::Neutral],
            target
        ));
        assert!(weapon_allows_target_filters(&[TargetFilter::Units], target));
        assert!(!weapon_allows_target_filters(
            &[TargetFilter::Structures],
            target
        ));
    }

    #[test]
    fn wildlife_cannot_proactively_attack_player() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "bandit", pos(1.0, 1.0));
        let wildlife = spawn_wildlife(&mut world, &catalog);
        assert_eq!(
            validate_autonomous_attack_target(
                &world,
                &empty_authored(),
                wildlife,
                player,
                &weapons,
                &catalog,
                policy()
            ),
            Err(UnitOrderError::InvalidOwnershipTarget)
        );
    }

    #[test]
    fn wildlife_can_reactively_retaliate_against_player() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "bandit", pos(1.0, 1.0));
        let wildlife = spawn_wildlife(&mut world, &catalog);
        assert!(
            validate_reactive_retaliation_target(
                &world,
                wildlife,
                player,
                &weapons,
                &catalog,
                policy(),
            )
            .is_ok()
        );
    }

    #[test]
    fn active_combat_target_accepts_persisted_reactive_authorization() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "bandit", pos(1.0, 1.0));
        let wildlife = spawn_wildlife(&mut world, &catalog);
        world
            .set_reactive_combat_target(wildlife, Some(player))
            .unwrap();
        world
            .set_unit_combat_state(wildlife, CombatState::Attacking { target: player })
            .unwrap();
        assert_eq!(
            validate_autonomous_attack_target(
                &world,
                &empty_authored(),
                wildlife,
                player,
                &weapons,
                &catalog,
                policy()
            ),
            Err(UnitOrderError::InvalidOwnershipTarget)
        );
        assert!(
            validate_active_combat_target(&world, wildlife, player, &weapons, &catalog, policy(),)
                .is_ok()
        );
    }

    #[test]
    fn active_combat_target_keeps_explicit_neutral_engagement() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let neutral = spawn_neutral(&mut world, &catalog);
        assert!(
            validate_explicit_attack_target(&world, player, neutral, &weapons, &catalog, policy(),)
                .is_ok()
        );
        assert!(
            validate_active_combat_target(&world, player, neutral, &weapons, &catalog, policy(),)
                .is_ok()
        );
    }

    #[test]
    fn reactive_retaliation_rejects_same_team_friendly_fire() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player_a = spawn_player(&mut world, &catalog, "bandit", pos(1.0, 1.0));
        let player_b = spawn_player(&mut world, &catalog, "wolf", pos(2.0, 2.0));
        assert_eq!(
            validate_reactive_retaliation_target(
                &world,
                player_a,
                player_b,
                &weapons,
                &catalog,
                policy(),
            ),
            Err(UnitOrderError::InvalidOwnershipTarget)
        );
    }

    fn reassign_unit_ownership(world: &mut WorldData, unit_id: UnitId, ownership: UnitOwnership) {
        let mut record = world.remove_unit_by_id(unit_id).expect("unit exists");
        record.owner_id = ownership.owner_id;
        record.team_id = ownership.team_id;
        record.affiliation = ownership.affiliation;
        let chunk = crate::world::ChunkId::new(record.placement.position.chunk);
        world.insert_unit(chunk, record).unwrap();
    }

    #[test]
    fn projectile_impact_not_blocked_by_social_hostility_change() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let hostile = spawn_hostile(&mut world, &catalog);
        let attacker = world.get_unit(player).unwrap().clone();
        let weapon = weapons
            .get(
                &catalog
                    .get(&attacker.definition_id)
                    .unwrap()
                    .default_weapon_id,
            )
            .unwrap();
        let snapshot = ProjectileLaunchSnapshot::capture(&attacker, weapon, policy());
        reassign_unit_ownership(&mut world, hostile, UnitOwnership::neutral());
        assert!(validate_projectile_impact_target(&world, hostile, &snapshot).is_ok());
    }

    #[test]
    fn projectile_impact_still_blocks_same_team() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let player = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let hostile = spawn_hostile(&mut world, &catalog);
        let attacker = world.get_unit(player).unwrap().clone();
        let weapon = weapons
            .get(
                &catalog
                    .get(&attacker.definition_id)
                    .unwrap()
                    .default_weapon_id,
            )
            .unwrap();
        let snapshot = ProjectileLaunchSnapshot::capture(&attacker, weapon, policy());
        reassign_unit_ownership(&mut world, hostile, UnitOwnership::player_default());
        assert_eq!(
            validate_projectile_impact_target(&world, hostile, &snapshot),
            Err(ProjectileImpactRejection::TargetNowFriendly)
        );
    }

    #[test]
    fn dev_allow_all_bypasses_team_for_explicit_and_desire() {
        let catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = layout_world();
        let a = spawn_player(&mut world, &catalog, "wolf", pos(1.0, 1.0));
        let b = spawn_player(&mut world, &catalog, "bandit", pos(2.0, 2.0));
        let dev_policy = AttackTargetingPolicy {
            dev_allow_all_targets: true,
        };
        assert!(
            validate_explicit_attack_target(&world, a, b, &weapons, &catalog, dev_policy).is_ok()
        );
        assert!(super::super::autonomous_wants_to_attack(
            &world,
            &empty_authored(),
            world.get_unit(a).unwrap(),
            world.get_unit(b).unwrap(),
            dev_policy,
        ));
    }

    #[test]
    fn target_filter_units_parses() {
        assert_eq!(TargetFilter::parse("Units").unwrap(), TargetFilter::Units);
        assert_eq!(
            TargetFilter::parse("enemies").unwrap(),
            TargetFilter::Enemies
        );
    }
}
