//! Starting-origin definitions for New Game / squad preview (CG7).

mod catalog;
mod definition;
mod id;
mod starter;

#[cfg(test)]
mod tests;

pub use catalog::OriginCatalog;
pub use definition::{OriginDefinition, OriginRosterMember};
pub use id::OriginId;
pub use starter::{starter_origin_catalog, starter_origin_definitions};
