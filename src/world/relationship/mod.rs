pub mod authored;
mod compose;
mod domain;
mod facet;
pub mod faction;
mod resolve;
pub mod species;
pub mod standing;

pub use authored::{
    AuthoredFacetKey, AuthoredRelationshipCatalog, AuthoredRelationshipCatalogError,
    DirectedRelationshipEdgeKey,
};
pub use compose::{assemble_relationship_facets, faction_display_name, species_display_name};
pub use domain::{MatrixDirection, RelationshipMatrixDomain};
pub use facet::RelationshipFacet;
pub use faction::{FactionCatalog, FactionCatalogError, FactionDefinition, FactionId};
pub use resolve::{
    RelationshipContribution, RelationshipContributionLayer, RelationshipExplanation,
    effective_relationship, effective_relationship_for_records, explain_relationship,
    explain_relationship_for_records,
};
pub use species::{SpeciesCatalog, SpeciesCatalogError, SpeciesDefinition, SpeciesId};
pub use standing::{
    DirectedRelationshipFacetKey, RelationshipStandingSaveState, RelationshipStandingStore,
};
