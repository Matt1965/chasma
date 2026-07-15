//! Read-only inspector snapshot types (ADR-048).

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
