//! Directional relationship resolver and provenance (ADR-132 Phase 3).

use crate::world::WorldData;
use crate::world::unit::{UnitId, UnitRecord};

use super::authored::AuthoredRelationshipCatalog;
use super::compose::assemble_relationship_facets;
use super::facet::RelationshipFacet;
use super::standing::RelationshipStandingStore;

/// Contributing layer for one resolved relationship term.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelationshipContributionLayer {
    Authored,
    Standing,
}

/// One additive term in a directional relationship calculation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipContribution {
    pub source: RelationshipFacet,
    pub target: RelationshipFacet,
    pub value: i32,
    pub layer: RelationshipContributionLayer,
}

/// Structured provenance for `effective_relationship`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipExplanation {
    pub contributions: Vec<RelationshipContribution>,
    pub total: i32,
}

fn sum_contributions(contributions: &[RelationshipContribution]) -> i32 {
    let total: i64 = contributions
        .iter()
        .map(|contribution| i64::from(contribution.value))
        .sum();
    i32::try_from(total).expect("relationship contribution overflow")
}

fn compute_relationship(
    authored: &AuthoredRelationshipCatalog,
    standing: &RelationshipStandingStore,
    observer: &UnitRecord,
    target: &UnitRecord,
) -> RelationshipExplanation {
    let source_facets = assemble_relationship_facets(observer);
    let target_facets = assemble_relationship_facets(target);
    let mut contributions = Vec::new();

    for source in &source_facets {
        for target_facet in &target_facets {
            if let (Some(source_key), Some(target_key)) = (
                source.to_authored_facet_key(),
                target_facet.to_authored_facet_key(),
            ) {
                if let Some(value) = authored.get_edge(&source_key, &target_key) {
                    contributions.push(RelationshipContribution {
                        source: source.clone(),
                        target: target_facet.clone(),
                        value,
                        layer: RelationshipContributionLayer::Authored,
                    });
                }
            }

            let standing_value = standing.get(source, target_facet);
            if standing_value != 0 {
                contributions.push(RelationshipContribution {
                    source: source.clone(),
                    target: target_facet.clone(),
                    value: standing_value,
                    layer: RelationshipContributionLayer::Standing,
                });
            }
        }
    }

    let total = sum_contributions(&contributions);
    RelationshipExplanation {
        contributions,
        total,
    }
}

/// Resolve the directional relationship from observer to target using world identity.
pub fn effective_relationship(
    world: &WorldData,
    authored: &AuthoredRelationshipCatalog,
    standing: &RelationshipStandingStore,
    observer: UnitId,
    target: UnitId,
) -> Option<i32> {
    let observer_record = world.get_unit(observer)?;
    let target_record = world.get_unit(target)?;
    Some(effective_relationship_for_records(
        authored,
        standing,
        observer_record,
        target_record,
    ))
}

/// Resolve the directional relationship between two unit records.
pub fn effective_relationship_for_records(
    authored: &AuthoredRelationshipCatalog,
    standing: &RelationshipStandingStore,
    observer: &UnitRecord,
    target: &UnitRecord,
) -> i32 {
    compute_relationship(authored, standing, observer, target).total
}

/// Explain the directional relationship using the same calculation path as the value resolver.
pub fn explain_relationship(
    world: &WorldData,
    authored: &AuthoredRelationshipCatalog,
    standing: &RelationshipStandingStore,
    observer: UnitId,
    target: UnitId,
) -> Option<RelationshipExplanation> {
    let observer_record = world.get_unit(observer)?;
    let target_record = world.get_unit(target)?;
    Some(explain_relationship_for_records(
        authored,
        standing,
        observer_record,
        target_record,
    ))
}

/// Explain the directional relationship between two unit records.
pub fn explain_relationship_for_records(
    authored: &AuthoredRelationshipCatalog,
    standing: &RelationshipStandingStore,
    observer: &UnitRecord,
    target: &UnitRecord,
) -> RelationshipExplanation {
    compute_relationship(authored, standing, observer, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ownership::UnitOwnership;
    use crate::world::relationship::{
        AuthoredFacetKey, AuthoredRelationshipCatalog, DirectedRelationshipEdgeKey, FactionId,
        SpeciesId,
    };
    use crate::world::{UnitDefinitionId, UnitPlacement, UnitSource, WorldPosition};
    use bevy::prelude::{Quat, Vec3};

    use crate::world::{ChunkCoord, LocalPosition};

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn unit(id: u64, faction: &str, species: &str) -> UnitRecord {
        UnitRecord::new(
            UnitId::new(id),
            UnitDefinitionId::new("test"),
            UnitPlacement::new(pos(0.0, 0.0), Quat::IDENTITY),
            UnitSource::Authored,
            UnitOwnership::wildlife(),
            5,
            FactionId::new(faction),
            SpeciesId::new(species),
        )
    }

    fn authored_catalog(
        edges: &[(AuthoredFacetKey, AuthoredFacetKey, i32)],
    ) -> AuthoredRelationshipCatalog {
        AuthoredRelationshipCatalog::from_edges(edges.iter().cloned().map(
            |(source, target, value)| (DirectedRelationshipEdgeKey::new(source, target), value),
        ))
        .expect("valid authored edges")
    }

    #[test]
    fn directionality_when_contributions_differ() {
        let authored = authored_catalog(&[(
            AuthoredFacetKey::Faction(FactionId::new("wild")),
            AuthoredFacetKey::Faction(FactionId::new("player")),
            -300,
        )]);
        let standing = RelationshipStandingStore::default();
        let wild = unit(1, "wild", "wolf");
        let player = unit(2, "player", "human");
        assert_eq!(
            effective_relationship_for_records(&authored, &standing, &wild, &player),
            -300
        );
        assert_eq!(
            effective_relationship_for_records(&authored, &standing, &player, &wild),
            0
        );
    }

    #[test]
    fn additive_same_domain_contributions_stack() {
        let authored = authored_catalog(&[
            (
                AuthoredFacetKey::Faction(FactionId::new("wild")),
                AuthoredFacetKey::Faction(FactionId::new("player")),
                -100,
            ),
            (
                AuthoredFacetKey::Species(SpeciesId::new("wolf")),
                AuthoredFacetKey::Species(SpeciesId::new("human")),
                -50,
            ),
        ]);
        let standing = RelationshipStandingStore::default();
        let wild = unit(1, "wild", "wolf");
        let player = unit(2, "player", "human");
        assert_eq!(
            effective_relationship_for_records(&authored, &standing, &wild, &player),
            -150
        );
    }

    #[test]
    fn cross_domain_contributions_participate() {
        let authored = authored_catalog(&[(
            AuthoredFacetKey::Faction(FactionId::new("wild")),
            AuthoredFacetKey::Species(SpeciesId::new("human")),
            -80,
        )]);
        let standing = RelationshipStandingStore::default();
        let wild = unit(1, "wild", "wolf");
        let player = unit(2, "player", "human");
        assert_eq!(
            effective_relationship_for_records(&authored, &standing, &wild, &player),
            -80
        );
    }

    #[test]
    fn all_applicable_facets_stack() {
        let authored = authored_catalog(&[(
            AuthoredFacetKey::Faction(FactionId::new("wild")),
            AuthoredFacetKey::Faction(FactionId::new("player")),
            -100,
        )]);
        let mut standing = RelationshipStandingStore::default();
        standing.apply_delta(
            RelationshipFacet::Individual(UnitId::new(1)),
            RelationshipFacet::Individual(UnitId::new(2)),
            150,
        );
        let wild = unit(1, "wild", "wolf");
        let player = unit(2, "player", "human");
        assert_eq!(
            effective_relationship_for_records(&authored, &standing, &wild, &player),
            50
        );
    }

    #[test]
    fn missing_edges_contribute_zero() {
        let authored = AuthoredRelationshipCatalog::default();
        let standing = RelationshipStandingStore::default();
        let a = unit(1, "wild", "wolf");
        let b = unit(2, "player", "human");
        assert_eq!(
            effective_relationship_for_records(&authored, &standing, &a, &b),
            0
        );
    }

    #[test]
    fn personal_standing_is_directional() {
        let authored = AuthoredRelationshipCatalog::default();
        let mut standing = RelationshipStandingStore::default();
        standing.apply_delta(
            RelationshipFacet::Individual(UnitId::new(1)),
            RelationshipFacet::Individual(UnitId::new(2)),
            150,
        );
        let a = unit(1, "wild", "wolf");
        let b = unit(2, "player", "human");
        assert_eq!(
            effective_relationship_for_records(&authored, &standing, &a, &b),
            150
        );
        assert_eq!(
            effective_relationship_for_records(&authored, &standing, &b, &a),
            0
        );
    }

    #[test]
    fn group_standing_targets_faction_and_species() {
        let authored = AuthoredRelationshipCatalog::default();
        let mut standing = RelationshipStandingStore::default();
        standing.apply_delta(
            RelationshipFacet::Faction(FactionId::new("player")),
            RelationshipFacet::Species(SpeciesId::new("wolf")),
            40,
        );
        let player = unit(1, "player", "human");
        let wolf = unit(2, "wild", "wolf");
        assert_eq!(
            effective_relationship_for_records(&authored, &standing, &player, &wolf),
            40
        );
    }

    #[test]
    fn authored_and_standing_on_same_edge_are_additive() {
        let authored = authored_catalog(&[(
            AuthoredFacetKey::Faction(FactionId::new("wild")),
            AuthoredFacetKey::Faction(FactionId::new("player")),
            -100,
        )]);
        let mut standing = RelationshipStandingStore::default();
        standing.apply_delta(
            RelationshipFacet::Faction(FactionId::new("wild")),
            RelationshipFacet::Faction(FactionId::new("player")),
            30,
        );
        let wild = unit(1, "wild", "wolf");
        let player = unit(2, "player", "human");
        assert_eq!(
            effective_relationship_for_records(&authored, &standing, &wild, &player),
            -70
        );
    }

    #[test]
    fn provenance_sum_matches_total() {
        let authored = authored_catalog(&[(
            AuthoredFacetKey::Faction(FactionId::new("wild")),
            AuthoredFacetKey::Faction(FactionId::new("player")),
            -100,
        )]);
        let mut standing = RelationshipStandingStore::default();
        standing.apply_delta(
            RelationshipFacet::Individual(UnitId::new(1)),
            RelationshipFacet::Individual(UnitId::new(2)),
            150,
        );
        let wild = unit(1, "wild", "wolf");
        let player = unit(2, "player", "human");
        let explanation = explain_relationship_for_records(&authored, &standing, &wild, &player);
        let contribution_sum: i32 = explanation
            .contributions
            .iter()
            .map(|contribution| contribution.value)
            .sum();
        assert_eq!(contribution_sum, explanation.total);
        assert_eq!(
            explanation.total,
            effective_relationship_for_records(&authored, &standing, &wild, &player)
        );
    }

    #[test]
    fn explanation_order_is_deterministic() {
        let authored = authored_catalog(&[(
            AuthoredFacetKey::Faction(FactionId::new("wild")),
            AuthoredFacetKey::Faction(FactionId::new("player")),
            -100,
        )]);
        let mut standing = RelationshipStandingStore::default();
        standing.apply_delta(
            RelationshipFacet::Individual(UnitId::new(1)),
            RelationshipFacet::Individual(UnitId::new(2)),
            150,
        );
        let wild = unit(1, "wild", "wolf");
        let player = unit(2, "player", "human");
        let first = explain_relationship_for_records(&authored, &standing, &wild, &player);
        let second = explain_relationship_for_records(&authored, &standing, &wild, &player);
        assert_eq!(first.contributions, second.contributions);
    }
}
