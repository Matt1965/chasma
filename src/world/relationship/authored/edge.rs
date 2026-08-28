use bevy::prelude::*;

use super::super::domain::RelationshipMatrixDomain;
use super::super::faction::FactionId;
use super::super::species::SpeciesId;

/// Catalog-backed facet identity storable in the authored Disposition layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub enum AuthoredFacetKey {
    Faction(FactionId),
    Species(SpeciesId),
}

impl AuthoredFacetKey {
    pub fn domain(&self) -> RelationshipMatrixDomain {
        match self {
            Self::Faction(_) => RelationshipMatrixDomain::Faction,
            Self::Species(_) => RelationshipMatrixDomain::Species,
        }
    }

    pub fn prose_id(&self) -> String {
        match self {
            Self::Faction(id) => id.as_str().to_string(),
            Self::Species(id) => id.as_str().to_string(),
        }
    }
}

/// Sparse directed authored edge key `(source facet -> target facet)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub struct DirectedRelationshipEdgeKey {
    pub source: AuthoredFacetKey,
    pub target: AuthoredFacetKey,
}

impl DirectedRelationshipEdgeKey {
    pub fn new(source: AuthoredFacetKey, target: AuthoredFacetKey) -> Self {
        Self { source, target }
    }

    pub fn prose_direction(&self) -> String {
        format!(
            "{} \"{}\" -> {} \"{}\"",
            self.source.domain().label(),
            self.source.prose_id(),
            self.target.domain().label(),
            self.target.prose_id(),
        )
    }
}
