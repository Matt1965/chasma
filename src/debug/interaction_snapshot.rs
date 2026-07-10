//! Client-local interaction debug snapshot (REVIEW-A6).
//!
//! Owned by the debug layer — not [`crate::world::WorldData`]. Populated by
//! [`super::interaction_capture::capture_interaction_debug_snapshot`]; overlays read only.

use bevy::prelude::*;

use crate::world::{
    interaction_plan_to_unit_order, query_world_interaction, resolve_interaction_to_order,
    InteractionQueryContext, InteractionResult, UnitOrder, WorldPosition,
};

/// Last interaction query + resolved order for debug tooling (client-local).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct InteractionDebugSnapshot {
    pub query: Option<InteractionResult>,
    pub resolved_order: Option<UnitOrder>,
}

impl InteractionDebugSnapshot {
    pub fn record_query_and_order(&mut self, query: InteractionResult, order: Option<UnitOrder>) {
        self.query = Some(query);
        self.resolved_order = order;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Capture interaction classification from a world click (read-only world access).
pub fn capture_interaction_at_position(
    snapshot: &mut InteractionDebugSnapshot,
    ctx: &InteractionQueryContext<'_>,
    position: WorldPosition,
) {
    let Some(interaction) = query_world_interaction(ctx, position) else {
        snapshot.clear();
        return;
    };
    let plan = resolve_interaction_to_order(&interaction);
    let order = interaction_plan_to_unit_order(plan);
    snapshot.record_query_and_order(interaction, order);
}
