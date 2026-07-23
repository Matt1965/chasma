use bevy::prelude::*;

use super::id::{PortalId, SpaceId};
use super::portal::PortalRecord;
use super::registry::SpaceRegistry;
use crate::world::{ChunkLayout, WorldPosition};

/// Per-unit portal hysteresis to prevent oscillation (ADR-083 B6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitPortalTransitionState {
    pub lockout_portal: Option<PortalId>,
}

/// Authoritative portal transition when agent enters a portal region.
pub fn try_portal_transition(
    space_registry: &SpaceRegistry,
    layout: ChunkLayout,
    current_space: SpaceId,
    agent_position: WorldPosition,
    transition_state: &mut UnitPortalTransitionState,
    preferred_portal: Option<PortalId>,
) -> Option<(SpaceId, WorldPosition, PortalId)> {
    let agent_global = agent_position.to_global(layout);
    let agent_xz = Vec2::new(agent_global.x, agent_global.z);

    let mut candidates: Vec<&PortalRecord> = space_registry
        .sorted_portals_from_space(current_space)
        .into_iter()
        .filter(|portal| portal.from_space == current_space || portal.bidirectional)
        .collect();

    if let Some(preferred) = preferred_portal {
        candidates.sort_by_key(|portal| {
            if portal.id == preferred {
                0
            } else {
                1
            }
        });
    }

    for portal in &mut candidates {
        if transition_state.lockout_portal == Some(portal.id) {
            if !portal.contains_agent_global(agent_xz) {
                transition_state.lockout_portal = None;
            } else {
                continue;
            }
        }

        let (dest_space, dest_position) = if portal.from_space == current_space {
            (portal.to_space, portal.to_position)
        } else if portal.bidirectional {
            (portal.from_space, portal.to_position)
        } else {
            continue;
        };

        if !portal.contains_agent_global(agent_xz) {
            continue;
        }

        transition_state.lockout_portal = Some(portal.id);
        return Some((dest_space, dest_position, portal.id));
    }
    None
}
