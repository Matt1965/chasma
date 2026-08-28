//! Sparse mutable Standing layer (ADR-132 Phase 3).

use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::unit::UnitId;

use super::super::facet::RelationshipFacet;
use super::super::faction::FactionId;
use super::super::species::SpeciesId;

/// Directed Standing edge key `(source facet -> target facet)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct DirectedRelationshipFacetKey {
    pub source: RelationshipFacet,
    pub target: RelationshipFacet,
}

impl DirectedRelationshipFacetKey {
    pub fn new(source: RelationshipFacet, target: RelationshipFacet) -> Self {
        Self { source, target }
    }
}

/// Serializable Standing edge for dev-scene persistence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipStandingEdgeSave {
    pub source: RelationshipFacetSave,
    pub target: RelationshipFacetSave,
    pub delta: i32,
}

/// Serializable facet identity for Standing persistence.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RelationshipFacetSave {
    Faction { id: String },
    Species { id: String },
    Individual { unit_id: u64 },
}

impl From<&RelationshipFacet> for RelationshipFacetSave {
    fn from(facet: &RelationshipFacet) -> Self {
        match facet {
            RelationshipFacet::Faction(id) => Self::Faction {
                id: id.as_str().to_string(),
            },
            RelationshipFacet::Species(id) => Self::Species {
                id: id.as_str().to_string(),
            },
            RelationshipFacet::Individual(id) => Self::Individual { unit_id: id.raw() },
        }
    }
}

impl TryFrom<RelationshipFacetSave> for RelationshipFacet {
    type Error = ();

    fn try_from(value: RelationshipFacetSave) -> Result<Self, Self::Error> {
        Ok(match value {
            RelationshipFacetSave::Faction { id } => Self::Faction(FactionId::new(id)),
            RelationshipFacetSave::Species { id } => Self::Species(SpeciesId::new(id)),
            RelationshipFacetSave::Individual { unit_id } => Self::Individual(UnitId::new(unit_id)),
        })
    }
}

/// Persisted Standing store payload.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RelationshipStandingSaveState {
    #[serde(default)]
    pub edges: Vec<RelationshipStandingEdgeSave>,
}

/// Authoritative sparse mutable Standing store (ADR-132 Phase 3).
#[derive(Debug, Clone, Default, Reflect)]
pub struct RelationshipStandingStore {
    edges: HashMap<DirectedRelationshipFacetKey, i32>,
}

impl RelationshipStandingStore {
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Returns the stored Standing delta, or zero when absent.
    pub fn get(&self, source: &RelationshipFacet, target: &RelationshipFacet) -> i32 {
        self.edges
            .get(&DirectedRelationshipFacetKey::new(
                source.clone(),
                target.clone(),
            ))
            .copied()
            .unwrap_or(0)
    }

    /// Applies an additive Standing delta; removes the edge when the result is zero.
    pub fn apply_delta(
        &mut self,
        source: RelationshipFacet,
        target: RelationshipFacet,
        delta: i32,
    ) {
        let key = DirectedRelationshipFacetKey::new(source, target);
        let current = self.edges.get(&key).copied().unwrap_or(0);
        let next = current.checked_add(delta).expect("Standing delta overflow");
        if next == 0 {
            self.edges.remove(&key);
        } else {
            self.edges.insert(key, next);
        }
    }

    pub fn export_save_state(&self) -> RelationshipStandingSaveState {
        let mut edges: Vec<_> = self
            .edges
            .iter()
            .map(|(key, delta)| RelationshipStandingEdgeSave {
                source: RelationshipFacetSave::from(&key.source),
                target: RelationshipFacetSave::from(&key.target),
                delta: *delta,
            })
            .collect();
        edges.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.target.cmp(&right.target))
                .then_with(|| left.delta.cmp(&right.delta))
        });
        RelationshipStandingSaveState { edges }
    }

    pub fn import_save_state(&mut self, state: RelationshipStandingSaveState) {
        self.clear();
        for edge in state.edges {
            let Ok(source) = RelationshipFacet::try_from(edge.source) else {
                continue;
            };
            let Ok(target) = RelationshipFacet::try_from(edge.target) else {
                continue;
            };
            if edge.delta == 0 {
                continue;
            }
            self.edges.insert(
                DirectedRelationshipFacetKey::new(source, target),
                edge.delta,
            );
        }
    }

    pub fn clear(&mut self) {
        self.edges.clear();
    }

    pub fn sorted_edges(&self) -> Vec<(DirectedRelationshipFacetKey, i32)> {
        let mut edges: Vec<_> = self
            .edges
            .iter()
            .map(|(key, delta)| (key.clone(), *delta))
            .collect();
        edges.sort_by(|(left_key, left_delta), (right_key, right_delta)| {
            left_key
                .source
                .cmp(&right_key.source)
                .then_with(|| left_key.target.cmp(&right_key.target))
                .then_with(|| left_delta.cmp(right_delta))
        });
        edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sparse_zero_removes_entry() {
        let mut store = RelationshipStandingStore::default();
        let source = RelationshipFacet::Faction(FactionId::new("player"));
        let target = RelationshipFacet::Faction(FactionId::new("wild"));
        store.apply_delta(source.clone(), target.clone(), 50);
        assert_eq!(store.len(), 1);
        store.apply_delta(source, target, -50);
        assert!(store.is_empty());
    }

    #[test]
    fn group_and_individual_standing_edges_persist() {
        let mut store = RelationshipStandingStore::default();
        store.apply_delta(
            RelationshipFacet::Faction(FactionId::new("player")),
            RelationshipFacet::Species(SpeciesId::new("wolf")),
            25,
        );
        store.apply_delta(
            RelationshipFacet::Individual(UnitId::new(3)),
            RelationshipFacet::Individual(UnitId::new(7)),
            150,
        );
        let save = store.export_save_state();
        let mut restored = RelationshipStandingStore::default();
        restored.import_save_state(save);
        assert_eq!(restored.len(), 2);
        assert_eq!(
            restored.get(
                &RelationshipFacet::Faction(FactionId::new("player")),
                &RelationshipFacet::Species(SpeciesId::new("wolf")),
            ),
            25
        );
        assert_eq!(
            restored.get(
                &RelationshipFacet::Individual(UnitId::new(3)),
                &RelationshipFacet::Individual(UnitId::new(7)),
            ),
            150
        );
    }

    #[test]
    fn save_state_round_trip_is_deterministic() {
        let mut store = RelationshipStandingStore::default();
        store.apply_delta(
            RelationshipFacet::Species(SpeciesId::new("deer")),
            RelationshipFacet::Faction(FactionId::new("wild")),
            -10,
        );
        store.apply_delta(
            RelationshipFacet::Faction(FactionId::new("wild")),
            RelationshipFacet::Species(SpeciesId::new("deer")),
            -20,
        );
        let first = store.export_save_state();
        let second = {
            let mut restored = RelationshipStandingStore::default();
            restored.import_save_state(first.clone());
            restored.export_save_state()
        };
        assert_eq!(first, second);
    }

    #[test]
    fn backward_compatible_empty_save_state() {
        let mut store = RelationshipStandingStore::default();
        store.apply_delta(
            RelationshipFacet::Faction(FactionId::new("wild")),
            RelationshipFacet::Species(SpeciesId::new("deer")),
            5,
        );
        store.import_save_state(RelationshipStandingSaveState::default());
        assert!(store.is_empty());
    }
}
