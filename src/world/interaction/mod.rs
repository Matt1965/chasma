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
