use bevy::prelude::*;

use super::id::{PortalId, SpaceId};

/// Structured space/portal failures (ADR-083 B6).
#[derive(Debug, Clone, PartialEq)]
pub enum SpaceError {
    MissingSpace(SpaceId),
    DisabledSpace(SpaceId),
    MissingPortal(PortalId),
    InvalidPortalEndpoint {
        portal_id: PortalId,
        reason: &'static str,
    },
    PortalNotBidirectional(PortalId),
    PortalTransitionMismatch {
        portal_id: PortalId,
        expected_space: SpaceId,
        found_space: SpaceId,
    },
    SpaceSupportUnavailable(SpaceId),
    InvalidVisibilityGroup(u32),
    MissingSceneSpaceTag(String),
    CrossSpacePathUnavailable {
        from: SpaceId,
        to: SpaceId,
    },
    UnitSpaceMismatch {
        unit_id: crate::world::UnitId,
        expected: SpaceId,
        found: SpaceId,
    },
}
