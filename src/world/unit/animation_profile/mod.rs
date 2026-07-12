//! Animation profile catalog (A1) — data-only locomotion clip mapping.

mod catalog;
mod definition;
mod id;
mod starter;

pub use catalog::{AnimationProfileCatalog, AnimationProfileCatalogError};
pub use definition::{AnimationClipKey, AnimationProfile};
pub use id::AnimationProfileId;
#[cfg(any(test, feature = "dev"))]
pub use starter::starter_definitions;
