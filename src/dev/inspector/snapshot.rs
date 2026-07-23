//! Read-only inspector snapshot types (ADR-048).

pub use super::doodad_snapshot::{DoodadInspectorSnapshot, capture_doodad_inspector_snapshot};

use bevy::prelude::*;

use crate::world::{
    BuildingDefinitionId, ChunkCoord, DoodadDefinitionId, ProjectileId, SpaceId, UnitDefinitionId,
    UnitId, UnitOrder, WorldPosition,
};

/// Optional runtime presentation metadata for dev inspector (ADR-095 BA1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuildingAssetPresentationInfo {
    pub desired_render_key: Option<String>,
    pub resolved_asset_path: Option<String>,
    pub asset_load_state: Option<String>,
    pub runtime_entity: Option<u64>,
    pub uses_diagnostic_fallback: bool,
    pub fallback_reason: Option<String>,
    pub space_tag_count: Option<u32>,
    pub roof_tag_count: Option<u32>,
}

/// Full read-only inspection payload for one building (B2).
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingInspectorSnapshot {
    pub building_id: crate::world::BuildingId,
    pub definition_id: BuildingDefinitionId,
    pub display_name: String,
    pub current_hp: u32,
    pub max_hp: u32,
    pub lifecycle_state: String,
    pub progress_percent: f32,
    pub operational: bool,
    pub affiliation: String,
    pub chunk: ChunkCoord,
    pub inventory_summary: Option<String>,
    pub interaction_point: Option<String>,
    pub desired_render_key: Option<String>,
    pub resolved_asset_path: Option<String>,
    pub asset_load_state: Option<String>,
    pub runtime_entity: Option<u64>,
    pub uses_diagnostic_fallback: bool,
    pub fallback_reason: Option<String>,
    pub space_tag_count: Option<u32>,
    pub roof_tag_count: Option<u32>,
    pub terrain_output_rate: Option<String>,
    pub final_output_rate: Option<String>,
    pub operation_progress: Option<String>,
    pub operation_completions: Option<u32>,
    pub operation_limiting_factor: Option<String>,
    pub production_lifecycle: Option<String>,
    pub selected_operation: Option<String>,
    pub policy_enabled: Option<bool>,
    pub policy_paused: Option<bool>,
    pub repeat_mode: Option<String>,
    pub control_source: Option<String>,
    pub policy_priority: Option<u8>,
    pub assigned_workers: Option<String>,
    pub production_blocking_reason: Option<String>,
    pub active_worker_count: Option<u32>,
    pub remaining_repeat_count: Option<u32>,
    pub last_efficiency_revision: Option<u64>,
    pub supported_operations: Option<String>,
    pub default_operation: Option<String>,
    pub operation_category: Option<String>,
    pub base_labor: Option<u32>,
    pub max_workers: Option<u32>,
    pub validation_state: Option<String>,
    pub execution_inputs_summary: Option<String>,
    pub execution_outputs_summary: Option<String>,
    pub execution_inventory_summary: Option<String>,
    pub execution_blocking: Option<String>,
    pub terrain_assessment_summary: Option<String>,
    pub terrain_assessment_revision: Option<u64>,
    pub terrain_assessment_stale: Option<bool>,
    pub inventory_bindings_summary: Option<String>,
    pub hauling_requests_summary: Option<String>,
    pub planner_summary: Option<String>,
}

/// Read-only navigation blueprint inspection payload (NV1.2.5).
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingBlueprintInspectorSnapshot {
    pub blueprint_id: Option<String>,
    pub blueprint_source: String,
    pub generator_version: u32,
    pub generation_status: String,
    pub cache_fresh: bool,
    pub source_fingerprint: Option<String>,
    pub floor_ids: Vec<i32>,
    pub selected_floor_id: Option<i32>,
    pub selected_floor_vertex_count: usize,
    pub selected_floor_elevation: Option<f32>,
    pub selected_floor_entrances: Vec<String>,
    pub selected_floor_transitions: Vec<String>,
    pub entrance_count: usize,
    pub transition_count: usize,
    pub validation: crate::world::BlueprintInspectionValidation,
    pub inspection_active: bool,
    pub edit_active: bool,
    pub edit_dirty: bool,
    pub selected_element: Option<String>,
    pub variant_draft_active: bool,
    pub variant_draft_display_name: Option<String>,
    pub variant_draft_asset_id: Option<String>,
    pub variant_draft_description: Option<String>,
    pub variant_draft_active_field: Option<String>,
    pub building_center: Vec3,
    pub world_bounds_radius: f32,
    pub resolved_blueprint: Option<crate::world::BuildingNavigationBlueprint>,
}

/// Full read-only inspection payload for one unit.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitInspectorSnapshot {
    pub unit_id: UnitId,
    pub definition_id: UnitDefinitionId,
    pub state_label: String,
    pub current_hp: u32,
    pub max_hp: u32,
    pub combat_state_label: String,
    pub combat: CombatInspectorSnapshot,
    pub projectiles: Vec<ProjectileInspectorSnapshot>,
    pub path: PathInspectorSnapshot,
    pub formation: FormationInspectorSnapshot,
    pub steering: SteeringInspectorSnapshot,
    pub block_reason: Option<String>,
    pub chunk: ChunkResidencySnapshot,
    pub simulation_tick: u64,
    pub current_space_id: SpaceId,
    pub display_floor_label: String,
    pub inventory_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CombatInspectorSnapshot {
    pub weapon_name: Option<String>,
    pub target_unit_id: Option<UnitId>,
    pub attack_phase: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectileInspectorSnapshot {
    pub projectile_id: ProjectileId,
    pub source_unit_id: UnitId,
    pub target_unit_id: UnitId,
    pub weapon_id: String,
    pub position: WorldPosition,
    pub speed_mps: f32,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PathInspectorSnapshot {
    pub waypoints: Vec<WorldPosition>,
    pub waypoint_index: usize,
    pub segment_start: Option<WorldPosition>,
    pub segment_end: Option<WorldPosition>,
    pub length_meters: f32,
    pub chunk_transitions: Vec<ChunkCoord>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FormationInspectorSnapshot {
    pub slot_index: Option<usize>,
    pub offset_xz: Vec2,
    pub target: Option<WorldPosition>,
    pub spacing_meters: f32,
    pub peers_sharing_target: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SteeringInspectorSnapshot {
    pub separation: Vec2,
    pub cohesion: Vec2,
    pub alignment: Vec2,
    pub final_direction: Vec2,
    pub neighbor_count: u32,
    pub path_direction: Vec2,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkResidencySnapshot {
    pub unit_chunk: ChunkCoord,
    pub terrain_loaded: bool,
    pub doodads_in_chunk: u32,
    pub units_in_chunk: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InteractionInspectorSnapshot {
    pub click_position: WorldPosition,
    pub terrain_hit: bool,
    pub doodad_hit: Option<DoodadDefinitionId>,
    pub interaction_type: String,
    pub resolved_command: Option<String>,
    pub resolved_order: Option<UnitOrder>,
}
