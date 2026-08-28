use bevy::prelude::*;

use crate::world::unit::UnitId;

use super::authored::AuthoredFacetKey;
use super::faction::FactionId;
use super::species::SpeciesId;

/// One relationship-relevant identity facet (ADR-132 Phase 1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub enum RelationshipFacet {
    Faction(FactionId),
    Species(SpeciesId),
    Individual(UnitId),
}

impl RelationshipFacet {
    /// Maps catalog-backed facets into the authored Disposition layer.
    ///
    /// Individual facets have no authored matrix contribution.
    pub fn to_authored_facet_key(&self) -> Option<AuthoredFacetKey> {
        match self {
            Self::Faction(id) => Some(AuthoredFacetKey::Faction(id.clone())),
            Self::Species(id) => Some(AuthoredFacetKey::Species(id.clone())),
            Self::Individual(_) => None,
        }
    }
}
