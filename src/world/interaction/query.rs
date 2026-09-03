//! Unified world interaction query (ADR-042 U6).
//!
//! Composes terrain heightfield queries and doodad obstacle queries without
//! merging those systems.

use bevy::prelude::*;

use crate::world::{
    BuildingCatalog, BuildingId, BuildingInteractionProfileCatalog, ChunkCoord, ChunkId,
    DoodadCatalog, DoodadDefinition, DoodadId, DoodadKind, DoodadRecord, FootprintCatalog,
    FootprintSpec, ItemPileSettings, PassabilityAgent, PassabilityCatalogs, PassabilityResult,
    SlopeWalkability, UnitCatalog, WorldData, WorldItemPileRecord, WorldPosition,
    building_accepts_workstation_use, building_is_constructible, classify_slope_walkability,
    ground_world_position, interior_navigation_move_target_at_position,
    nearest_item_pile_at_position, query_passability_at, resolve_navigation_space_at_position,
    unit_may_work_on_building,
};

use super::types::{InteractionMetadata, InteractionResult, InteractionTargetRef, InteractionType};

/// Default query radius for “under cursor” classification (meters).
pub const DEFAULT_INTERACTION_QUERY_RADIUS_METERS: f32 = 2.5;

/// Default agent footprint radius when classifying blocked areas.
pub const DEFAULT_INTERACTION_AGENT_RADIUS_METERS: f32 = 0.5;

/// Default max slope for walkable move targets (degrees).
pub const DEFAULT_INTERACTION_MAX_SLOPE_DEGREES: f32 = 40.0;

/// Inputs for [`query_world_interaction`].
#[derive(Debug, Clone, Copy)]
pub struct InteractionQueryContext<'a> {
    pub world: &'a WorldData,
    pub doodad_catalog: &'a DoodadCatalog,
    pub building_catalog: &'a BuildingCatalog,
    pub footprint_catalog: &'a FootprintCatalog,
    pub interaction_catalog: &'a BuildingInteractionProfileCatalog,
    pub unit_catalog: &'a UnitCatalog,
    pub weapon_catalog: &'a crate::world::WeaponCatalog,
    pub pile_settings: &'a ItemPileSettings,
    pub query_radius_meters: f32,
    pub agent_radius_meters: f32,
    pub max_slope_degrees: f32,
}

impl<'a> InteractionQueryContext<'a> {
    pub fn new(
        world: &'a WorldData,
        doodad_catalog: &'a DoodadCatalog,
        building_catalog: &'a BuildingCatalog,
        footprint_catalog: &'a FootprintCatalog,
        interaction_catalog: &'a BuildingInteractionProfileCatalog,
        unit_catalog: &'a UnitCatalog,
        weapon_catalog: &'a crate::world::WeaponCatalog,
        pile_settings: &'a ItemPileSettings,
    ) -> Self {
        Self {
            world,
            doodad_catalog,
            building_catalog,
            footprint_catalog,
            interaction_catalog,
            unit_catalog,
            weapon_catalog,
            pile_settings,
            query_radius_meters: DEFAULT_INTERACTION_QUERY_RADIUS_METERS,
            agent_radius_meters: DEFAULT_INTERACTION_AGENT_RADIUS_METERS,
            max_slope_degrees: DEFAULT_INTERACTION_MAX_SLOPE_DEGREES,
        }
    }
}

/// Classify the world at `position` within `query_radius_meters`.
///
/// Read-only — never mutates [`WorldData`].
pub fn query_world_interaction(
    ctx: &InteractionQueryContext<'_>,
    position: WorldPosition,
) -> Option<InteractionResult> {
    let Some(grounded) = ground_world_position(ctx.world, position) else {
        return None;
    };

    if interior_navigation_move_target_at_position(
        ctx.world.building_navigation_runtime(),
        ctx.world.space_registry(),
        ctx.world.layout(),
        grounded,
    )
    .is_some()
    {
        return Some(InteractionResult {
            interaction_type: InteractionType::MoveTarget,
            position: grounded,
            metadata: InteractionMetadata {
                label: "Move interior".to_string(),
                doodad_kind: None,
                blocks_movement: false,
            },
            valid: true,
            target: InteractionTargetRef::Terrain(grounded),
        });
    }

    // Candidate cascade (ADR-042 / ADR-090): interior → building → item pile → doodad → terrain.

    if let Some((building_id, record, definition)) =
        nearest_building_in_radius(ctx, grounded, ctx.query_radius_meters)
    {
        return Some(classify_building_hit(
            ctx,
            grounded,
            building_id,
            record,
            definition,
        ));
    }

    let space_id = resolve_navigation_space_at_position(
        ctx.world.building_navigation_runtime(),
        ctx.world.space_registry(),
        ctx.world.layout(),
        grounded,
    );
    if let Some(pile) =
        nearest_item_pile_at_position(ctx.world, grounded, space_id, ctx.pile_settings)
    {
        return Some(classify_pile_hit(grounded, pile));
    }

    if let Some((record, definition)) =
        nearest_doodad_in_radius(ctx, grounded, ctx.query_radius_meters)
    {
        return Some(classify_doodad_hit(grounded, record, definition));
    }

    if !matches!(
        query_passability_at(
            ctx.world,
            PassabilityCatalogs {
                doodad: ctx.doodad_catalog,
                building: ctx.building_catalog,
                footprint: ctx.footprint_catalog,
            },
            grounded,
            PassabilityAgent {
                radius_meters: ctx.agent_radius_meters,
                max_slope_degrees: ctx.max_slope_degrees,
            },
        ),
        PassabilityResult::Passable { .. }
    ) {
        return Some(InteractionResult {
            interaction_type: InteractionType::BlockedArea,
            position: grounded,
            metadata: InteractionMetadata {
                label: "Blocked".to_string(),
                doodad_kind: None,
                blocks_movement: true,
            },
            valid: true,
            target: InteractionTargetRef::Terrain(grounded),
        });
    }

    match classify_slope_walkability(ctx.world, grounded, ctx.max_slope_degrees) {
        SlopeWalkability::Walkable => {}
        SlopeWalkability::TooSteep => {
            return Some(InteractionResult {
                interaction_type: InteractionType::BlockedArea,
                position: grounded,
                metadata: InteractionMetadata {
                    label: "Unwalkable terrain".to_string(),
                    doodad_kind: None,
                    blocks_movement: true,
                },
                valid: true,
                target: InteractionTargetRef::Terrain(grounded),
            });
        }
        SlopeWalkability::Unavailable => {
            return Some(InteractionResult {
                interaction_type: InteractionType::BlockedArea,
                position: grounded,
                metadata: InteractionMetadata {
                    label: "Terrain unavailable".to_string(),
                    doodad_kind: None,
                    blocks_movement: true,
                },
                valid: false,
                target: InteractionTargetRef::Terrain(grounded),
            });
        }
    }

    Some(InteractionResult {
        interaction_type: InteractionType::MoveTarget,
        position: grounded,
        metadata: InteractionMetadata {
            label: "Move".to_string(),
            doodad_kind: None,
            blocks_movement: false,
        },
        valid: true,
        target: InteractionTargetRef::Terrain(grounded),
    })
}

fn classify_pile_hit(grounded: WorldPosition, pile: &WorldItemPileRecord) -> InteractionResult {
    let label = pile_item_label(pile);
    InteractionResult {
        interaction_type: InteractionType::ItemPile,
        position: grounded,
        metadata: InteractionMetadata {
            label,
            doodad_kind: None,
            blocks_movement: false,
        },
        valid: true,
        target: InteractionTargetRef::ItemPile(pile.id),
    }
}

fn pile_item_label(pile: &WorldItemPileRecord) -> String {
    use crate::world::WorldPileContents;
    match &pile.contents {
        WorldPileContents::Stack {
            item_definition_id,
            quantity,
        } => format!("{} x{quantity}", item_definition_id.as_str()),
        WorldPileContents::Unique { .. } => "Item pile".to_string(),
    }
}

fn classify_doodad_hit(
    grounded: WorldPosition,
    record: &DoodadRecord,
    definition: &DoodadDefinition,
) -> InteractionResult {
    let interaction_type = if definition.kind == DoodadKind::ResourceNode {
        InteractionType::ResourceNode
    } else if definition.blocks_movement {
        InteractionType::BlockedArea
    } else {
        InteractionType::InteractableObject
    };

    InteractionResult {
        interaction_type,
        position: grounded,
        metadata: InteractionMetadata {
            label: definition.display_name.clone(),
            doodad_kind: Some(definition.kind),
            blocks_movement: definition.blocks_movement,
        },
        valid: true,
        target: InteractionTargetRef::Doodad(record.id),
    }
}

fn nearest_doodad_in_radius<'a>(
    ctx: &'a InteractionQueryContext<'_>,
    position: WorldPosition,
    radius_meters: f32,
) -> Option<(&'a DoodadRecord, &'a DoodadDefinition)> {
    let layout = ctx.world.layout();
    let center = position.to_global(layout);
    let center_xz = Vec2::new(center.x, center.z);

    let mut best: Option<(f32, DoodadId)> = None;
    let mut best_record: Option<&'a DoodadRecord> = None;
    let mut best_definition: Option<&'a DoodadDefinition> = None;

    let mut chunks: Vec<ChunkCoord> = Vec::with_capacity(9);
    for dz in -1..=1 {
        for dx in -1..=1 {
            chunks.push(ChunkCoord::new(
                position.chunk.x + dx,
                position.chunk.z + dz,
            ));
        }
    }
    chunks.sort_by_key(|coord| (coord.x, coord.z));

    for chunk_coord in chunks {
        let chunk_id = ChunkId::new(chunk_coord);
        let Some(store) = ctx.world.doodads_in_chunk(chunk_id) else {
            continue;
        };
        for record in store.records() {
            let Some(definition) = ctx.doodad_catalog.get(&record.definition_id) else {
                continue;
            };
            let doodad_global = record.placement.position.to_global(layout);
            let doodad_xz = Vec2::new(doodad_global.x, doodad_global.z);
            let reach = radius_meters
                + crate::world::doodad_interaction_radius_meters(record, definition).max(0.5);
            let distance = center_xz.distance(doodad_xz);
            if distance > reach {
                continue;
            }
            let replace = match &best {
                None => true,
                Some((best_dist, best_id)) => {
                    distance < *best_dist - 1e-4
                        || ((distance - *best_dist).abs() <= 1e-4
                            && record.id.raw() < best_id.raw())
                }
            };
            if replace {
                best = Some((distance, record.id));
                best_record = Some(record);
                best_definition = Some(definition);
            }
        }
    }

    match (best_record, best_definition) {
        (Some(record), Some(definition)) => Some((record, definition)),
        _ => None,
    }
}

fn building_pick_radius(definition: &crate::world::BuildingDefinition, uniform_scale: f32) -> f32 {
    match &definition.footprint {
        FootprintSpec::Rectangle {
            width_meters,
            depth_meters,
        } => (width_meters.max(*depth_meters) * 0.5 * uniform_scale).max(1.0),
        FootprintSpec::Circle { radius_meters } => (*radius_meters * uniform_scale).max(1.0),
        FootprintSpec::MeshDerived => (2.0 * uniform_scale).max(1.0),
    }
}

fn nearest_building_in_radius<'a>(
    ctx: &'a InteractionQueryContext<'_>,
    position: WorldPosition,
    radius_meters: f32,
) -> Option<(
    BuildingId,
    &'a crate::world::BuildingRecord,
    &'a crate::world::BuildingDefinition,
)> {
    let layout = ctx.world.layout();
    let center = position.to_global(layout);
    let center_xz = Vec2::new(center.x, center.z);
    let mut best: Option<(f32, BuildingId)> = None;
    let mut best_record = None;
    let mut best_definition = None;

    for building_id in ctx.world.sorted_building_ids() {
        let Some(record) = ctx.world.get_building(building_id) else {
            continue;
        };
        let Some(definition) = ctx.building_catalog.get(&record.definition_id) else {
            continue;
        };
        let building_global = record.placement.position.to_global(layout);
        let building_xz = Vec2::new(building_global.x, building_global.z);
        let reach =
            radius_meters + building_pick_radius(definition, record.placement.uniform_scale_f32());
        let distance = center_xz.distance(building_xz);
        if distance > reach {
            continue;
        }
        let replace = match &best {
            None => true,
            Some((best_dist, best_id)) => {
                distance < *best_dist - 1e-4
                    || ((distance - *best_dist).abs() <= 1e-4 && building_id.raw() < best_id.raw())
            }
        };
        if replace {
            best = Some((distance, building_id));
            best_record = Some(record);
            best_definition = Some(definition);
        }
    }

    match (best, best_record, best_definition) {
        (Some((_, building_id)), Some(record), Some(definition)) => {
            Some((building_id, record, definition))
        }
        _ => None,
    }
}

fn classify_building_hit(
    ctx: &InteractionQueryContext<'_>,
    grounded: WorldPosition,
    building_id: BuildingId,
    record: &crate::world::BuildingRecord,
    definition: &crate::world::BuildingDefinition,
) -> InteractionResult {
    let profile = ctx.interaction_catalog.profile_for_definition(definition);
    let interaction_type = if building_is_constructible(record)
        && profile.is_some_and(|profile| profile.capabilities.construction_site)
    {
        InteractionType::ConstructionSite
    } else if building_accepts_workstation_use(record)
        && profile.is_some_and(|profile| profile.capabilities.workstation)
    {
        InteractionType::Workstation
    } else if ctx
        .world
        .settlement_store()
        .settlement_for_building(building_id)
        .is_some()
        && crate::world::building_supports_settlement_treasury(
            ctx.building_catalog,
            ctx.interaction_catalog,
            building_id,
            ctx.world,
        )
    {
        InteractionType::Treasury
    } else if crate::world::building_has_inventory(ctx.world, building_id)
        && crate::world::building_inventory_operational(ctx.world, record)
        && profile.is_some_and(|profile| profile.capabilities.container)
    {
        InteractionType::Container
    } else if record.lifecycle_state.is_terminal_damage_state() {
        InteractionType::None
    } else {
        InteractionType::InteractableObject
    };

    let valid = match interaction_type {
        InteractionType::ConstructionSite
        | InteractionType::Workstation
        | InteractionType::Container
        | InteractionType::Treasury => true,
        InteractionType::None => false,
        _ => true,
    };

    InteractionResult {
        interaction_type,
        position: grounded,
        metadata: InteractionMetadata {
            label: definition.display_name.clone(),
            doodad_kind: None,
            blocks_movement: false,
        },
        valid,
        target: InteractionTargetRef::Building(building_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, BuildingCatalog, BuildingSource, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
        DoodadDefinitionId, DoodadPlacementOverrides, DoodadSource, FootprintCatalog, Heightfield,
        ItemPileId, ItemPileSettings, LocalPosition, SpaceId, create_building, create_doodad,
        default_building_catalog, default_footprint_catalog,
    };

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

    fn ctx<'a>(
        world: &'a WorldData,
        catalog: &'a DoodadCatalog,
        unit_catalog: &'a UnitCatalog,
        weapon_catalog: &'a crate::world::WeaponCatalog,
        interaction_catalog: &'a BuildingInteractionProfileCatalog,
        pile_settings: &'a ItemPileSettings,
    ) -> InteractionQueryContext<'a> {
        InteractionQueryContext::new(
            world,
            catalog,
            default_building_catalog(),
            default_footprint_catalog(),
            interaction_catalog,
            unit_catalog,
            weapon_catalog,
            pile_settings,
        )
    }

    fn test_pile_settings() -> &'static ItemPileSettings {
        use std::sync::OnceLock;
        static SETTINGS: OnceLock<ItemPileSettings> = OnceLock::new();
        SETTINGS.get_or_init(ItemPileSettings::default)
    }

    fn weapons() -> crate::world::WeaponCatalog {
        crate::world::WeaponCatalog::default()
    }

    fn interaction_catalog() -> &'static BuildingInteractionProfileCatalog {
        use std::sync::OnceLock;
        static CATALOG: OnceLock<BuildingInteractionProfileCatalog> = OnceLock::new();
        CATALOG.get_or_init(BuildingInteractionProfileCatalog::default)
    }

    #[test]
    fn terrain_click_returns_move_target() {
        let world = flat_world();
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let result = query_world_interaction(
            &ctx(
                &world,
                &catalog,
                &unit_catalog,
                &weapons,
                &interaction_catalog,
                test_pile_settings(),
            ),
            pos(64.0, 64.0),
        )
        .unwrap();
        assert_eq!(result.interaction_type, InteractionType::MoveTarget);
        assert!(result.valid);
    }

    #[test]
    fn blocking_doodad_returns_blocked_area() {
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        let mut world = flat_world();
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            pos(50.0, 50.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
            None,
        )
        .unwrap();

        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let result = query_world_interaction(
            &ctx(
                &world,
                &catalog,
                &unit_catalog,
                &weapons,
                &interaction_catalog,
                test_pile_settings(),
            ),
            pos(50.0, 50.0),
        )
        .unwrap();
        assert_eq!(result.interaction_type, InteractionType::BlockedArea);
        assert!(matches!(result.target, InteractionTargetRef::Doodad(_)));
    }

    #[test]
    fn non_blocking_doodad_returns_interactable_object() {
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        let mut world = flat_world();
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("bush_scrub"),
            pos(30.0, 30.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
            None,
        )
        .unwrap();

        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let result = query_world_interaction(
            &ctx(
                &world,
                &catalog,
                &unit_catalog,
                &weapons,
                &interaction_catalog,
                test_pile_settings(),
            ),
            pos(30.0, 30.0),
        )
        .unwrap();
        assert_eq!(result.interaction_type, InteractionType::InteractableObject);
    }

    #[test]
    fn resource_node_returns_resource_node_type() {
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        let mut world = flat_world();
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("resource_node_iron"),
            pos(70.0, 70.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
            None,
        )
        .unwrap();

        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let result = query_world_interaction(
            &ctx(
                &world,
                &catalog,
                &unit_catalog,
                &weapons,
                &interaction_catalog,
                test_pile_settings(),
            ),
            pos(70.0, 70.0),
        )
        .unwrap();
        assert_eq!(result.interaction_type, InteractionType::ResourceNode);
    }

    #[test]
    fn invalid_position_without_terrain_returns_none() {
        let world = WorldData::new(layout());
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        assert!(
            query_world_interaction(
                &ctx(
                    &world,
                    &catalog,
                    &unit_catalog,
                    &weapons,
                    interaction_catalog(),
                    test_pile_settings(),
                ),
                pos(1.0, 1.0)
            )
            .is_none()
        );
    }

    #[test]
    fn query_layer_does_not_mutate_world_data() {
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let world = flat_world();
        let chunks_before = world.len();
        let _ = query_world_interaction(
            &InteractionQueryContext::new(
                &world,
                &catalog,
                default_building_catalog(),
                default_footprint_catalog(),
                &BuildingInteractionProfileCatalog::default(),
                &unit_catalog,
                &crate::world::WeaponCatalog::default(),
                test_pile_settings(),
            ),
            pos(10.0, 10.0),
        );
        assert_eq!(world.len(), chunks_before);
    }

    #[test]
    fn classification_is_deterministic() {
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        let world = flat_world();
        let a = query_world_interaction(
            &ctx(
                &world,
                &catalog,
                &unit_catalog,
                &weapons,
                interaction_catalog(),
                test_pile_settings(),
            ),
            pos(12.0, 14.0),
        );
        let b = query_world_interaction(
            &ctx(
                &world,
                &catalog,
                &unit_catalog,
                &weapons,
                interaction_catalog(),
                test_pile_settings(),
            ),
            pos(12.0, 14.0),
        );
        assert_eq!(a, b);
    }

    #[test]
    fn slope_unavailable_returns_blocked_area_not_none() {
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        let mut world = WorldData::new(layout());
        let samples: Vec<f32> = (0..9).map(|i| i as f32 * 10.0).collect();
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        let position = pos(0.0, 0.0);
        let result = query_world_interaction(
            &ctx(
                &world,
                &catalog,
                &unit_catalog,
                &weapons,
                interaction_catalog(),
                test_pile_settings(),
            ),
            position,
        );
        if matches!(
            classify_slope_walkability(&world, position, DEFAULT_INTERACTION_MAX_SLOPE_DEGREES),
            SlopeWalkability::Unavailable
        ) {
            let result = result.expect("slope unavailable should surface blocked feedback");
            assert_eq!(result.interaction_type, InteractionType::BlockedArea);
            assert!(!result.valid);
            assert_eq!(result.metadata.label, "Terrain unavailable");
        }
    }

    fn insert_stack_pile(
        world: &mut WorldData,
        pile_id: ItemPileId,
        position: WorldPosition,
        space_id: SpaceId,
        quantity: u32,
    ) {
        use crate::world::{Affiliation, ItemDefinitionId, ItemPileSource, WorldItemPileRecord};
        let record = WorldItemPileRecord::new_stack(
            pile_id,
            position,
            space_id,
            ItemDefinitionId::new("gold"),
            quantity,
            None,
            None,
            Affiliation::Player,
            ItemPileSource::DevSpawned,
            0,
        );
        let chunk = ChunkId::new(position.chunk);
        world.item_pile_store_mut().insert(chunk, record).unwrap();
    }

    #[test]
    fn pile_near_click_classifies_as_item_pile() {
        let mut world = flat_world();
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
        insert_stack_pile(&mut world, pile_id, pos(20.0, 20.0), SpaceId::SURFACE, 5);

        let result = query_world_interaction(
            &ctx(
                &world,
                &catalog,
                &unit_catalog,
                &weapons,
                &interaction_catalog,
                test_pile_settings(),
            ),
            pos(20.0, 20.0),
        )
        .unwrap();
        assert_eq!(result.interaction_type, InteractionType::ItemPile);
        assert!(result.valid);
    }

    #[test]
    fn pile_hit_returns_correct_target_id() {
        let mut world = flat_world();
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let pile_id = ItemPileId::new(42);
        insert_stack_pile(&mut world, pile_id, pos(22.0, 22.0), SpaceId::SURFACE, 3);

        let result = query_world_interaction(
            &ctx(
                &world,
                &catalog,
                &unit_catalog,
                &weapons,
                &interaction_catalog,
                test_pile_settings(),
            ),
            pos(22.0, 22.0),
        )
        .unwrap();
        assert_eq!(result.target, InteractionTargetRef::ItemPile(pile_id));
    }

    #[test]
    fn building_overlapping_pile_wins_priority() {
        let mut world = flat_world();
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let building_catalog = default_building_catalog();
        let click = pos(55.0, 55.0);
        let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
        insert_stack_pile(&mut world, pile_id, click, SpaceId::SURFACE, 5);
        create_building(
            building_catalog,
            &mut world,
            &crate::world::BuildingDefinitionId::new("hut"),
            click,
            Quat::IDENTITY,
            crate::world::BuildingSource::Authored,
            crate::world::BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
        )
        .unwrap();

        let result = query_world_interaction(
            &ctx(
                &world,
                &catalog,
                &unit_catalog,
                &weapons,
                &interaction_catalog,
                test_pile_settings(),
            ),
            click,
        )
        .unwrap();
        assert!(matches!(result.target, InteractionTargetRef::Building(_)));
        assert_ne!(result.interaction_type, InteractionType::ItemPile);
    }

    #[test]
    fn pile_overlapping_doodad_wins_priority() {
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        let mut world = flat_world();
        let click = pos(35.0, 35.0);
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("bush_scrub"),
            click,
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
            None,
        )
        .unwrap();
        let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
        insert_stack_pile(&mut world, pile_id, click, SpaceId::SURFACE, 2);

        let result = query_world_interaction(
            &ctx(
                &world,
                &catalog,
                &unit_catalog,
                &weapons,
                &BuildingInteractionProfileCatalog::default(),
                test_pile_settings(),
            ),
            click,
        )
        .unwrap();
        assert_eq!(result.interaction_type, InteractionType::ItemPile);
        assert_eq!(result.target, InteractionTargetRef::ItemPile(pile_id));
    }

    #[test]
    fn pile_outside_interaction_radius_is_ignored() {
        let mut world = flat_world();
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
        insert_stack_pile(&mut world, pile_id, pos(10.0, 10.0), SpaceId::SURFACE, 1);
        let mut settings = ItemPileSettings::default();
        settings.interaction_radius_meters = 2.0;

        let result = query_world_interaction(
            &InteractionQueryContext {
                world: &world,
                doodad_catalog: &catalog,
                building_catalog: default_building_catalog(),
                footprint_catalog: default_footprint_catalog(),
                interaction_catalog: &interaction_catalog,
                unit_catalog: &unit_catalog,
                weapon_catalog: &weapons,
                pile_settings: &settings,
                query_radius_meters: DEFAULT_INTERACTION_QUERY_RADIUS_METERS,
                agent_radius_meters: DEFAULT_INTERACTION_AGENT_RADIUS_METERS,
                max_slope_degrees: DEFAULT_INTERACTION_MAX_SLOPE_DEGREES,
            },
            pos(10.0, 20.0),
        )
        .unwrap();
        assert_eq!(result.interaction_type, InteractionType::MoveTarget);
    }

    #[test]
    fn pile_in_different_space_is_ignored_on_surface_click() {
        let mut world = flat_world();
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let click = pos(18.0, 18.0);
        let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
        insert_stack_pile(&mut world, pile_id, click, SpaceId::new(99), 4);

        let result = query_world_interaction(
            &ctx(
                &world,
                &catalog,
                &unit_catalog,
                &weapons,
                &interaction_catalog,
                test_pile_settings(),
            ),
            click,
        )
        .unwrap();
        assert_ne!(result.interaction_type, InteractionType::ItemPile);
    }

    #[test]
    fn nearest_pile_wins_deterministic_tie_break() {
        let mut world = flat_world();
        let catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let weapons = weapons();
        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let near_id = ItemPileId::new(2);
        let far_id = ItemPileId::new(1);
        insert_stack_pile(&mut world, far_id, pos(40.0, 40.0), SpaceId::SURFACE, 1);
        insert_stack_pile(&mut world, near_id, pos(40.5, 40.5), SpaceId::SURFACE, 1);

        let result = query_world_interaction(
            &ctx(
                &world,
                &catalog,
                &unit_catalog,
                &weapons,
                &interaction_catalog,
                test_pile_settings(),
            ),
            pos(40.0, 40.0),
        )
        .unwrap();
        assert_eq!(result.target, InteractionTargetRef::ItemPile(far_id));
    }
}
