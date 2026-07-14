//! WorldData scene snapshot model and capture (ADR-045).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::dev::dev_mode::DevDebugFlags;
use crate::world::{
    Affiliation, BuildingLifecycleState, BuildingRecord, BuildingSource, BuildingVitals,
    ConstructionState, DoodadId, DoodadKind, DoodadRecord, DoodadSource, OwnerId, TaskId,
    TaskPriority, TaskRecord, TaskState, TaskTarget, TaskType, TeamId, UnitId, UnitRecord,
    UnitSource, UnitState, WorldData, WorldPosition, default_ownership_for_source,
};

use super::SceneCaptureContext;

/// On-disk scene format version.
pub const SCENE_VERSION: u32 = 7;

/// Whether a scene file version can be loaded by the current runtime.
pub fn scene_version_supported(version: u32) -> bool {
    matches!(version, 1 | 4 | 5 | 6 | 7)
}

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
    #[serde(default)]
    pub building_records: Vec<SceneBuildingRecord>,
    pub camera_state: Option<SceneCameraState>,
    pub debug_flags: Option<SceneDebugFlagsSnapshot>,
    pub next_unit_id: u64,
    pub next_doodad_id: u64,
    #[serde(default)]
    pub next_building_id: u64,
    #[serde(default)]
    pub task_records: Vec<SceneTaskRecord>,
    #[serde(default = "default_next_task_id")]
    pub next_task_id: u32,
    #[serde(default = "default_next_door_id")]
    pub next_door_id: u32,
    #[serde(default = "default_next_space_id")]
    pub next_space_id: u32,
    #[serde(default = "default_next_portal_id")]
    pub next_portal_id: u32,
    #[serde(default)]
    pub settlement_records: Vec<SceneSettlementRecord>,
    #[serde(default)]
    pub treasury_records: Vec<SceneTreasuryRecord>,
    #[serde(default = "default_next_settlement_id")]
    pub next_settlement_id: u64,
    #[serde(default = "default_next_treasury_id")]
    pub next_treasury_id: u64,
    /// Full inventory world persistence (ADR-094 I8). Absent in v6 and earlier.
    #[serde(flatten, default)]
    pub inventory_persistence: super::inventory_snapshot::SceneInventoryPersistence,
}

fn default_next_task_id() -> u32 {
    1
}

fn default_next_door_id() -> u32 {
    1
}

fn default_next_space_id() -> u32 {
    1
}

fn default_next_portal_id() -> u32 {
    1
}

fn default_next_settlement_id() -> u64 {
    1
}

fn default_next_treasury_id() -> u64 {
    1
}

/// Serializable settlement instance for dev scenes (ADR-093 I7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneSettlementRecord {
    pub id: u64,
    pub display_name: String,
    pub treasury_id: u64,
    pub anchor_building_id: u64,
    #[serde(default)]
    pub owner_id: Option<u64>,
    #[serde(default)]
    pub team_id: Option<u64>,
    #[serde(default)]
    pub affiliation: Option<String>,
    pub interaction_position: SceneWorldPosition,
    pub created_tick: u64,
}

/// Serializable treasury instance for dev scenes (ADR-093 I7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneTreasuryRecord {
    pub id: u64,
    pub settlement_id: u64,
    pub balance_gold: u64,
    pub created_tick: u64,
    #[serde(default)]
    pub metadata: String,
    #[serde(default)]
    pub owner_id: Option<u64>,
    #[serde(default)]
    pub team_id: Option<u64>,
    #[serde(default)]
    pub affiliation: Option<String>,
}

/// Serializable task instance for dev scenes (ADR-086 B9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneTaskRecord {
    pub id: u32,
    pub task_type: String,
    pub target_building_id: u64,
    #[serde(default)]
    pub interaction_point_key: Option<String>,
    pub state: String,
    pub priority: String,
    #[serde(default)]
    pub assigned_unit_id: Option<u64>,
    #[serde(default)]
    pub reserved_point_key: Option<String>,
    pub created_tick: u64,
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
    /// Authoritative navigable space (ADR-083 B6). Defaults to surface (0).
    #[serde(default)]
    pub current_space_id: u32,
    /// Unit inventory link (ADR-094 I8).
    #[serde(default)]
    pub inventory_id: Option<u32>,
}

/// Serializable building instance for dev scenes (ADR-082 B5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneBuildingRecord {
    pub id: u64,
    pub definition_id: String,
    pub position: SceneWorldPosition,
    pub rotation: SceneQuat,
    pub lifecycle_state: String,
    pub progress_0_1: f32,
    pub current_hp: u32,
    pub max_hp: u32,
    pub source: SceneBuildingSource,
    #[serde(default)]
    pub owner_id: Option<u64>,
    #[serde(default)]
    pub team_id: Option<u64>,
    #[serde(default)]
    pub affiliation: Option<String>,
    /// Parent building for interior child objects (ADR-084 B7).
    #[serde(default)]
    pub parent_building_id: Option<u64>,
    #[serde(default)]
    pub interior_activated: bool,
    #[serde(default)]
    pub interior_profile_id: Option<String>,
    #[serde(default)]
    pub child_doodad_ids: Vec<u64>,
    #[serde(default)]
    pub child_building_ids: Vec<u64>,
    #[serde(default)]
    pub door_states: Vec<SceneDoorSnapshot>,
    #[serde(default)]
    pub inventory_id: Option<u64>,
    #[serde(default)]
    pub container_locked: bool,
}

/// Serializable door state for scene restore (ADR-084 B7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneDoorSnapshot {
    pub definition_key: String,
    pub state: String,
    pub access: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneBuildingSource {
    Authored,
    Dev,
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
    #[serde(default)]
    pub parent_building_id: Option<u64>,
    #[serde(default)]
    pub interior_space_id: Option<u32>,
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
    Working {
        task_id: u32,
    },
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
        next_building_id: u64,
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
            building_records: Vec::new(),
            camera_state: ctx.camera_state,
            debug_flags: ctx.debug_flags,
            next_unit_id,
            next_doodad_id,
            next_building_id,
            task_records: Vec::new(),
            next_task_id: 1,
            next_door_id: 1,
            next_space_id: 1,
            next_portal_id: 1,
            settlement_records: Vec::new(),
            treasury_records: Vec::new(),
            next_settlement_id: 1,
            next_treasury_id: 1,
            inventory_persistence: super::inventory_snapshot::SceneInventoryPersistence::default(),
        }
    }

    pub fn with_building_records(mut self, building_records: Vec<SceneBuildingRecord>) -> Self {
        self.building_records = building_records;
        self
    }

    pub fn with_task_records(mut self, task_records: Vec<SceneTaskRecord>) -> Self {
        self.task_records = task_records;
        self
    }

    pub fn with_runtime_counters(
        mut self,
        next_task_id: u32,
        next_door_id: u32,
        next_space_id: u32,
        next_portal_id: u32,
    ) -> Self {
        self.next_task_id = next_task_id;
        self.next_door_id = next_door_id;
        self.next_space_id = next_space_id;
        self.next_portal_id = next_portal_id;
        self
    }

    pub fn with_settlement_records(
        mut self,
        settlement_records: Vec<SceneSettlementRecord>,
        treasury_records: Vec<SceneTreasuryRecord>,
        next_settlement_id: u64,
        next_treasury_id: u64,
    ) -> Self {
        self.settlement_records = settlement_records;
        self.treasury_records = treasury_records;
        self.next_settlement_id = next_settlement_id;
        self.next_treasury_id = next_treasury_id;
        self
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

    let mut building_records = Vec::new();
    for id in world.sorted_building_ids() {
        let record = world.get_building(id).expect("sorted id must resolve");
        building_records.push(SceneBuildingRecord::from_record(record, world));
    }

    let scene_id = super::registry::make_scene_id(&ctx.name, ctx.created_at);
    let mut task_records = Vec::new();
    for task_id in world.task_store().sorted_task_ids() {
        let record = world
            .task_store()
            .get(task_id)
            .expect("sorted task id must resolve");
        task_records.push(SceneTaskRecord::from_record(record));
    }
    let mut settlement_records = Vec::new();
    for settlement_id in world.settlement_store().sorted_settlement_ids() {
        let record = world
            .settlement_store()
            .get_settlement(settlement_id)
            .expect("sorted settlement id must resolve");
        settlement_records.push(SceneSettlementRecord::from_record(record));
    }
    let mut treasury_records = Vec::new();
    for treasury_id in world.settlement_store().sorted_treasury_ids() {
        let record = world
            .settlement_store()
            .get_treasury(treasury_id)
            .expect("sorted treasury id must resolve");
        treasury_records.push(SceneTreasuryRecord::from_record(record));
    }
    let mut scene = SceneDefinition::new(
        scene_id,
        ctx,
        unit_records,
        doodad_records,
        world.dev_next_unit_id(),
        world.dev_next_doodad_id(),
        world.dev_next_building_id(),
    )
    .with_building_records(building_records)
    .with_task_records(task_records)
    .with_runtime_counters(
        world.task_store().next_id(),
        world.door_store().next_id(),
        world.space_registry().next_space_id(),
        world.space_registry().next_portal_id(),
    )
    .with_settlement_records(
        settlement_records,
        treasury_records,
        world.settlement_store().next_settlement_id(),
        world.settlement_store().next_treasury_id(),
    );
    scene.inventory_persistence = super::inventory_snapshot::capture_inventory_persistence(world);
    scene
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
            current_space_id: record.current_space_id.raw(),
            inventory_id: record.inventory_id.map(|id| id.raw()),
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
        record.current_space_id = crate::world::SpaceId::new(self.current_space_id);
        record.state = self.state.to_state()?;
        record.inventory_id = self.inventory_id.map(crate::world::InventoryId::new);
        Ok(record)
    }
}

impl SceneBuildingRecord {
    pub fn from_record(record: &BuildingRecord, world: &WorldData) -> Self {
        let door_states = world
            .door_store()
            .building_door_ids(record.id)
            .iter()
            .filter_map(|door_id| {
                world
                    .door_store()
                    .get(*door_id)
                    .map(|door| SceneDoorSnapshot {
                        definition_key: door.definition_key.clone(),
                        state: door.state.label().to_string(),
                        access: door.access.label().to_string(),
                    })
            })
            .collect();
        Self {
            id: record.id.raw(),
            definition_id: record.definition_id.as_str().to_string(),
            position: SceneWorldPosition::from_world(record.placement.position),
            rotation: SceneQuat::from_quat(record.placement.rotation),
            lifecycle_state: record.lifecycle_state.label().to_string(),
            progress_0_1: record.construction.progress_0_1,
            current_hp: record.vitals.current_hp,
            max_hp: record.vitals.max_hp,
            source: SceneBuildingSource::from_source(record.source),
            owner_id: record.ownership.owner_id.map(|id| id.raw()),
            team_id: record.ownership.team_id.map(|id| id.raw()),
            affiliation: Some(record.ownership.affiliation.label().to_string()),
            parent_building_id: record.parent_building_id.map(|id| id.raw()),
            interior_activated: record.interior.activated,
            interior_profile_id: record.interior.profile_id.clone(),
            child_doodad_ids: record.interior.child_doodad_ids.clone(),
            child_building_ids: record.interior.child_building_ids.clone(),
            door_states,
            inventory_id: record.inventory_id.map(|id| u64::from(id.raw())),
            container_locked: record.container_locked,
        }
    }

    pub fn to_record(&self) -> Result<BuildingRecord, SceneRecordError> {
        let source = self.source.to_source();
        let mut ownership = crate::world::BuildingOwnership::neutral();
        if let Some(owner) = self.owner_id {
            ownership.owner_id = Some(OwnerId::new(owner));
        }
        if let Some(team) = self.team_id {
            ownership.team_id = Some(TeamId::new(team));
        }
        if let Some(label) = self.affiliation.as_deref() {
            ownership.affiliation = affiliation_from_label(label);
        }
        let lifecycle_state = parse_building_lifecycle(&self.lifecycle_state)?;
        if !self.progress_0_1.is_finite() || !(0.0..=1.0).contains(&self.progress_0_1) {
            return Err(SceneRecordError::InvalidBuildingProgress);
        }
        if self.max_hp == 0 || self.current_hp > self.max_hp {
            return Err(SceneRecordError::InvalidBuildingVitals);
        }
        Ok(BuildingRecord {
            id: crate::world::BuildingId::new(self.id),
            definition_id: crate::world::BuildingDefinitionId::new(&self.definition_id),
            placement: crate::world::BuildingPlacement::new(
                self.position.to_world()?,
                self.rotation.to_quat(),
            ),
            ownership,
            vitals: BuildingVitals::clamped(self.current_hp, self.max_hp),
            lifecycle_state,
            spaces: Default::default(),
            construction: ConstructionState {
                progress_0_1: self.progress_0_1,
            },
            source,
            interior: crate::world::BuildingInteriorState {
                profile_id: self.interior_profile_id.clone(),
                door_ids: Vec::new(),
                child_doodad_ids: self.child_doodad_ids.clone(),
                child_building_ids: self.child_building_ids.clone(),
                activated: self.interior_activated,
                interior_space_id: None,
            },
            parent_building_id: self.parent_building_id.map(crate::world::BuildingId::new),
            inventory_id: match self.inventory_id {
                Some(id) => Some(crate::world::InventoryId::new(
                    u32::try_from(id).map_err(|_| SceneRecordError::InvalidBuildingProgress)?,
                )),
                None => None,
            },
            container_locked: self.container_locked,
        })
    }

    pub fn door_states(&self) -> &[SceneDoorSnapshot] {
        &self.door_states
    }
}

impl SceneBuildingSource {
    pub fn from_source(source: BuildingSource) -> Self {
        match source {
            BuildingSource::Authored => Self::Authored,
            BuildingSource::Dev => Self::Dev,
        }
    }

    pub fn to_source(self) -> BuildingSource {
        match self {
            Self::Authored => BuildingSource::Authored,
            Self::Dev => BuildingSource::Dev,
        }
    }
}

fn parse_building_lifecycle(label: &str) -> Result<BuildingLifecycleState, SceneRecordError> {
    Ok(match label {
        "Complete" => BuildingLifecycleState::Complete,
        "Planned" => BuildingLifecycleState::Planned,
        "Foundation" => BuildingLifecycleState::Foundation,
        "InProgress" => BuildingLifecycleState::InProgress,
        "Destroyed" => BuildingLifecycleState::Destroyed,
        "Ruins" => BuildingLifecycleState::Ruins,
        _ => return Err(SceneRecordError::InvalidBuildingLifecycle),
    })
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
            parent_building_id: record.metadata.parent_building_id.map(|id| id.raw()),
            interior_space_id: record.metadata.interior_space_id.map(|id| id.raw()),
        }
    }

    pub fn to_record(&self, kind: DoodadKind) -> Result<DoodadRecord, SceneRecordError> {
        let mut record = DoodadRecord::new(
            DoodadId::new(self.id),
            crate::world::DoodadDefinitionId::new(&self.definition_id),
            kind,
            crate::world::DoodadPlacement::new(
                self.position.to_world()?,
                self.rotation.to_quat(),
                Vec3::new(self.scale[0], self.scale[1], self.scale[2]),
            ),
            self.source.to_source(),
        );
        record.metadata.parent_building_id =
            self.parent_building_id.map(crate::world::BuildingId::new);
        record.metadata.interior_space_id = self.interior_space_id.map(crate::world::SpaceId::new);
        Ok(record)
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
            UnitState::Working { task_id } => Self::Working {
                task_id: task_id.raw(),
            },
            UnitState::Moving {
                target,
                path,
                waypoint_index,
            } => Self::Moving {
                target: SceneWorldPosition::from_world(*target),
                waypoints: path
                    .waypoints
                    .iter()
                    .map(|waypoint| SceneWorldPosition::from_world(waypoint.position))
                    .collect(),
                waypoint_index: *waypoint_index,
            },
            UnitState::Dead => Self::Dead,
        }
    }

    pub fn to_state(&self) -> Result<UnitState, SceneRecordError> {
        Ok(match self {
            Self::Idle => UnitState::Idle,
            Self::Working { task_id } => UnitState::Working {
                task_id: TaskId::new(*task_id),
            },
            Self::Dead => UnitState::Dead,
            Self::Moving {
                target,
                waypoints,
                waypoint_index,
            } => UnitState::Moving {
                target: target.to_world()?,
                path: crate::world::NavigationPath::from_surface_positions(
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

impl SceneTaskRecord {
    pub fn from_record(record: &TaskRecord) -> Self {
        let (target_building_id, interaction_point_key) = match &record.target {
            TaskTarget::Building(id) => (id.raw(), None),
            TaskTarget::InteractionPoint {
                building_id,
                point_key,
            } => (building_id.raw(), Some(point_key.clone())),
        };
        Self {
            id: record.id.raw(),
            task_type: record.task_type.label().to_string(),
            target_building_id,
            interaction_point_key,
            state: task_state_label(record.state),
            priority: task_priority_label(record.priority),
            assigned_unit_id: record.assigned_unit_id.map(|id| id.raw()),
            reserved_point_key: record.reserved_point_key.clone(),
            created_tick: record.created_tick,
        }
    }

    pub fn to_record(&self) -> Result<TaskRecord, SceneRecordError> {
        let task_type = parse_task_type(&self.task_type)?;
        let state = parse_task_state(&self.state)?;
        let priority = parse_task_priority(&self.priority)?;
        let target = if let Some(point_key) = &self.interaction_point_key {
            TaskTarget::InteractionPoint {
                building_id: crate::world::BuildingId::new(self.target_building_id),
                point_key: point_key.clone(),
            }
        } else {
            TaskTarget::Building(crate::world::BuildingId::new(self.target_building_id))
        };
        Ok(TaskRecord {
            id: TaskId::new(self.id),
            task_type,
            target,
            state,
            priority,
            assigned_unit_id: self.assigned_unit_id.map(UnitId::new),
            reserved_point_key: self.reserved_point_key.clone(),
            created_tick: self.created_tick,
        })
    }
}

fn task_state_label(state: TaskState) -> String {
    match state {
        TaskState::Available => "Available".into(),
        TaskState::Assigned => "Assigned".into(),
        TaskState::InProgress => "InProgress".into(),
        TaskState::Completed => "Completed".into(),
        TaskState::Canceled => "Canceled".into(),
    }
}

fn task_priority_label(priority: TaskPriority) -> String {
    match priority {
        TaskPriority::PlayerAssigned => "PlayerAssigned".into(),
        TaskPriority::High => "High".into(),
        TaskPriority::Normal => "Normal".into(),
        TaskPriority::Low => "Low".into(),
    }
}

fn parse_task_type(label: &str) -> Result<TaskType, SceneRecordError> {
    Ok(match label {
        "ConstructBuilding" => TaskType::ConstructBuilding,
        "OperateWorkstation" => TaskType::OperateWorkstation,
        _ => return Err(SceneRecordError::InvalidTaskType),
    })
}

fn parse_task_state(label: &str) -> Result<TaskState, SceneRecordError> {
    Ok(match label {
        "Available" => TaskState::Available,
        "Assigned" => TaskState::Assigned,
        "InProgress" => TaskState::InProgress,
        "Completed" => TaskState::Completed,
        "Canceled" => TaskState::Canceled,
        _ => return Err(SceneRecordError::InvalidTaskState),
    })
}

fn parse_task_priority(label: &str) -> Result<TaskPriority, SceneRecordError> {
    Ok(match label {
        "PlayerAssigned" => TaskPriority::PlayerAssigned,
        "High" => TaskPriority::High,
        "Normal" => TaskPriority::Normal,
        "Low" => TaskPriority::Low,
        _ => return Err(SceneRecordError::InvalidTaskPriority),
    })
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

pub(crate) fn affiliation_from_label(label: &str) -> Affiliation {
    match label {
        "Player" => Affiliation::Player,
        "Neutral" => Affiliation::Neutral,
        "Hostile" => Affiliation::Hostile,
        "Wildlife" => Affiliation::Wildlife,
        "Dev" => Affiliation::Dev,
        _ => Affiliation::Unknown,
    }
}

impl SceneSettlementRecord {
    pub fn from_record(record: &crate::world::SettlementRecord) -> Self {
        Self {
            id: record.id.raw(),
            display_name: record.display_name.clone(),
            treasury_id: record.treasury_id.raw(),
            anchor_building_id: record.anchor_building_id.raw(),
            owner_id: record.ownership.owner_id.map(|id| id.raw()),
            team_id: record.ownership.team_id.map(|id| id.raw()),
            affiliation: Some(record.ownership.affiliation.label().to_string()),
            interaction_position: SceneWorldPosition::from_world(record.interaction_position),
            created_tick: record.created_tick,
        }
    }

    pub fn to_record(&self) -> Result<crate::world::SettlementRecord, SceneRecordError> {
        let mut ownership = crate::world::SettlementOwnership::player_default();
        if let Some(owner) = self.owner_id {
            ownership.owner_id = Some(OwnerId::new(owner));
        }
        if let Some(team) = self.team_id {
            ownership.team_id = Some(TeamId::new(team));
        }
        if let Some(label) = self.affiliation.as_deref() {
            ownership.affiliation = affiliation_from_label(label);
        }
        Ok(crate::world::SettlementRecord {
            id: crate::world::SettlementId::new(self.id),
            display_name: self.display_name.clone(),
            treasury_id: crate::world::TreasuryId::new(self.treasury_id),
            anchor_building_id: crate::world::BuildingId::new(self.anchor_building_id),
            ownership,
            interaction_position: self.interaction_position.to_world()?,
            created_tick: self.created_tick,
        })
    }
}

impl SceneTreasuryRecord {
    pub fn from_record(record: &crate::world::SettlementTreasuryRecord) -> Self {
        Self {
            id: record.id.raw(),
            settlement_id: record.settlement_id.raw(),
            balance_gold: record.balance_gold,
            created_tick: record.created_tick,
            metadata: record.metadata.clone(),
            owner_id: record.ownership.owner_id.map(|id| id.raw()),
            team_id: record.ownership.team_id.map(|id| id.raw()),
            affiliation: Some(record.ownership.affiliation.label().to_string()),
        }
    }

    pub fn to_record(&self) -> Result<crate::world::SettlementTreasuryRecord, SceneRecordError> {
        let mut ownership = crate::world::SettlementOwnership::player_default();
        if let Some(owner) = self.owner_id {
            ownership.owner_id = Some(OwnerId::new(owner));
        }
        if let Some(team) = self.team_id {
            ownership.team_id = Some(TeamId::new(team));
        }
        if let Some(label) = self.affiliation.as_deref() {
            ownership.affiliation = affiliation_from_label(label);
        }
        Ok(crate::world::SettlementTreasuryRecord {
            id: crate::world::TreasuryId::new(self.id),
            settlement_id: crate::world::SettlementId::new(self.settlement_id),
            ownership,
            balance_gold: self.balance_gold,
            created_tick: self.created_tick,
            metadata: self.metadata.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneRecordError {
    InvalidPosition,
    UnknownDoodadKind(String),
    InvalidBuildingLifecycle,
    InvalidBuildingProgress,
    InvalidBuildingVitals,
    InvalidTaskType,
    InvalidTaskState,
    InvalidTaskPriority,
}
