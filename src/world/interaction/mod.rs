//! World interaction query layer (ADR-042 U6).
//!
//! `WorldData` → interaction query → [`UnitOrder`] plan — not ECS-driven.

mod query;
mod resolver;
mod types;

pub use query::{
    query_world_interaction, InteractionQueryContext, DEFAULT_INTERACTION_AGENT_RADIUS_METERS,
    DEFAULT_INTERACTION_MAX_SLOPE_DEGREES, DEFAULT_INTERACTION_QUERY_RADIUS_METERS,
};
pub use resolver::{
    interaction_plan_to_unit_order, resolve_interaction_to_order, resolve_unit_click_to_order,
    resolve_world_click_to_order, resolve_world_click_to_unit_order, InteractionOrderPlan,
    InteractionResolveContext,
};
pub use types::{
    InteractionMetadata, InteractionResult, InteractionTargetRef, InteractionType,
};

use bevy::prelude::*;

use crate::world::WorldPosition;

/// Last interaction query + resolved order for debug overlays (read-only hook).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct InteractionDebugSnapshot {
    pub query: Option<InteractionResult>,
    pub resolved_order: Option<crate::world::UnitOrder>,
}

impl InteractionDebugSnapshot {
    pub fn record_query_and_order(
        &mut self,
        query: InteractionResult,
        order: Option<crate::world::UnitOrder>,
    ) {
        self.query = Some(query);
        self.resolved_order = order;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Record debug snapshot from a world click resolution (optional hook).
pub fn record_interaction_debug_from_click(
    snapshot: &mut InteractionDebugSnapshot,
    ctx: &InteractionResolveContext<'_>,
    position: WorldPosition,
) {
    let Some(interaction) = query_world_interaction(&ctx.query, position) else {
        snapshot.clear();
        return;
    };
    let plan = resolve_interaction_to_order(&interaction);
    let order = interaction_plan_to_unit_order(plan);
    snapshot.record_query_and_order(interaction, order);
}
