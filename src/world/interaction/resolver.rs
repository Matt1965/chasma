//! Interaction → [`UnitOrder`] routing (ADR-042 U6).
//!
//! Produces orders only — movement/pathfinding remain authoritative downstream.

use crate::world::{UnitId, UnitOrder, WorldData, WorldPosition};

use super::query::{query_world_interaction, InteractionQueryContext};
use super::types::{InteractionResult, InteractionType};

/// Resolved interaction outcome before per-unit issuance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InteractionOrderPlan {
    MoveTo { target: WorldPosition },
    NoOp,
}

/// Inputs for click / command resolution.
#[derive(Debug, Clone)]
pub struct InteractionResolveContext<'a> {
    pub query: InteractionQueryContext<'a>,
    pub selected_units: &'a [UnitId],
}

impl<'a> InteractionResolveContext<'a> {
    pub fn new(
        world: &'a WorldData,
        doodad_catalog: &'a crate::world::DoodadCatalog,
        unit_catalog: &'a crate::world::UnitCatalog,
        selected_units: &'a [UnitId],
    ) -> Self {
        Self {
            query: InteractionQueryContext::new(world, doodad_catalog, unit_catalog),
            selected_units,
        }
    }
}

/// Map a classified interaction to an order plan (no gameplay execution).
pub fn resolve_interaction_to_order(interaction: &InteractionResult) -> InteractionOrderPlan {
    match interaction.interaction_type {
        InteractionType::MoveTarget | InteractionType::TerrainPoint => {
            if interaction.valid {
                InteractionOrderPlan::MoveTo {
                    target: interaction.position,
                }
            } else {
                InteractionOrderPlan::NoOp
            }
        }
        InteractionType::ResourceNode | InteractionType::InteractableObject => {
            // Placeholder — move adjacent until harvest/interact exists.
            InteractionOrderPlan::MoveTo {
                target: interaction.position,
            }
        }
        InteractionType::BlockedArea | InteractionType::None => InteractionOrderPlan::NoOp,
    }
}

/// Query and resolve a terrain/world click for the current selection.
pub fn resolve_world_click_to_order(
    ctx: &InteractionResolveContext<'_>,
    position: WorldPosition,
) -> Option<InteractionOrderPlan> {
    if ctx.selected_units.is_empty() {
        return None;
    }

    let interaction = query_world_interaction(&ctx.query, position)?;
    Some(resolve_interaction_to_order(&interaction))
}

/// Query and resolve a unit-target click (uses unit placement as query origin).
pub fn resolve_unit_click_to_order(
    ctx: &InteractionResolveContext<'_>,
    target_unit: UnitId,
) -> Option<InteractionOrderPlan> {
    if ctx.selected_units.is_empty() {
        return None;
    }

    let position = ctx
        .query
        .world
        .get_unit(target_unit)
        .map(|record| record.placement.position)?;

    let interaction = query_world_interaction(&ctx.query, position)?;
    Some(resolve_interaction_to_order(&interaction))
}

/// Convert a plan into the authoritative [`UnitOrder`] enum.
pub fn interaction_plan_to_unit_order(plan: InteractionOrderPlan) -> Option<UnitOrder> {
    match plan {
        InteractionOrderPlan::MoveTo { target } => Some(UnitOrder::MoveTo { target }),
        InteractionOrderPlan::NoOp => None,
    }
}

/// Full pipeline: world click → optional [`UnitOrder`].
pub fn resolve_world_click_to_unit_order(
    ctx: &InteractionResolveContext<'_>,
    position: WorldPosition,
) -> Option<UnitOrder> {
    resolve_world_click_to_order(ctx, position).and_then(interaction_plan_to_unit_order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_doodad, create_unit, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
        DoodadDefinitionId, DoodadPlacementOverrides, DoodadSource, Heightfield, LocalPosition,
        UnitDefinitionId, UnitSource,
    };
    use bevy::prelude::Vec3;

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(layout());
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    #[test]
    fn resolver_produces_move_to_for_move_target() {
        let world = flat_world();
        let catalog = crate::world::DoodadCatalog::default();
        let unit_catalog = crate::world::UnitCatalog::default();
        let units = [UnitId::new(1)];
        let ctx = InteractionResolveContext::new(&world, &catalog, &unit_catalog, &units);
        let plan = resolve_world_click_to_order(&ctx, pos(40.0, 40.0)).unwrap();
        assert!(matches!(plan, InteractionOrderPlan::MoveTo { .. }));
        let order = interaction_plan_to_unit_order(plan).unwrap();
        assert!(matches!(order, UnitOrder::MoveTo { .. }));
    }

    #[test]
    fn blocked_area_produces_no_op() {
        let catalog = crate::world::DoodadCatalog::default();
        let mut world = flat_world();
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            pos(50.0, 50.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();
        let unit_catalog = crate::world::UnitCatalog::default();
        let units = [UnitId::new(1)];
        let ctx = InteractionResolveContext::new(&world, &catalog, &unit_catalog, &units);
        let plan = resolve_world_click_to_order(&ctx, pos(50.0, 50.0)).unwrap();
        assert_eq!(plan, InteractionOrderPlan::NoOp);
        assert!(interaction_plan_to_unit_order(plan).is_none());
    }

    #[test]
    fn resource_node_placeholder_maps_to_move_to() {
        let catalog = crate::world::DoodadCatalog::default();
        let mut world = flat_world();
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("resource_node_iron"),
            pos(60.0, 60.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();
        let unit_catalog = crate::world::UnitCatalog::default();
        let units = [UnitId::new(1)];
        let ctx = InteractionResolveContext::new(&world, &catalog, &unit_catalog, &units);
        let plan = resolve_world_click_to_order(&ctx, pos(60.0, 60.0)).unwrap();
        assert!(matches!(plan, InteractionOrderPlan::MoveTo { .. }));
    }

    #[test]
    fn empty_selection_returns_none() {
        let world = flat_world();
        let catalog = crate::world::DoodadCatalog::default();
        let unit_catalog = crate::world::UnitCatalog::default();
        let ctx = InteractionResolveContext::new(&world, &catalog, &unit_catalog, &[]);
        assert!(resolve_world_click_to_order(&ctx, pos(1.0, 1.0)).is_none());
    }

    #[test]
    fn multi_unit_selection_uses_same_resolution() {
        let world = flat_world();
        let catalog = crate::world::DoodadCatalog::default();
        let unit_catalog = crate::world::UnitCatalog::default();
        let one = [UnitId::new(1)];
        let many = [UnitId::new(1), UnitId::new(2), UnitId::new(3)];
        let ctx_one = InteractionResolveContext::new(&world, &catalog, &unit_catalog, &one);
        let ctx_many = InteractionResolveContext::new(&world, &catalog, &unit_catalog, &many);
        let a = resolve_world_click_to_order(&ctx_one, pos(20.0, 20.0));
        let b = resolve_world_click_to_order(&ctx_many, pos(20.0, 20.0));
        assert_eq!(a, b);
    }

    #[test]
    fn unit_click_resolves_from_unit_position() {
        let unit_catalog = crate::world::UnitCatalog::default();
        let catalog = crate::world::DoodadCatalog::default();
        let mut world = flat_world();
        let unit_id = create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(15.0, 15.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let selected = [UnitId::new(99)];
        let ctx = InteractionResolveContext::new(&world, &catalog, &unit_catalog, &selected);
        let plan = resolve_unit_click_to_order(&ctx, unit_id).unwrap();
        assert!(matches!(plan, InteractionOrderPlan::MoveTo { .. }));
    }
}
