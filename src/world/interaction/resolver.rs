//! Interaction → [`UnitOrder`] routing (ADR-042 U6).
//!
//! Produces orders only — movement/pathfinding remain authoritative downstream.

use crate::world::combat::{AttackTargetingPolicy, classify_unit_target};
use crate::world::{UnitId, UnitOrder, WorldData, WorldPosition};

use super::query::{InteractionQueryContext, query_world_interaction};
use super::types::{InteractionResult, InteractionTargetRef, InteractionType};

/// Resolved interaction outcome before per-unit issuance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InteractionOrderPlan {
    MoveTo { target: WorldPosition },
    Attack { target: UnitId },
    AttackMove { destination: WorldPosition },
    NoOp,
}

/// Inputs for click / command resolution.
#[derive(Debug, Clone)]
pub struct InteractionResolveContext<'a> {
    pub query: InteractionQueryContext<'a>,
    pub selected_units: &'a [UnitId],
    pub targeting_policy: AttackTargetingPolicy,
}

impl<'a> InteractionResolveContext<'a> {
    pub fn new(
        world: &'a WorldData,
        doodad_catalog: &'a crate::world::DoodadCatalog,
        unit_catalog: &'a crate::world::UnitCatalog,
        weapon_catalog: &'a crate::world::WeaponCatalog,
        selected_units: &'a [UnitId],
    ) -> Self {
        Self {
            query: InteractionQueryContext::new(
                world,
                doodad_catalog,
                unit_catalog,
                weapon_catalog,
            ),
            selected_units,
            targeting_policy: AttackTargetingPolicy::default(),
        }
    }

    pub fn with_targeting_policy(mut self, policy: AttackTargetingPolicy) -> Self {
        self.targeting_policy = policy;
        self
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
            InteractionOrderPlan::NoOp
        }
        InteractionType::AttackableUnit => match interaction.target {
            InteractionTargetRef::Unit(target) => InteractionOrderPlan::Attack { target },
            _ => InteractionOrderPlan::NoOp,
        },
        InteractionType::FriendlyUnit | InteractionType::NeutralUnit => {
            if interaction.target.is_unit() {
                InteractionOrderPlan::MoveTo {
                    target: interaction.position,
                }
            } else {
                InteractionOrderPlan::NoOp
            }
        }
        InteractionType::BlockedArea | InteractionType::None => InteractionOrderPlan::NoOp,
    }
}

impl InteractionTargetRef {
    fn is_unit(self) -> bool {
        matches!(self, Self::Unit(_))
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

    let attacker = *ctx.selected_units.first()?;
    let interaction_type = classify_unit_target(
        ctx.query.world,
        attacker,
        target_unit,
        ctx.query.weapon_catalog,
        ctx.query.unit_catalog,
        ctx.targeting_policy,
    );
    let interaction = InteractionResult {
        interaction_type,
        position,
        metadata: super::types::InteractionMetadata {
            label: interaction_type.label().to_string(),
            doodad_kind: None,
            blocks_movement: false,
        },
        valid: true,
        target: InteractionTargetRef::Unit(target_unit),
    };
    Some(resolve_interaction_to_order(&interaction))
}

/// Convert a plan into the authoritative [`UnitOrder`] enum.
pub fn interaction_plan_to_unit_order(plan: InteractionOrderPlan) -> Option<UnitOrder> {
    match plan {
        InteractionOrderPlan::MoveTo { target } => Some(UnitOrder::MoveTo { target }),
        InteractionOrderPlan::Attack { target } => Some(UnitOrder::Attack { target }),
        InteractionOrderPlan::AttackMove { destination } => {
            Some(UnitOrder::AttackMove { destination })
        }
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
    use crate::world::interaction::types::{
        InteractionMetadata, InteractionTargetRef, InteractionType,
    };
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadDefinitionId, DoodadPlacementOverrides,
        DoodadSource, Heightfield, LocalPosition, UnitDefinitionId, UnitSource, WorldData,
        create_doodad, create_unit,
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

    fn weapons() -> crate::world::WeaponCatalog {
        crate::world::WeaponCatalog::default()
    }

    fn resolve_ctx<'a>(
        world: &'a WorldData,
        catalog: &'a crate::world::DoodadCatalog,
        unit_catalog: &'a crate::world::UnitCatalog,
        weapon_catalog: &'a crate::world::WeaponCatalog,
        selected: &'a [UnitId],
    ) -> InteractionResolveContext<'a> {
        InteractionResolveContext::new(world, catalog, unit_catalog, weapon_catalog, selected)
    }

    #[test]
    fn resolver_produces_move_to_for_move_target() {
        let world = flat_world();
        let catalog = crate::world::DoodadCatalog::default();
        let unit_catalog = crate::world::UnitCatalog::default();
        let weapon_catalog = weapons();
        let units = [UnitId::new(1)];
        let ctx = resolve_ctx(&world, &catalog, &unit_catalog, &weapon_catalog, &units);
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
        let weapon_catalog = weapons();
        let ctx = resolve_ctx(&world, &catalog, &unit_catalog, &weapon_catalog, &units);
        let plan = resolve_world_click_to_order(&ctx, pos(50.0, 50.0)).unwrap();
        assert_eq!(plan, InteractionOrderPlan::NoOp);
        assert!(interaction_plan_to_unit_order(plan).is_none());
    }

    #[test]
    fn resource_node_produces_no_op_until_interact_exists() {
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
        let weapon_catalog = weapons();
        let ctx = resolve_ctx(&world, &catalog, &unit_catalog, &weapon_catalog, &units);
        let plan = resolve_world_click_to_order(&ctx, pos(60.0, 60.0)).unwrap();
        assert_eq!(plan, InteractionOrderPlan::NoOp);
        assert!(interaction_plan_to_unit_order(plan).is_none());
    }

    #[test]
    fn attackable_unit_without_target_produces_no_op() {
        let world = flat_world();
        let catalog = crate::world::DoodadCatalog::default();
        let unit_catalog = crate::world::UnitCatalog::default();
        let units = [UnitId::new(1)];
        let weapon_catalog = weapons();
        let interaction = InteractionResult {
            interaction_type: InteractionType::AttackableUnit,
            position: pos(10.0, 10.0),
            metadata: InteractionMetadata {
                label: "Attack".to_string(),
                doodad_kind: None,
                blocks_movement: false,
            },
            valid: true,
            target: InteractionTargetRef::None,
        };
        let plan = resolve_interaction_to_order(&interaction);
        assert_eq!(plan, InteractionOrderPlan::NoOp);
    }

    #[test]
    fn empty_selection_returns_none() {
        let world = flat_world();
        let catalog = crate::world::DoodadCatalog::default();
        let unit_catalog = crate::world::UnitCatalog::default();
        let weapon_catalog = weapons();
        let ctx = resolve_ctx(&world, &catalog, &unit_catalog, &weapon_catalog, &[]);
        assert!(resolve_world_click_to_order(&ctx, pos(1.0, 1.0)).is_none());
    }

    #[test]
    fn multi_unit_selection_uses_same_resolution() {
        let world = flat_world();
        let catalog = crate::world::DoodadCatalog::default();
        let unit_catalog = crate::world::UnitCatalog::default();
        let one = [UnitId::new(1)];
        let many = [UnitId::new(1), UnitId::new(2), UnitId::new(3)];
        let weapon_catalog = weapons();
        let ctx_one = resolve_ctx(&world, &catalog, &unit_catalog, &weapon_catalog, &one);
        let ctx_many = resolve_ctx(&world, &catalog, &unit_catalog, &weapon_catalog, &many);
        let a = resolve_world_click_to_order(&ctx_one, pos(20.0, 20.0));
        let b = resolve_world_click_to_order(&ctx_many, pos(20.0, 20.0));
        assert_eq!(a, b);
    }

    #[test]
    fn unit_click_on_hostile_resolves_to_attack() {
        use crate::world::{UnitOwnership, create_unit_with_ownership};
        let unit_catalog = crate::world::UnitCatalog::default();
        let catalog = crate::world::DoodadCatalog::default();
        let mut world = flat_world();
        let player = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let hostile = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(15.0, 15.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap()
        .id;
        let selected = [player];
        let weapon_catalog = weapons();
        let ctx = resolve_ctx(&world, &catalog, &unit_catalog, &weapon_catalog, &selected);
        let plan = resolve_unit_click_to_order(&ctx, hostile).unwrap();
        assert!(matches!(plan, InteractionOrderPlan::Attack { target } if target == hostile));
    }

    #[test]
    fn unit_click_on_friendly_resolves_to_move() {
        use crate::world::{UnitOwnership, create_unit_with_ownership};
        let unit_catalog = crate::world::UnitCatalog::default();
        let catalog = crate::world::DoodadCatalog::default();
        let mut world = flat_world();
        let player = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let friendly = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(15.0, 15.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let selected = [player];
        let weapon_catalog = weapons();
        let ctx = resolve_ctx(&world, &catalog, &unit_catalog, &weapon_catalog, &selected);
        let plan = resolve_unit_click_to_order(&ctx, friendly).unwrap();
        assert!(matches!(plan, InteractionOrderPlan::MoveTo { .. }));
    }
}
