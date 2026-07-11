//! WorldData scene snapshot model and capture (ADR-045).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::dev::dev_mode::DevDebugFlags;
use crate::world::{
    Affiliation, DoodadId, DoodadKind, DoodadRecord, DoodadSource, OwnerId, TeamId, UnitId,
    UnitRecord, UnitSource, UnitState, WorldData, WorldPosition, default_ownership_for_source,
};

use super::SceneCaptureContext;

/// On-disk scene format version.
pub const SCENE_VERSION: u32 = 1;

/// Pure-data scene snapshot — no logic (ADR-045).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneDefinition {
    pub version: u32,
    pub scene_id: String,
    pub name: String,
    pub description: String,
    pub created_at: u64,
    pub tags: Vec<String>,
    pub world_seed: u64,
    pub unit_records: Vec<SceneUnitRecord>,
    pub doodad_records: Vec<SceneDoodadRecord>,
    pub camera_state: Option<SceneCameraState>,
    pub debug_flags: Option<SceneDebugFlagsSnapshot>,
    pub next_unit_id: u64,
    pub next_doodad_id: u64,
}

/// Serializable unit instance for dev scenes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneUnitRecord {
    pub id: u64,
    pub definition_id: String,
    pub position: SceneWorldPosition,
    pub rotation: SceneQuat,
    pub state: SceneUnitState,
    pub source: SceneUnitSource,
    #[serde(default)]
    pub owner_id: Option<u64>,
    #[serde(default)]
    pub team_id: Option<u64>,
    #[serde(default)]
    pub affiliation: Option<String>,
}

/// Serializable doodad instance for dev scenes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneDoodadRecord {
    pub id: u64,
    pub definition_id: String,
    pub kind: String,
    pub position: SceneWorldPosition,
    pub rotation: SceneQuat,
    pub scale: [f32; 3],
    pub source: SceneDoodadSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SceneWorldPosition {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub local_x: f32,
    pub local_y: f32,
    pub local_z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SceneQuat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SceneUnitState {
    Idle,
    Moving {
        target: SceneWorldPosition,
        waypoints: Vec<SceneWorldPosition>,
        waypoint_index: usize,
    },
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneUnitSource {
    Authored,
    Dev,
    Procedural { seed: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneDoodadSource {
    Authored,
    Dev,
    Procedural { seed: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SceneCameraState {
    pub position: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SceneDebugFlagsSnapshot {
    pub show_paths: bool,
    pub show_steering_vectors: bool,
    pub show_formations: bool,
    pub show_selection_circles: bool,
    pub show_interaction_hits: bool,
    pub show_command_trace: bool,
    pub show_grid_overlay: bool,
    pub master_enabled: bool,
}

impl SceneDefinition {
    pub fn new(
        scene_id: impl Into<String>,
        ctx: &SceneCaptureContext,
        unit_records: Vec<SceneUnitRecord>,
        doodad_records: Vec<SceneDoodadRecord>,
        next_unit_id: u64,
        next_doodad_id: u64,
    ) -> Self {
        Self {
            version: SCENE_VERSION,
            scene_id: scene_id.into(),
            name: ctx.name.clone(),
            description: ctx.description.clone(),
            created_at: ctx.created_at,
            tags: ctx.tags.clone(),
            world_seed: ctx.world_seed,
            unit_records,
            doodad_records,
            camera_state: ctx.camera_state,
            debug_flags: ctx.debug_flags,
            next_unit_id,
            next_doodad_id,
        }
    }
}

/// Extract a deterministic scene snapshot from authoritative [`WorldData`].
pub fn capture_scene(world: &WorldData, ctx: &SceneCaptureContext) -> SceneDefinition {
    let mut unit_records = Vec::new();
    for id in world.sorted_unit_ids() {
        let record = world.get_unit(id).expect("sorted id must resolve");
        unit_records.push(SceneUnitRecord::from_record(record));
    }

    let mut doodad_records = Vec::new();
    for id in world.sorted_doodad_ids() {
        let record = world.get_doodad(id).expect("sorted id must resolve");
        doodad_records.push(SceneDoodadRecord::from_record(record));
    }

    let scene_id = super::registry::make_scene_id(&ctx.name, ctx.created_at);
    SceneDefinition::new(
        scene_id,
        ctx,
        unit_records,
        doodad_records,
        world.dev_next_unit_id(),
        world.dev_next_doodad_id(),
    )
}

impl SceneUnitRecord {
    pub fn from_record(record: &UnitRecord) -> Self {
        Self {
            id: record.id.raw(),
            definition_id: record.definition_id.as_str().to_string(),
            position: SceneWorldPosition::from_world(record.placement.position),
            rotation: SceneQuat::from_quat(record.placement.rotation),
            state: SceneUnitState::from_state(&record.state),
            source: SceneUnitSource::from_source(record.source),
            owner_id: record.owner_id.map(|id| id.raw()),
            team_id: record.team_id.map(|id| id.raw()),
            affiliation: Some(record.affiliation.label().to_string()),
        }
    }

    pub fn to_record(&self) -> Result<UnitRecord, SceneRecordError> {
        let source = self.source.to_source();
        let mut ownership = default_ownership_for_source(source);
        if let Some(owner) = self.owner_id {
            ownership.owner_id = Some(OwnerId::new(owner));
        }
        if let Some(team) = self.team_id {
            ownership.team_id = Some(TeamId::new(team));
        }
        if let Some(label) = self.affiliation.as_deref() {
            ownership.affiliation = affiliation_from_label(label);
        }
        let mut record = UnitRecord::new(
            UnitId::new(self.id),
            crate::world::UnitDefinitionId::new(&self.definition_id),
            crate::world::UnitPlacement::new(self.position.to_world()?, self.rotation.to_quat()),
            source,
            ownership,
            5,
        );
        record.state = self.state.to_state()?;
        Ok(record)
    }
}

impl SceneDoodadRecord {
    pub fn from_record(record: &DoodadRecord) -> Self {
        Self {
            id: record.id.raw(),
            definition_id: record.definition_id.as_str().to_string(),
            kind: doodad_kind_label(record.kind).to_string(),
            position: SceneWorldPosition::from_world(record.placement.position),
            rotation: SceneQuat::from_quat(record.placement.rotation),
            scale: [
                record.placement.scale.x,
                record.placement.scale.y,
                record.placement.scale.z,
            ],
            source: SceneDoodadSource::from_source(record.source),
        }
    }

    pub fn to_record(&self, kind: DoodadKind) -> Result<DoodadRecord, SceneRecordError> {
        Ok(DoodadRecord::new(
            DoodadId::new(self.id),
            crate::world::DoodadDefinitionId::new(&self.definition_id),
            kind,
            crate::world::DoodadPlacement::new(
                self.position.to_world()?,
                self.rotation.to_quat(),
                Vec3::new(self.scale[0], self.scale[1], self.scale[2]),
            ),
            self.source.to_source(),
        ))
    }
}

impl SceneWorldPosition {
    pub fn from_world(position: WorldPosition) -> Self {
        Self {
            chunk_x: position.chunk.x,
            chunk_z: position.chunk.z,
            local_x: position.local.0.x,
            local_y: position.local.0.y,
            local_z: position.local.0.z,
        }
    }

    pub fn to_world(self) -> Result<WorldPosition, SceneRecordError> {
        if !self.local_x.is_finite() || !self.local_y.is_finite() || !self.local_z.is_finite() {
            return Err(SceneRecordError::InvalidPosition);
        }
        Ok(WorldPosition::new(
            crate::world::ChunkCoord::new(self.chunk_x, self.chunk_z),
            crate::world::LocalPosition::new(Vec3::new(self.local_x, self.local_y, self.local_z)),
        ))
    }
}

impl SceneQuat {
    pub fn from_quat(quat: Quat) -> Self {
        Self {
            x: quat.x,
            y: quat.y,
            z: quat.z,
            w: quat.w,
        }
    }

    pub fn to_quat(self) -> Quat {
        Quat::from_xyzw(self.x, self.y, self.z, self.w)
    }
}

impl SceneUnitState {
    pub fn from_state(state: &UnitState) -> Self {
        match state {
            UnitState::Idle => Self::Idle,
            UnitState::Moving {
                target,
                path,
                waypoint_index,
            } => Self::Moving {
                target: SceneWorldPosition::from_world(*target),
                waypoints: path
                    .waypoints
                    .iter()
                    .copied()
                    .map(SceneWorldPosition::from_world)
                    .collect(),
                waypoint_index: *waypoint_index,
            },
            UnitState::Dead => Self::Dead,
        }
    }

    pub fn to_state(&self) -> Result<UnitState, SceneRecordError> {
        Ok(match self {
            Self::Idle => UnitState::Idle,
            Self::Dead => UnitState::Dead,
            Self::Moving {
                target,
                waypoints,
                waypoint_index,
            } => UnitState::Moving {
                target: target.to_world()?,
                path: crate::world::NavigationPath::new(
                    waypoints
                        .iter()
                        .copied()
                        .map(SceneWorldPosition::to_world)
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                waypoint_index: *waypoint_index,
            },
        })
    }
}

impl SceneUnitSource {
    pub fn from_source(source: UnitSource) -> Self {
        match source {
            UnitSource::Authored => Self::Authored,
            UnitSource::Dev => Self::Dev,
            UnitSource::Procedural { seed } => Self::Procedural { seed },
        }
    }

    pub fn to_source(self) -> UnitSource {
        match self {
            Self::Authored => UnitSource::Authored,
            Self::Dev => UnitSource::Dev,
            Self::Procedural { seed } => UnitSource::Procedural { seed },
        }
    }
}

impl SceneDoodadSource {
    pub fn from_source(source: DoodadSource) -> Self {
        match source {
            DoodadSource::Authored => Self::Authored,
            DoodadSource::Dev => Self::Dev,
            DoodadSource::Procedural { seed } => Self::Procedural { seed },
        }
    }

    pub fn to_source(self) -> DoodadSource {
        match self {
            Self::Authored => DoodadSource::Authored,
            Self::Dev => DoodadSource::Dev,
            Self::Procedural { seed } => DoodadSource::Procedural { seed },
        }
    }
}

impl From<DevDebugFlags> for SceneDebugFlagsSnapshot {
    fn from(flags: DevDebugFlags) -> Self {
        Self {
            master_enabled: flags.enabled,
            show_paths: flags.path,
            show_steering_vectors: flags.steering,
            show_formations: flags.formation,
            show_selection_circles: flags.selection,
            show_interaction_hits: flags.interaction,
            show_command_trace: flags.intent,
            show_grid_overlay: flags.grid,
        }
    }
}

impl From<SceneDebugFlagsSnapshot> for DevDebugFlags {
    fn from(flags: SceneDebugFlagsSnapshot) -> Self {
        Self {
            enabled: flags.master_enabled,
            path: flags.show_paths,
            steering: flags.show_steering_vectors,
            formation: flags.show_formations,
            selection: flags.show_selection_circles,
            interaction: flags.show_interaction_hits,
            intent: flags.show_command_trace,
            grid: flags.show_grid_overlay,
            ..Default::default()
        }
    }
}

fn doodad_kind_label(kind: DoodadKind) -> &'static str {
    match kind {
        DoodadKind::Tree => "Tree",
        DoodadKind::Rock => "Rock",
        DoodadKind::Bush => "Bush",
        DoodadKind::Ruin => "Ruin",
        DoodadKind::ResourceNode => "ResourceNode",
    }
}

pub fn parse_doodad_kind(label: &str) -> Result<DoodadKind, SceneRecordError> {
    match label {
        "Tree" => Ok(DoodadKind::Tree),
        "Rock" => Ok(DoodadKind::Rock),
        "Bush" => Ok(DoodadKind::Bush),
        "Ruin" => Ok(DoodadKind::Ruin),
        "ResourceNode" => Ok(DoodadKind::ResourceNode),
        _ => Err(SceneRecordError::UnknownDoodadKind(label.to_string())),
    }
}

fn affiliation_from_label(label: &str) -> Affiliation {
    match label {
        "Player" => Affiliation::Player,
        "Neutral" => Affiliation::Neutral,
        "Hostile" => Affiliation::Hostile,
        "Wildlife" => Affiliation::Wildlife,
        "Dev" => Affiliation::Dev,
        _ => Affiliation::Unknown,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneRecordError {
    InvalidPosition,
    UnknownDoodadKind(String),
}
