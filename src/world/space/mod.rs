//! Navigable spaces, portals, and interior support (ADR-083 B6).

mod definition;
mod error;
mod id;
mod portal;
mod profile;
mod registry;
mod support;
mod transition;

#[cfg(test)]
mod tests;

pub use definition::SpaceRecord;
pub use error::SpaceError;
pub use id::{PortalId, SpaceId};
pub use portal::{PortalRecord, PortalType};
pub use profile::{
    PortalTemplate, SpaceTemplate, register_building_space_profile, space_hidden_by_default,
    space_visible_in_view, two_story_hut_profile,
};
pub use registry::SpaceRegistry;
pub use support::{ground_position_in_space, sample_support_height};
pub use transition::{UnitPortalTransitionState, try_portal_transition};

#[cfg(any(test, feature = "dev"))]
pub use profile::two_story_hut_profile as starter_space_profile;
