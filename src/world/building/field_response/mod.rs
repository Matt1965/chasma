mod catalog;
mod definition;
mod efficiency;
mod error;
mod evaluate;
mod id;
mod starter;

pub use catalog::{
    FIELD_RESPONSE_PROFILE_CATALOG_RON_PATH, FieldResponseProfileCatalog,
    FieldResponseProfileCatalogRevision, FieldResponseProfileCatalogRon,
    load_field_response_profile_catalog,
};
pub use definition::{
    FieldResponsePoint, FieldResponseProfileDefinition, field_value_from_percent,
    field_value_to_percent_display,
};
pub use efficiency::{
    EFFICIENCY_BASIS_POINTS_ONE_HUNDRED_PERCENT, EfficiencyBasisPoints, MAX_EFFICIENCY_BASIS_POINTS,
};
pub use error::{FieldResponseEvaluationError, FieldResponseProfileError};
pub use evaluate::evaluate_field_response;
pub use id::{FieldResponseProfileId, validate_field_response_profile_id};

#[cfg(any(test, feature = "dev"))]
pub use starter::starter_profiles;
