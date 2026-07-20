//! Response Engine (SA3 / ADR-118).
//!
//! Converts need pressures into scored, data-driven CandidateResponses.
//! Never selects a response, generates tasks, or mutates production.

mod candidate;
mod catalog;
mod definition;
mod discover;
mod id;
mod score;
mod starter;
mod step;
mod store;
mod validation;

#[cfg(test)]
mod tests;

pub use candidate::{
    CandidateResponse, ResponseAvailability, ResponseBlockingReason, SettlementResponseCandidates,
};
pub use catalog::ResponseCatalog;
pub use definition::{
    CapabilityRequirement, ExpectedEffect, ResponseDefinition, ResponseType,
};
pub use discover::{discover_settlement_responses, ResponseDiscoveryContext};
pub use id::ResponseId;
pub use score::score_candidate;
pub use starter::starter_response_definitions;
pub use step::{
    discover_settlement_responses_now, step_settlement_response_discovery,
    RESPONSE_DISCOVERY_CADENCE_TICKS,
};
pub use store::ResponseCandidateStore;
pub use validation::{
    validate_candidate, validate_response_catalog_against_needs,
    validate_response_catalog_definitions, validate_response_catalog_definitions_with_needs,
    validate_settlement_response_candidates, ResponseCandidateValidationError, ResponseCatalogError,
};
