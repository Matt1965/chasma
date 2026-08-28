use std::collections::HashMap;

use bevy::prelude::*;

use super::edge::{AuthoredFacetKey, DirectedRelationshipEdgeKey};

/// Sparse authored Disposition-layer relationship store (ADR-132 Phase 2).
///
/// Missing edges are not stored; consumers treat absence as zero contribution.
#[derive(Debug, Clone, Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct AuthoredRelationshipCatalog {
    edges: HashMap<DirectedRelationshipEdgeKey, i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthoredRelationshipCatalogError {
    DuplicateEdge(DirectedRelationshipEdgeKey),
}

impl std::fmt::Display for AuthoredRelationshipCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateEdge(key) => {
                write!(f, "duplicate authored edge {}", key.prose_direction())
            }
        }
    }
}

impl AuthoredRelationshipCatalog {
    pub fn from_edges(
        edges: impl IntoIterator<Item = (DirectedRelationshipEdgeKey, i32)>,
    ) -> Result<Self, AuthoredRelationshipCatalogError> {
        let mut map = HashMap::new();
        for (key, value) in edges {
            if map.insert(key.clone(), value).is_some() {
                return Err(AuthoredRelationshipCatalogError::DuplicateEdge(key));
            }
        }
        Ok(Self { edges: map })
    }

    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Returns the stored authored contribution, if any.
    pub fn get_edge(&self, source: &AuthoredFacetKey, target: &AuthoredFacetKey) -> Option<i32> {
        self.edges
            .get(&DirectedRelationshipEdgeKey::new(
                source.clone(),
                target.clone(),
            ))
            .copied()
    }

    pub fn sorted_edges(&self) -> Vec<(DirectedRelationshipEdgeKey, i32)> {
        let mut edges: Vec<_> = self
            .edges
            .iter()
            .map(|(key, value)| (key.clone(), *value))
            .collect();
        edges.sort_by(|(left_key, left_value), (right_key, right_value)| {
            left_key
                .source
                .prose_id()
                .cmp(&right_key.source.prose_id())
                .then_with(|| left_key.target.prose_id().cmp(&right_key.target.prose_id()))
                .then_with(|| left_value.cmp(right_value))
        });
        edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::relationship::{FactionId, SpeciesId};

    #[test]
    fn missing_edge_is_not_stored() {
        let catalog = AuthoredRelationshipCatalog::default();
        assert_eq!(
            catalog.get_edge(
                &AuthoredFacetKey::Faction(FactionId::new("wild")),
                &AuthoredFacetKey::Faction(FactionId::new("player")),
            ),
            None
        );
    }

    #[test]
    fn stores_directional_edges() {
        let key = DirectedRelationshipEdgeKey::new(
            AuthoredFacetKey::Faction(FactionId::new("wild")),
            AuthoredFacetKey::Faction(FactionId::new("player")),
        );
        let catalog =
            AuthoredRelationshipCatalog::from_edges([(key.clone(), -300)]).expect("valid edges");
        assert_eq!(catalog.get_edge(&key.source, &key.target), Some(-300));
        assert_eq!(
            catalog.get_edge(&key.target, &key.source),
            None,
            "reverse direction is independent"
        );
    }

    #[test]
    fn duplicate_edge_rejected_at_build() {
        let key = DirectedRelationshipEdgeKey::new(
            AuthoredFacetKey::Species(SpeciesId::new("wolf")),
            AuthoredFacetKey::Species(SpeciesId::new("deer")),
        );
        let err = AuthoredRelationshipCatalog::from_edges([(key.clone(), -100), (key, -100)])
            .unwrap_err();
        assert!(matches!(
            err,
            AuthoredRelationshipCatalogError::DuplicateEdge(_)
        ));
    }
}
