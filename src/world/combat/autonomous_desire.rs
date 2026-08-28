//! Primitive autonomous combat desire from resolved relationship (ADR-132 Phase 6).
//!
//! Long-term interpretation belongs in behavior/personality layers (ADR-071). This module holds
//! the current placeholder combat-AI threshold only.

use crate::world::WorldData;
use crate::world::ownership::Affiliation;
use crate::world::relationship::{AuthoredRelationshipCatalog, effective_relationship_for_records};
use crate::world::unit::{UnitId, UnitRecord};

use super::targeting::AttackTargetingPolicy;

/// Current placeholder combat-AI interpretation — not a universal relationship semantic.
pub const HOSTILE_RELATIONSHIP_THRESHOLD: i32 = -100;

/// Outcome of evaluating autonomous proactive attack desire for one observer/target pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutonomousDesireDecision {
    pub wants_attack: bool,
    pub effective_relationship: i32,
    pub threshold: i32,
    pub dev_override: bool,
}

/// Evaluate whether `observer` autonomously wants to attack `target` using relationship truth.
pub fn evaluate_autonomous_desire(
    world: &WorldData,
    authored: &AuthoredRelationshipCatalog,
    observer: &UnitRecord,
    target: &UnitRecord,
    policy: AttackTargetingPolicy,
) -> AutonomousDesireDecision {
    if policy.dev_allow_all_targets || observer.affiliation == Affiliation::Dev {
        return AutonomousDesireDecision {
            wants_attack: observer.id != target.id,
            effective_relationship: 0,
            threshold: HOSTILE_RELATIONSHIP_THRESHOLD,
            dev_override: true,
        };
    }

    let effective = effective_relationship_for_records(
        authored,
        world.relationship_standing_store(),
        observer,
        target,
    );
    AutonomousDesireDecision {
        wants_attack: effective <= HOSTILE_RELATIONSHIP_THRESHOLD,
        effective_relationship: effective,
        threshold: HOSTILE_RELATIONSHIP_THRESHOLD,
        dev_override: false,
    }
}

/// Whether autonomous AI / AttackMove proactively wants to attack `target`.
pub fn autonomous_wants_to_attack(
    world: &WorldData,
    authored: &AuthoredRelationshipCatalog,
    observer: &UnitRecord,
    target: &UnitRecord,
    policy: AttackTargetingPolicy,
) -> bool {
    evaluate_autonomous_desire(world, authored, observer, target, policy).wants_attack
}

#[cfg(feature = "dev")]
pub fn trace_autonomous_desire_decision(
    observer_id: UnitId,
    target_id: UnitId,
    decision: AutonomousDesireDecision,
) {
    super::runtime_trace::autonomous_desire_decision(observer_id, target_id, decision);
}

#[cfg(not(feature = "dev"))]
pub fn trace_autonomous_desire_decision(
    _observer_id: UnitId,
    _target_id: UnitId,
    _decision: AutonomousDesireDecision,
) {
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use crate::world::relationship::{
        AuthoredFacetKey, AuthoredRelationshipCatalog, DirectedRelationshipEdgeKey, FactionId,
    };

    /// Standard Phase 6 authored edge: `wild -> player = -300`.
    pub fn phase6_authored_catalog() -> AuthoredRelationshipCatalog {
        AuthoredRelationshipCatalog::from_edges([(
            DirectedRelationshipEdgeKey::new(
                AuthoredFacetKey::Faction(FactionId::new("wild")),
                AuthoredFacetKey::Faction(FactionId::new("player")),
            ),
            -300,
        )])
        .expect("valid phase 6 authored edge")
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::phase6_authored_catalog;
    use super::*;
    use crate::world::ownership::UnitOwnership;
    use crate::world::relationship::{
        AuthoredFacetKey, AuthoredRelationshipCatalog, DirectedRelationshipEdgeKey, FactionId,
        RelationshipFacet, SpeciesId,
    };
    use crate::world::unit::UnitRecord;
    use crate::world::{UnitDefinitionId, UnitPlacement, UnitSource, WorldPosition};
    use bevy::prelude::{Quat, Vec3};

    use crate::world::{ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition};

    fn test_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
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

    fn unit(id: u64, faction: &str, species: &str, ownership: UnitOwnership) -> UnitRecord {
        UnitRecord::new(
            UnitId::new(id),
            UnitDefinitionId::new("test"),
            UnitPlacement::new(pos(0.0, 0.0), Quat::IDENTITY),
            UnitSource::Authored,
            ownership,
            5,
            FactionId::new(faction),
            SpeciesId::new(species),
        )
    }

    fn policy() -> AttackTargetingPolicy {
        AttackTargetingPolicy::default()
    }

    #[test]
    fn wild_to_player_authored_edge_creates_desire() {
        let authored = phase6_authored_catalog();
        let world = test_world();
        let wild = unit(1, "wild", "cavecrawler", UnitOwnership::wildlife());
        let player = unit(2, "player", "human", UnitOwnership::player_default());
        assert!(autonomous_wants_to_attack(
            &world,
            &authored,
            &wild,
            &player,
            policy(),
        ));
    }

    #[test]
    fn player_to_wild_zero_does_not_create_desire() {
        let authored = phase6_authored_catalog();
        let world = test_world();
        let wild = unit(1, "wild", "cavecrawler", UnitOwnership::wildlife());
        let player = unit(2, "player", "human", UnitOwnership::player_default());
        assert!(!autonomous_wants_to_attack(
            &world,
            &authored,
            &player,
            &wild,
            policy(),
        ));
    }

    #[test]
    fn directionality_differs_by_observer() {
        let authored = phase6_authored_catalog();
        let world = test_world();
        let wild = unit(1, "wild", "wolf", UnitOwnership::wildlife());
        let player = unit(2, "player", "human", UnitOwnership::player_default());
        assert!(autonomous_wants_to_attack(
            &world,
            &authored,
            &wild,
            &player,
            policy(),
        ));
        assert!(!autonomous_wants_to_attack(
            &world,
            &authored,
            &player,
            &wild,
            policy(),
        ));
    }

    #[test]
    fn threshold_inclusive_at_minus_one_hundred() {
        let authored = AuthoredRelationshipCatalog::from_edges([(
            DirectedRelationshipEdgeKey::new(
                AuthoredFacetKey::Faction(FactionId::new("wild")),
                AuthoredFacetKey::Faction(FactionId::new("player")),
            ),
            -100,
        )])
        .expect("edge");
        let world = test_world();
        let wild = unit(1, "wild", "wolf", UnitOwnership::wildlife());
        let player = unit(2, "player", "human", UnitOwnership::player_default());
        assert!(autonomous_wants_to_attack(
            &world,
            &authored,
            &wild,
            &player,
            policy(),
        ));
    }

    #[test]
    fn above_threshold_does_not_attack() {
        let authored = AuthoredRelationshipCatalog::from_edges([(
            DirectedRelationshipEdgeKey::new(
                AuthoredFacetKey::Faction(FactionId::new("wild")),
                AuthoredFacetKey::Faction(FactionId::new("player")),
            ),
            -99,
        )])
        .expect("edge");
        let world = test_world();
        let wild = unit(1, "wild", "wolf", UnitOwnership::wildlife());
        let player = unit(2, "player", "human", UnitOwnership::player_default());
        assert!(!autonomous_wants_to_attack(
            &world,
            &authored,
            &wild,
            &player,
            policy(),
        ));
    }

    #[test]
    fn far_below_threshold_still_attacks_without_clamp() {
        let authored = AuthoredRelationshipCatalog::from_edges([(
            DirectedRelationshipEdgeKey::new(
                AuthoredFacetKey::Faction(FactionId::new("wild")),
                AuthoredFacetKey::Faction(FactionId::new("player")),
            ),
            -10_000,
        )])
        .expect("edge");
        let world = test_world();
        let wild = unit(1, "wild", "wolf", UnitOwnership::wildlife());
        let player = unit(2, "player", "human", UnitOwnership::player_default());
        assert!(autonomous_wants_to_attack(
            &world,
            &authored,
            &wild,
            &player,
            policy(),
        ));
    }

    #[test]
    fn faction_and_species_contributions_stack() {
        let authored = AuthoredRelationshipCatalog::from_edges([
            (
                DirectedRelationshipEdgeKey::new(
                    AuthoredFacetKey::Faction(FactionId::new("wild")),
                    AuthoredFacetKey::Faction(FactionId::new("player")),
                ),
                -80,
            ),
            (
                DirectedRelationshipEdgeKey::new(
                    AuthoredFacetKey::Species(SpeciesId::new("wolf")),
                    AuthoredFacetKey::Species(SpeciesId::new("human")),
                ),
                -30,
            ),
        ])
        .expect("edges");
        let world = test_world();
        let wild = unit(1, "wild", "wolf", UnitOwnership::wildlife());
        let player = unit(2, "player", "human", UnitOwnership::player_default());
        assert!(autonomous_wants_to_attack(
            &world,
            &authored,
            &wild,
            &player,
            policy(),
        ));
    }

    #[test]
    fn standing_can_raise_relationship_above_threshold() {
        let authored = phase6_authored_catalog();
        let mut world = test_world();
        let wild = unit(1, "wild", "cavecrawler", UnitOwnership::wildlife());
        let player = unit(2, "player", "human", UnitOwnership::player_default());
        world.relationship_standing_store_mut().apply_delta(
            RelationshipFacet::Individual(UnitId::new(1)),
            RelationshipFacet::Individual(UnitId::new(2)),
            250,
        );
        assert!(!autonomous_wants_to_attack(
            &world,
            &authored,
            &wild,
            &player,
            policy(),
        ));
        assert_eq!(
            evaluate_autonomous_desire(&world, &authored, &wild, &player, policy())
                .effective_relationship,
            -50
        );
    }

    #[test]
    fn affiliation_hostile_alone_does_not_create_desire() {
        let authored = AuthoredRelationshipCatalog::default();
        let world = test_world();
        let hostile = unit(1, "bandits", "human", UnitOwnership::hostile());
        let player = unit(2, "player", "human", UnitOwnership::player_default());
        assert!(!autonomous_wants_to_attack(
            &world,
            &authored,
            &hostile,
            &player,
            policy(),
        ));
    }

    #[test]
    fn affiliation_wildlife_alone_does_not_create_desire() {
        let authored = AuthoredRelationshipCatalog::default();
        let world = test_world();
        let wildlife = unit(1, "wild", "wolf", UnitOwnership::wildlife());
        let player = unit(2, "player", "human", UnitOwnership::player_default());
        assert!(!autonomous_wants_to_attack(
            &world,
            &authored,
            &wildlife,
            &player,
            policy(),
        ));
    }

    #[test]
    fn dev_override_bypasses_relationship() {
        let authored = AuthoredRelationshipCatalog::default();
        let world = test_world();
        let a = unit(1, "player", "human", UnitOwnership::player_default());
        let b = unit(2, "player", "human", UnitOwnership::player_default());
        let dev_policy = AttackTargetingPolicy {
            dev_allow_all_targets: true,
        };
        assert!(autonomous_wants_to_attack(
            &world, &authored, &a, &b, dev_policy
        ));
    }
}
