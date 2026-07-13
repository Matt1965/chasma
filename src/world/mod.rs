use bevy::prelude::*;

mod biome;
mod building;
mod chunk;
mod combat;
mod config;
mod coordinates;
mod data;
mod doodad;
mod formation;
mod interaction;
mod movement;
mod navigation;
mod obstacle;
mod occupancy;
mod ownership;
mod projectile;
mod space;
mod task;
mod terrain;
mod unit;
mod weapon;

#[cfg(any(test, feature = "dev"))]
pub use weapon::starter_definitions as starter_weapon_definitions;
pub use weapon::{
    AttackPlaybackPolicy, DamageType, HitMode, TargetFilter, WeaponAttackAnimation, WeaponCatalog,
    WeaponCatalogError, WeaponDefinition, WeaponDefinitionId,
};

pub use biome::{
    BiomeColorEntry, BiomeColorMapping, BiomeId, BiomeImportError, BiomeMask, BiomeMaskBounds,
    BiomeSample, import_biome_mask_from_png, import_biome_mask_from_png_bytes,
};
#[cfg(any(test, feature = "dev"))]
pub use biome::{
    DEV_BIOME_MASK_PATH, DEV_SOURCE_WORLD_DIR, DevBiomeLoadOutcome, DevBiomeLoadSummary,
    biome_mask_path_for_world, dev_biome_mask_bounds, log_dev_biome_load_outcome,
    try_load_default_dev_biome_mask, try_load_dev_biome_mask,
};
#[cfg(any(test, feature = "dev"))]
pub use building::starter_building_category_definitions;
#[cfg(any(test, feature = "dev"))]
pub use building::starter_definitions as starter_building_definitions;
pub use building::{
    BuildingAuthoringError, BuildingCapabilities, BuildingCatalog, BuildingCatalogError,
    BuildingCategoryCatalog, BuildingCategoryCatalogError, BuildingCategoryDefinition,
    BuildingCategoryId, BuildingConstructionReport, BuildingConstructionSettings,
    BuildingDefinition, BuildingDefinitionId, BuildingId, BuildingInsertError,
    BuildingInteractionProfile, BuildingInteractionProfileCatalog, BuildingInteriorState,
    BuildingLifecycleError, BuildingLifecycleEvent, BuildingLifecycleState, BuildingOwnership,
    BuildingPlacement, BuildingPlacementConfig, BuildingPlacementContext,
    BuildingPlacementRejectReason, BuildingPlacementValidation, BuildingRebuildError,
    BuildingRecord, BuildingRenderKey, BuildingRestoreError, BuildingSource, BuildingSpaces,
    BuildingVitals, ChunkBuildingStore, ConstructionState, DoorAccessPolicy, DoorId, DoorRecord,
    DoorState, DoorStore, FootprintSpec, FootprintType, INTERACTION_WORK_RANGE_METERS,
    InteractionPointDefinition, InteriorError, InteriorProfileCatalog, InteriorProfileId,
    activate_building_interior, add_building_construction_progress, anchor_from_terrain_position,
    close_door, create_building, damage_building, deactivate_building_interior, destroy_building,
    destroy_door, heal_building, interaction_point_world_position, is_building_operational,
    lock_door, lookup_building, move_building, open_door, place_player_building,
    portal_traversable, rebuild_building_world_indexes, remove_building, rotation_from_quadrants,
    set_building_lifecycle_stage, snap_anchor_global_xz, space_route_for_unit,
    step_all_building_construction, transition_to_ruins, try_open_door_at_portal_for_unit,
    try_open_door_for_unit, validate_building_for_restore, validate_building_placement,
};
pub use chunk::{ChunkData, ChunkId};
pub use combat::{
    AttackTargetingPolicy, CombatAiReport, CombatAiScanState, CombatAiSettings, CombatAiTrace,
    CombatAiTraceOutcome, CombatEngagementReport, CombatEngagementStatus, CombatEngagementTrace,
    CombatStrikeEvent, CombatStrikeReport, CombatStrikeTrace, ProjectileImpactRejection,
    ProjectileLaunchSnapshot, RANGE_HYSTERESIS_METERS, RangeCheck, WeaponTiming,
    classify_unit_target, clear_attack_cycle_for_order_cancel, find_auto_acquire_target,
    initial_attack_combat_state, is_in_weapon_range, is_unit_alive, is_valid_attack_target,
    reset_attack_cycle_for_retarget, step_all_combat_engagement, step_all_combat_strikes,
    step_combat_ai_acquisition, validate_attack_target, validate_projectile_impact_target,
    weapon_for_unit_record,
};
pub use config::WorldConfig;
pub use coordinates::{ChunkCoord, ChunkLayout, LocalPosition, WorldPosition};
pub use data::{ChunkExtent, WorldData};
#[cfg(test)]
pub use doodad::starter_definitions;
pub use doodad::{
    BiomeFilterResult, ChunkDoodadStore, ChunkProceduralMaterializeOutcome, DeterministicRng,
    DoodadAuthoringError, DoodadCatalog, DoodadCatalogError, DoodadDefinition, DoodadDefinitionId,
    DoodadExclusionZone, DoodadGenerationContext, DoodadGenerationSettings, DoodadId,
    DoodadInsertError, DoodadKind, DoodadMaterializationReport, DoodadMetadata, DoodadPlacement,
    DoodadPlacementOverrides, DoodadRecord, DoodadRenderKey, DoodadSource, DoodadSpawnCandidate,
    ExclusionFilterOptions, ExclusionFilterResult, FinalizedDoodadPlacement,
    MaterializationOptions, PlacementFinalizationResult, ProceduralDoodadKey,
    TerrainValidationResult, chunk_needs_procedural_materialization, create_doodad,
    default_blocks_movement, filter_candidates_by_biome, filter_candidates_by_exclusion_zones,
    filter_candidates_by_terrain, finalize_placements, generate_chunk_doodads,
    generate_chunk_doodads_with_settings, lookup_doodad, materialize_candidates,
    materialize_candidates_with_exclusion, materialize_candidates_with_options, move_doodad,
    remove_doodad, try_materialize_procedural_chunk_doodads,
};
#[cfg(any(test, feature = "dev"))]
pub use doodad::{DoodadRestoreError, restore_doodad_record, validate_doodad_for_restore};
pub use formation::{
    FormationAssignment, FormationKind, FormationMovePlan, FormationPlanner,
    circle_formation_radius, collision_separation_meters, formation_offsets,
    resolve_move_destination, unit_spacing_meters,
};
pub use interaction::{
    DEFAULT_INTERACTION_AGENT_RADIUS_METERS, DEFAULT_INTERACTION_MAX_SLOPE_DEGREES,
    DEFAULT_INTERACTION_QUERY_RADIUS_METERS, InteractionMetadata, InteractionOrderPlan,
    InteractionQueryContext, InteractionResolveContext, InteractionResult, InteractionTargetRef,
    InteractionType, interaction_plan_to_unit_order, query_world_interaction,
    resolve_interaction_to_order, resolve_unit_click_to_order, resolve_world_click_to_order,
    resolve_world_click_to_unit_order,
};
pub use movement::feel::{
    CommandBufferResolveReport, CommandResolveSuccess, MovementFeelSettings,
    MovementSmoothingState, PendingUnitOrder, StabilizedMovementHeading, UnitCommandBuffer,
    stabilized_movement_heading,
};
pub use movement::steering::{
    SteeringContext, SteeringNeighbor, SteeringSettings, alignment_force, apply_steering,
    cohesion_force, gather_steering_neighbors, separation_force,
};
pub use navigation::{
    GridCoord, NEIGHBOR_OFFSETS, NavigationAgent, NavigationConfig, NavigationError,
    NavigationPath, NavigationWaypoint, find_path, find_path_in_spaces, find_path_with_spaces,
    is_position_walkable_in_space, xz_distance,
};
pub use obstacle::{
    ObstacleQueryError, ObstacleQueryResult, blocking_doodad_at_position,
    is_position_blocked_by_doodads, query_obstacle_at_position,
};
#[cfg(test)]
pub use occupancy::test_support::{
    TestPassabilityBundle, default_building_catalog, default_footprint_catalog, default_passability,
};
pub use occupancy::{
    BakedCellMask, ChunkOccupancyGrid, FootprintCatalog, FootprintCatalogError,
    FootprintDefinition, FootprintId, FootprintShape, OCCUPANCY_CELL_SIZE_METERS,
    OccupancyCatalogs, OccupancyCellCoord, OccupancyCellEntry, OccupancyError,
    OccupancyRegistrationPlan, OccupancySource, OccupancyState, PassabilityAgent,
    PassabilityBlockReason, PassabilityCatalogs, PassabilityResult, PassabilityUnavailableReason,
    QuantizedRotation, SURFACE_SPACE_ID, StaticOccupancyResult, agent_overlaps_footprint,
    apply_registration_plan, chunk_for_occupancy_cell, conservative_block_radius_for_kind,
    default_space_id, effective_building_footprint, inline_building_footprint,
    is_position_blocked_by_static_occupancy, is_position_blocked_for_agent, is_position_passable,
    occupied_cells_for_footprint, plan_register_building, plan_register_doodad,
    query_passability_at, query_passability_in_space, query_static_occupancy_at,
    rebuild_occupancy_index, register_building_occupancy, register_doodad_occupancy,
    unregister_source_occupancy, update_building_occupancy, update_doodad_occupancy,
};
pub use ownership::{
    Affiliation, DEFAULT_PLAYER_OWNER_ID, DEFAULT_PLAYER_TEAM_ID, OwnerId,
    SelectionControllabilityPolicy, TeamId, UnitOwnership, default_ownership_for_source,
    filter_commandable_unit_ids, filter_selectable_unit_ids, is_owned_by, is_player_controllable,
    player_units, unit_is_commandable, unit_is_selectable,
};
pub use projectile::{
    ProjectileEvent, ProjectileId, ProjectileRecord, ProjectileReport, ProjectileStatus,
    ProjectileTrace, spawn_projectile_from_strike, step_all_projectiles,
};
#[cfg(any(test, feature = "dev"))]
pub use space::starter_space_profile;
pub use space::{
    PortalId, PortalRecord, PortalTemplate, PortalType, SpaceError, SpaceId, SpaceRecord,
    SpaceRegistry, SpaceTemplate, UnitPortalTransitionState, ground_position_in_space,
    register_building_space_profile, sample_support_height, space_hidden_by_default,
    space_visible_in_view, try_portal_transition, two_story_hut_profile,
};
pub use task::{
    TaskCancelReason, TaskError, TaskEvent, TaskId, TaskPriority, TaskRecord, TaskState, TaskStore,
    TaskTarget, TaskTickReport, TaskType, assign_construct_building_task,
    assign_operate_workstation_task, building_accepts_workstation_use, building_is_constructible,
    cancel_unit_task, ensure_building_task, prune_invalid_building_tasks, step_all_worker_tasks,
    sync_construction_tasks, unit_can_perform_task, unit_may_work_on_building,
};
#[cfg(feature = "terrain-import")]
pub use terrain::{
    DecodeError, GaeaImportError, ImportError, SourceHeightfield, chunk_data_from_source_tile,
    decode_exr_heightfield, expected_chunk_samples_per_edge, gaea_color_dir, gaea_height_dir,
    import_gaea_tile_directory, import_world, parse_gaea_export_filename,
    source_tile_samples_per_edge, validate_gaea_tile_dimensions,
};
pub use terrain::{
    Heightfield, TerrainDataError, TerrainMask, TerrainMetadata, TerrainQueryError,
    validate_heightfield_against_config,
};
pub use terrain::{
    SlopeWalkability, classify_slope_walkability, estimate_slope_degrees, ground_world_position,
    is_position_slope_walkable, slope_at, try_ground_world_position, try_sample_height_at_position,
};
#[cfg(any(test, feature = "dev"))]
pub use unit::starter_animation_profile_definitions;
#[cfg(any(test, feature = "dev"))]
pub use unit::starter_definitions as starter_unit_definitions;
pub use unit::{
    AnimationClipKey, AnimationProfile, AnimationProfileCatalog, AnimationProfileCatalogError,
    AnimationProfileId, AttackCycle, AttackPhase, BatchUnitMovementReport, BlockedMovementReason,
    ChunkUnitStore, CombatState, RemovalReason, UnitAuthoringError, UnitCatalog, UnitCatalogError,
    UnitDeathEvent, UnitDeathReport, UnitDeathTrace, UnitDefinition, UnitDefinitionId,
    UnitGroundingError, UnitId, UnitInsertError, UnitMetadata, UnitMovementError,
    UnitMovementReport, UnitMovementStepOutcome, UnitMovementStepReport, UnitMovementTrace,
    UnitOrder, UnitOrderError, UnitPlacement, UnitRecord, UnitRenderKey, UnitSimulationStepReport,
    UnitSource, UnitState, UnitVitals, UnitWorkCapabilities, create_unit,
    create_unit_with_ownership, ground_unit_position, ground_unit_to_terrain, issue_unit_order,
    lookup_unit, move_unit, remove_unit, resolve_all_pending_unit_orders,
    resolve_pending_unit_orders, step_all_unit_movement, step_unit_death_pipeline,
    step_unit_movement, unit_can_execute_actions, unit_record_can_execute_actions,
};
#[cfg(any(test, feature = "dev"))]
pub use unit::{
    UnitRestoreError, normalize_restored_unit, restore_unit_record, validate_unit_for_restore,
};

/// Owns the World Data Layer: the authoritative coordinate model (ADR-001),
/// chunk identity and definitions (ADR-002), terrain data (ADR-003, ADR-008),
/// doodad data (ADR-015), doodad type catalog (ADR-016), unit type catalog (ADR-027),
/// biome mask authority (ADR-024), unit instance records (ADR-027 U2), and world configuration.
///
/// This is the lowest architectural layer; every later layer depends on it. It
/// registers the foundational data types for reflection and initializes the
/// [`WorldConfig`], (empty) [`WorldData`], [`DoodadCatalog`], [`WeaponCatalog`],
/// and [`UnitCatalog`] resources.
/// Terrain import, rendering, and gameplay systems live in upper layers (ADR-007).
pub struct WorldFoundationPlugin;

impl Plugin for WorldFoundationPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<ChunkCoord>()
            .register_type::<LocalPosition>()
            .register_type::<WorldPosition>()
            .register_type::<ChunkLayout>()
            .register_type::<ChunkId>()
            .register_type::<WorldConfig>()
            .register_type::<Heightfield>()
            .register_type::<TerrainMetadata>()
            .register_type::<TerrainMask>()
            .register_type::<ChunkData>()
            .register_type::<ChunkExtent>()
            .register_type::<DoodadId>()
            .register_type::<DoodadKind>()
            .register_type::<DoodadPlacement>()
            .register_type::<DoodadSource>()
            .register_type::<DoodadMetadata>()
            .register_type::<DoodadRecord>()
            .register_type::<ChunkDoodadStore>()
            .register_type::<DoodadExclusionZone>()
            .register_type::<DoodadDefinitionId>()
            .register_type::<DoodadRenderKey>()
            .register_type::<DoodadDefinition>()
            .register_type::<DoodadCatalog>()
            .register_type::<UnitDefinitionId>()
            .register_type::<UnitRenderKey>()
            .register_type::<UnitDefinition>()
            .register_type::<UnitCatalog>()
            .register_type::<WeaponDefinitionId>()
            .register_type::<AttackPlaybackPolicy>()
            .register_type::<WeaponAttackAnimation>()
            .register_type::<DamageType>()
            .register_type::<HitMode>()
            .register_type::<TargetFilter>()
            .register_type::<WeaponDefinition>()
            .register_type::<WeaponCatalog>()
            .register_type::<AnimationProfileId>()
            .register_type::<AnimationClipKey>()
            .register_type::<AnimationProfile>()
            .register_type::<AnimationProfileCatalog>()
            .register_type::<UnitId>()
            .register_type::<UnitPlacement>()
            .register_type::<UnitSource>()
            .register_type::<UnitMetadata>()
            .register_type::<NavigationConfig>()
            .register_type::<NavigationPath>()
            .register_type::<UnitState>()
            .register_type::<UnitVitals>()
            .register_type::<CombatState>()
            .register_type::<Affiliation>()
            .register_type::<OwnerId>()
            .register_type::<TeamId>()
            .register_type::<UnitRecord>()
            .register_type::<ChunkUnitStore>()
            .register_type::<BiomeId>()
            .register_type::<BiomeSample>()
            .register_type::<BiomeColorEntry>()
            .register_type::<BiomeColorMapping>()
            .register_type::<BiomeMaskBounds>()
            .register_type::<BiomeMask>()
            .register_type::<BuildingDefinitionId>()
            .register_type::<BuildingCategoryId>()
            .register_type::<BuildingRenderKey>()
            .register_type::<FootprintType>()
            .register_type::<FootprintSpec>()
            .register_type::<BuildingCategoryDefinition>()
            .register_type::<BuildingCategoryCatalog>()
            .register_type::<BuildingDefinition>()
            .register_type::<BuildingCatalog>()
            .register_type::<BuildingId>()
            .register_type::<BuildingPlacement>()
            .register_type::<BuildingSource>()
            .register_type::<BuildingOwnership>()
            .register_type::<BuildingLifecycleState>()
            .register_type::<BuildingSpaces>()
            .register_type::<ConstructionState>()
            .register_type::<BuildingRecord>()
            .register_type::<ChunkBuildingStore>()
            .register_type::<FootprintId>()
            .register_type::<FootprintShape>()
            .register_type::<FootprintDefinition>()
            .register_type::<FootprintCatalog>()
            .register_type::<ChunkOccupancyGrid>()
            .register_type::<OccupancyCellEntry>()
            .register_type::<WorldData>();

        app.init_resource::<WorldConfig>();
        #[cfg(not(feature = "dev"))]
        {
            app.init_resource::<DoodadCatalog>();
            app.init_resource::<WeaponCatalog>();
            app.init_resource::<UnitCatalog>();
            app.init_resource::<AnimationProfileCatalog>();
            app.init_resource::<BuildingCategoryCatalog>();
            app.init_resource::<BuildingCatalog>();
            app.init_resource::<FootprintCatalog>();
        }
        #[cfg(feature = "dev")]
        {
            let weapons = crate::data_import::resolve_dev_weapon_catalog();
            let animation_profiles = crate::data_import::resolve_dev_animation_profile_catalog();
            let (building_categories, building_catalog) =
                crate::data_import::resolve_dev_building_catalog();
            let footprint_catalog = crate::data_import::resolve_dev_footprint_catalog();
            app.insert_resource(weapons.clone());
            app.insert_resource(animation_profiles.clone());
            app.insert_resource(building_categories);
            app.insert_resource(building_catalog);
            app.insert_resource(footprint_catalog);
            app.insert_resource(crate::data_import::resolve_dev_doodad_catalog());
            app.insert_resource(crate::data_import::resolve_dev_unit_catalog(
                &weapons,
                &animation_profiles,
            ));
        }
        app.init_resource::<WorldData>();
        app.init_resource::<NavigationConfig>();
        app.init_resource::<InteriorProfileCatalog>();
        app.init_resource::<BuildingInteractionProfileCatalog>();
    }
}
