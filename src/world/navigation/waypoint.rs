use bevy::prelude::*;

use crate::world::{SpaceId, WorldPosition};

/// One grounded navigation sample with space context (ADR-083 B6).
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct NavigationWaypoint {
    pub position: WorldPosition,
    pub space_id: SpaceId,
    pub portal_id: Option<crate::world::PortalId>,
}

impl NavigationWaypoint {
    pub fn surface(position: WorldPosition) -> Self {
        Self {
            position,
            space_id: SpaceId::SURFACE,
            portal_id: None,
        }
    }

    pub fn in_space(position: WorldPosition, space_id: SpaceId) -> Self {
        Self {
            position,
            space_id,
            portal_id: None,
        }
    }

    pub fn portal_transition(
        position: WorldPosition,
        space_id: SpaceId,
        portal_id: crate::world::PortalId,
    ) -> Self {
        Self {
            position,
            space_id,
            portal_id: Some(portal_id),
        }
    }
}
