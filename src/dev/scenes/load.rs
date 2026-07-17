//! Apply dev scenes to authoritative [`WorldData`] (ADR-045, REVIEW-A5).

use std::collections::HashSet;
use std::time::Instant;

use bevy::prelude::*;

use crate::world::{
    BuildingCatalog, BuildingRecord, ChunkId, DoodadCatalog, DoodadRecord, DoodadRestoreError,
    DoorAccessPolicy, DoorState, FootprintCatalog, InteriorProfileCatalog, OccupancyCatalogs,
    TaskRecord, UnitCatalog, UnitRecord, UnitRestoreError, WorldData,
    rebuild_building_world_indexes, restore_doodad_record, restore_unit_record,
    validate_building_for_restore, validate_doodad_for_restore, validate_unit_for_restore,
};

use super::inventory_snapshot::SceneInventoryPersistence;
use super::snapshot::{
    SCENE_VERSION, SceneBuildingRecord, SceneDefinition, SceneDoodadRecord, SceneSettlementRecord,
    SceneTaskRecord, SceneTreasuryRecord, SceneUnitRecord, parse_doodad_kind,
    scene_version_supported,
};

/// Outcome of applying a scene to world data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SceneApplyReport {
    pub units_loaded: u32,
    pub doodads_loaded: u32,
    pub buildings_loaded: u32,
    pub world_seed: u64,
    pub elapsed_ms: u64,
    pub transient_state_cleared: bool,
}

/// Why scene application failed (world left unchanged when validation fails first).
#[derive(Debug, Clone, PartialEq)]
pub enum SceneApplyError {
    UnsupportedVersion {
        found: u32,
        expected: u32,
    },
    MissingUnitDefinition {
        unit_id: u64,
        definition_id: String,
    },
    DisabledUnitDefinition {
        unit_id: u64,
        definition_id: String,
    },
    MissingDoodadDefinition {
        doodad_id: u64,
        definition_id: String,
    },
    DisabledDoodadDefinition {
        doodad_id: u64,
        definition_id: String,
    },
    DuplicateUnitId {
        unit_id: u64,
    },
    DuplicateDoodadId {
        doodad_id: u64,
    },
    DuplicateProceduralKey {
        doodad_id: u64,
    },
    InvalidUnitRecord {
        unit_id: u64,
        reason: String,
    },
    InvalidDoodadRecord {
        doodad_id: u64,
        reason: String,
    },
    InvalidVitals {
        unit_id: u64,
        reason: String,
    },
    PlacementChunkMismatch {
        entity: &'static str,
        id: u64,
    },
    IndexConsistencyFailure(&'static str),
    UnitRestore(UnitRestoreError),
    DoodadRestore(DoodadRestoreError),
    MissingBuildingDefinition {
        building_id: u64,
        definition_id: String,
    },
    DuplicateBuildingId {
        building_id: u64,
    },
    InvalidBuildingRecord {
        building_id: u64,
        reason: String,
    },
    BuildingRestore(crate::world::BuildingRestoreError),
    DuplicateTaskId {
        task_id: u32,
    },
    InvalidTaskRecord {
        task_id: u32,
        reason: String,
    },
    TaskRestore(crate::world::TaskError),
    SettlementRestore {
        reason: String,
    },
    InventoryRestore {
        reason: String,
    },
}

impl std::fmt::Display for SceneApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion { found, expected } => {
                write!(f, "unsupported scene version {found} (expected {expected})")
            }
            Self::MissingUnitDefinition {
                unit_id,
                definition_id,
            } => write!(f, "unit {unit_id}: missing definition {definition_id}"),
            Self::DisabledUnitDefinition {
                unit_id,
                definition_id,
            } => write!(f, "unit {unit_id}: disabled definition {definition_id}"),
            Self::MissingDoodadDefinition {
                doodad_id,
                definition_id,
            } => write!(f, "doodad {doodad_id}: missing definition {definition_id}"),
            Self::DisabledDoodadDefinition {
                doodad_id,
                definition_id,
            } => write!(f, "doodad {doodad_id}: disabled definition {definition_id}"),
            Self::DuplicateUnitId { unit_id } => write!(f, "duplicate unit id {unit_id}"),
            Self::DuplicateDoodadId { doodad_id } => write!(f, "duplicate doodad id {doodad_id}"),
            Self::DuplicateProceduralKey { doodad_id } => {
                write!(f, "duplicate procedural doodad key for id {doodad_id}")
            }
            Self::InvalidUnitRecord { unit_id, reason } => {
                write!(f, "invalid unit record {unit_id}: {reason}")
            }
            Self::InvalidDoodadRecord { doodad_id, reason } => {
                write!(f, "invalid doodad record {doodad_id}: {reason}")
            }
            Self::InvalidVitals { unit_id, reason } => {
                write!(f, "invalid vitals for unit {unit_id}: {reason}")
            }
            Self::PlacementChunkMismatch { entity, id } => {
                write!(f, "{entity} {id}: placement chunk mismatch")
            }
            Self::IndexConsistencyFailure(reason) => {
                write!(f, "index consistency failure: {reason}")
            }
            Self::UnitRestore(err) => write!(f, "unit restore failed: {err:?}"),
            Self::DoodadRestore(err) => write!(f, "doodad restore failed: {err:?}"),
            Self::MissingBuildingDefinition {
                building_id,
                definition_id,
            } => write!(
                f,
                "building {building_id}: missing definition {definition_id}"
            ),
            Self::DuplicateBuildingId { building_id } => {
                write!(f, "duplicate building id {building_id}")
            }
            Self::InvalidBuildingRecord {
                building_id,
                reason,
            } => {
                write!(f, "invalid building record {building_id}: {reason}")
            }
            Self::BuildingRestore(err) => write!(f, "building restore failed: {err}"),
            Self::DuplicateTaskId { task_id } => write!(f, "duplicate task id {task_id}"),
            Self::InvalidTaskRecord { task_id, reason } => {
                write!(f, "invalid task record {task_id}: {reason}")
            }
            Self::TaskRestore(err) => write!(f, "task restore failed: {err:?}"),
            Self::SettlementRestore { reason } => write!(f, "settlement restore failed: {reason}"),
            Self::InventoryRestore { reason } => write!(f, "inventory restore failed: {reason}"),
        }
    }
}

impl From<UnitRestoreError> for SceneApplyError {
    fn from(err: UnitRestoreError) -> Self {
        match err {
            UnitRestoreError::MissingUnitDefinition {
                unit_id,
                definition_id,
            } => Self::MissingUnitDefinition {
                unit_id,
                definition_id,
            },
            UnitRestoreError::DisabledUnitDefinition {
                unit_id,
                definition_id,
            } => Self::DisabledUnitDefinition {
                unit_id,
                definition_id,
            },
            UnitRestoreError::DuplicateUnitId { unit_id } => Self::DuplicateUnitId { unit_id },
            UnitRestoreError::PlacementChunkMismatch { unit_id } => Self::PlacementChunkMismatch {
                entity: "unit",
                id: unit_id,
            },
            UnitRestoreError::InvalidVitals { unit_id, reason } => Self::InvalidVitals {
                unit_id,
                reason: reason.to_string(),
            },
            UnitRestoreError::IndexInsert { unit_id, error } => Self::InvalidUnitRecord {
                unit_id,
                reason: format!("{error:?}"),
            },
        }
    }
}

impl From<DoodadRestoreError> for SceneApplyError {
    fn from(err: DoodadRestoreError) -> Self {
        match err {
            DoodadRestoreError::MissingDoodadDefinition {
                doodad_id,
                definition_id,
            } => Self::MissingDoodadDefinition {
                doodad_id,
                definition_id,
            },
            DoodadRestoreError::DisabledDoodadDefinition {
                doodad_id,
                definition_id,
            } => Self::DisabledDoodadDefinition {
                doodad_id,
                definition_id,
            },
            DoodadRestoreError::DuplicateDoodadId { doodad_id } => {
                Self::DuplicateDoodadId { doodad_id }
            }
            DoodadRestoreError::DuplicateProceduralKey { doodad_id } => {
                Self::DuplicateProceduralKey { doodad_id }
            }
            DoodadRestoreError::PlacementChunkMismatch { doodad_id } => {
                Self::PlacementChunkMismatch {
                    entity: "doodad",
                    id: doodad_id,
                }
            }
            DoodadRestoreError::ScaleOutOfRange {
                doodad_id,
                min,
                max,
            } => Self::InvalidDoodadRecord {
                doodad_id,
                reason: format!("scale out of range [{min}, {max}]"),
            },
            DoodadRestoreError::DefinitionKindMismatch {
                doodad_id,
                expected,
                found,
            } => Self::InvalidDoodadRecord {
                doodad_id,
                reason: format!("kind mismatch (expected {expected:?}, found {found:?})"),
            },
            DoodadRestoreError::IndexInsert { doodad_id, error } => Self::InvalidDoodadRecord {
                doodad_id,
                reason: format!("{error:?}"),
            },
        }
    }
}

struct RestorePlan {
    units: Vec<UnitRecord>,
    doodads: Vec<DoodadRecord>,
    buildings: Vec<BuildingRecord>,
    tasks: Vec<TaskRecord>,
    settlements: Vec<crate::world::SettlementRecord>,
    treasuries: Vec<crate::world::SettlementTreasuryRecord>,
    next_unit_id: u64,
    next_doodad_id: u64,
    next_building_id: u64,
    next_task_id: u32,
    next_door_id: u32,
    next_space_id: u32,
    next_portal_id: u32,
    next_settlement_id: u64,
    next_treasury_id: u64,
    inventory_persistence: SceneInventoryPersistence,
}

struct DevWorldEntityBackup {
    units: Vec<UnitRecord>,
    doodads: Vec<DoodadRecord>,
    next_unit_id: u64,
    next_doodad_id: u64,
}

impl DevWorldEntityBackup {
    fn capture(world: &WorldData) -> Self {
        let units = world
            .sorted_unit_ids()
            .into_iter()
            .filter_map(|id| world.get_unit(id).cloned())
            .collect();
        let doodads = world
            .sorted_doodad_ids()
            .into_iter()
            .filter_map(|id| world.get_doodad(id).cloned())
            .collect();
        Self {
            units,
            doodads,
            next_unit_id: world.dev_next_unit_id(),
            next_doodad_id: world.dev_next_doodad_id(),
        }
    }

    fn restore(
        &self,
        world: &mut WorldData,
        unit_catalog: &UnitCatalog,
        doodad_catalog: &DoodadCatalog,
    ) -> Result<(), SceneApplyError> {
        world.dev_clear_units_and_doodads();
        let mut unit_ids = HashSet::new();
        for unit in &self.units {
            restore_unit_record(world, unit_catalog, unit.clone(), &mut unit_ids)?;
        }
        let mut doodad_ids = HashSet::new();
        let mut procedural_keys = HashSet::new();
        for doodad in &self.doodads {
            restore_doodad_record(
                world,
                doodad_catalog,
                doodad.clone(),
                &mut doodad_ids,
                &mut procedural_keys,
            )?;
        }
        world.dev_restore_id_counters(self.next_unit_id, self.next_doodad_id, 1);
        world
            .verify_instance_indexes()
            .map_err(SceneApplyError::IndexConsistencyFailure)?;
        Ok(())
    }
}

/// Clear all unit and doodad instances from world data (dev-only).
pub fn clear_world_entities(world: &mut WorldData) {
    world.dev_clear_units_and_doodads();
}

/// Validate and apply a scene through validated restore APIs only (REVIEW-A5).
pub fn apply_scene(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    interior_catalog: &InteriorProfileCatalog,
    scene: &SceneDefinition,
) -> Result<SceneApplyReport, SceneApplyError> {
    let plan = build_restore_plan(unit_catalog, doodad_catalog, building_catalog, scene)?;
    let backup = DevWorldEntityBackup::capture(world);
    let started = Instant::now();

    world.dev_clear_units_and_doodads();

    let apply_result = apply_restore_plan(world, unit_catalog, doodad_catalog, &plan);
    if let Err(err) = apply_result {
        if let Err(rollback_err) = backup.restore(world, unit_catalog, doodad_catalog) {
            error!("dev scene rollback failed after apply error: {rollback_err}");
        }
        return Err(err);
    }

    let occ = OccupancyCatalogs {
        doodad: doodad_catalog,
        building: building_catalog,
        footprint: footprint_catalog,
    };
    reconcile_building_interiors_after_scene_load(
        world,
        building_catalog,
        interior_catalog,
        doodad_catalog,
        occ,
        &scene.building_records,
    );

    if !plan.tasks.is_empty() {
        world
            .task_store_mut()
            .restore_snapshot(plan.tasks.clone())
            .map_err(SceneApplyError::TaskRestore)?;
    }

    rebuild_building_world_indexes(
        world,
        building_catalog,
        footprint_catalog,
        doodad_catalog,
        0,
    )
    .map_err(|error| SceneApplyError::InvalidBuildingRecord {
        building_id: 0,
        reason: error.to_string(),
    })?;

    world.dev_restore_id_counters(
        plan.next_unit_id,
        plan.next_doodad_id,
        plan.next_building_id,
    );
    world.dev_restore_building_runtime_counters(
        plan.next_task_id,
        plan.next_door_id,
        plan.next_space_id,
        plan.next_portal_id,
    );
    world
        .verify_instance_indexes()
        .map_err(SceneApplyError::IndexConsistencyFailure)?;

    if !plan.inventory_persistence.inventory_records.is_empty()
        || !plan.inventory_persistence.corpse_records.is_empty()
        || !plan.inventory_persistence.item_pile_records.is_empty()
    {
        let ctx = dev_inventory_catalog_ctx();
        let validation = crate::world::validate_world_inventory_state(world, &ctx);
        if !validation.is_ok() {
            return Err(SceneApplyError::InventoryRestore {
                reason: format!("post-restore validation: {validation:?}"),
            });
        }
    }

    let elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let report = SceneApplyReport {
        units_loaded: plan.units.len() as u32,
        doodads_loaded: plan.doodads.len() as u32,
        buildings_loaded: plan.buildings.len() as u32,
        world_seed: scene.world_seed,
        elapsed_ms,
        transient_state_cleared: true,
    };

    info!(
        "dev scene loaded: units={} doodads={} buildings={} seed={} took={}ms transient_cleared=true",
        report.units_loaded,
        report.doodads_loaded,
        report.buildings_loaded,
        report.world_seed,
        report.elapsed_ms
    );

    Ok(report)
}

fn build_restore_plan(
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    scene: &SceneDefinition,
) -> Result<RestorePlan, SceneApplyError> {
    if !scene_version_supported(scene.version) {
        return Err(SceneApplyError::UnsupportedVersion {
            found: scene.version,
            expected: SCENE_VERSION,
        });
    }

    let mut units = Vec::with_capacity(scene.unit_records.len());
    let mut unit_ids = HashSet::new();
    for unit in &scene.unit_records {
        let record = scene_unit_to_record(unit)?;
        validate_unit_for_restore(unit_catalog, &record, &unit_ids)?;
        unit_ids.insert(record.id);
        units.push(record);
    }

    let mut doodads = Vec::with_capacity(scene.doodad_records.len());
    let mut doodad_ids = HashSet::new();
    let mut procedural_keys = HashSet::new();
    for doodad in &scene.doodad_records {
        let record = scene_doodad_to_record(doodad, doodad_catalog)?;
        validate_doodad_for_restore(doodad_catalog, &record, &doodad_ids, &procedural_keys)?;
        doodad_ids.insert(record.id);
        if let Some(key) = crate::world::ProceduralDoodadKey::from_record(&record) {
            procedural_keys.insert(key);
        }
        doodads.push(record);
    }

    let mut buildings = Vec::with_capacity(scene.building_records.len());
    let mut building_ids = HashSet::new();
    for building in &scene.building_records {
        let record = scene_building_to_record(building)?;
        validate_building_for_restore(building_catalog, &record, &building_ids)
            .map_err(SceneApplyError::BuildingRestore)?;
        building_ids.insert(record.id);
        buildings.push(record);
    }

    let unit_id_set: HashSet<_> = unit_ids.iter().copied().collect();
    let mut tasks = Vec::with_capacity(scene.task_records.len());
    let mut task_ids = HashSet::new();
    for task in &scene.task_records {
        let record = scene_task_to_record(task)?;
        if task_ids.contains(&record.id) {
            return Err(SceneApplyError::DuplicateTaskId {
                task_id: record.id.raw(),
            });
        }
        let building_id = record.target_building_id();
        if !building_ids.contains(&building_id) {
            return Err(SceneApplyError::InvalidTaskRecord {
                task_id: record.id.raw(),
                reason: format!("unknown building {}", building_id.raw()),
            });
        }
        if let Some(unit_id) = record.assigned_unit_id {
            if !unit_id_set.contains(&unit_id) {
                return Err(SceneApplyError::InvalidTaskRecord {
                    task_id: record.id.raw(),
                    reason: format!("unknown unit {}", unit_id.raw()),
                });
            }
        }
        task_ids.insert(record.id);
        tasks.push(record);
    }

    let mut settlements = Vec::with_capacity(scene.settlement_records.len());
    let mut settlement_ids = HashSet::new();
    for settlement in &scene.settlement_records {
        let record = scene_settlement_to_record(settlement)?;
        if settlement_ids.contains(&record.id) {
            return Err(SceneApplyError::SettlementRestore {
                reason: format!("duplicate settlement id {}", record.id.raw()),
            });
        }
        if !building_ids.contains(&record.anchor_building_id) {
            return Err(SceneApplyError::SettlementRestore {
                reason: format!(
                    "unknown anchor building {} for settlement {}",
                    record.anchor_building_id.raw(),
                    record.id.raw()
                ),
            });
        }
        settlement_ids.insert(record.id);
        settlements.push(record);
    }

    let mut treasuries = Vec::with_capacity(scene.treasury_records.len());
    let mut treasury_ids = HashSet::new();
    for treasury in &scene.treasury_records {
        let record = scene_treasury_to_record(treasury)?;
        if treasury_ids.contains(&record.id) {
            return Err(SceneApplyError::SettlementRestore {
                reason: format!("duplicate treasury id {}", record.id.raw()),
            });
        }
        if !settlement_ids.contains(&record.settlement_id) {
            return Err(SceneApplyError::SettlementRestore {
                reason: format!(
                    "unknown settlement {} for treasury {}",
                    record.settlement_id.raw(),
                    record.id.raw()
                ),
            });
        }
        treasury_ids.insert(record.id);
        treasuries.push(record);
    }

    Ok(RestorePlan {
        units,
        doodads,
        buildings,
        tasks,
        settlements,
        treasuries,
        next_unit_id: scene.next_unit_id,
        next_doodad_id: scene.next_doodad_id,
        next_building_id: scene.next_building_id.max(1),
        next_task_id: scene.next_task_id.max(1),
        next_door_id: scene.next_door_id.max(1),
        next_space_id: scene.next_space_id.max(1),
        next_portal_id: scene.next_portal_id.max(1),
        next_settlement_id: scene.next_settlement_id.max(1),
        next_treasury_id: scene.next_treasury_id.max(1),
        inventory_persistence: scene.inventory_persistence.clone(),
    })
}

fn apply_restore_plan(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    plan: &RestorePlan,
) -> Result<(), SceneApplyError> {
    let mut unit_ids = HashSet::new();
    for unit in &plan.units {
        restore_unit_record(world, unit_catalog, unit.clone(), &mut unit_ids)?;
    }

    let mut doodad_ids = HashSet::new();
    let mut procedural_keys = HashSet::new();
    for doodad in &plan.doodads {
        restore_doodad_record(
            world,
            doodad_catalog,
            doodad.clone(),
            &mut doodad_ids,
            &mut procedural_keys,
        )?;
    }

    for building in &plan.buildings {
        let chunk = ChunkId::new(building.placement.position.chunk);
        world
            .insert_building(chunk, building.clone())
            .map_err(|error| SceneApplyError::InvalidBuildingRecord {
                building_id: building.id.raw(),
                reason: format!("{error:?}"),
            })?;
    }

    world.settlement_store_mut().clear();
    if !plan.settlements.is_empty() || !plan.treasuries.is_empty() {
        world
            .settlement_store_mut()
            .restore_snapshot(
                plan.settlements.clone(),
                plan.treasuries.clone(),
                plan.next_settlement_id,
                plan.next_treasury_id,
            )
            .map_err(|error| SceneApplyError::SettlementRestore {
                reason: error.to_string(),
            })?;
    }

    if !plan.inventory_persistence.inventory_records.is_empty()
        || !plan.inventory_persistence.item_instance_records.is_empty()
        || !plan.inventory_persistence.corpse_records.is_empty()
        || !plan.inventory_persistence.item_pile_records.is_empty()
    {
        let ctx = dev_inventory_catalog_ctx();
        super::inventory_snapshot::restore_inventory_persistence(
            world,
            &plan.inventory_persistence,
            &ctx,
        )
        .map_err(|reason| SceneApplyError::InventoryRestore { reason })?;
    }

    Ok(())
}

fn dev_inventory_catalog_ctx() -> &'static crate::world::InventoryCatalogCtx<'static> {
    use std::sync::OnceLock;
    static CTX: OnceLock<crate::world::InventoryCatalogCtx<'static>> = OnceLock::new();
    CTX.get_or_init(|| {
        let categories = Box::leak(Box::new(
            crate::world::ItemCategoryCatalog::from_definitions(
                crate::world::starter_item_category_definitions(),
            )
            .unwrap(),
        ));
        let items = Box::leak(Box::new(
            crate::world::ItemCatalog::from_definitions(
                crate::world::starter_item_definitions(),
                categories,
            )
            .unwrap(),
        ));
        let profiles = Box::leak(Box::new(
            crate::world::InventoryProfileCatalog::from_definitions(
                crate::world::starter_inventory_profile_definitions(),
            )
            .unwrap(),
        ));
        crate::world::InventoryCatalogCtx::new(items, categories, profiles)
    })
}

fn reconcile_building_interiors_after_scene_load(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &crate::world::InteriorProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    scene_buildings: &[SceneBuildingRecord],
) {
    use crate::world::{BuildingId, InteriorProfileId, activate_building_interior};

    for scene in scene_buildings {
        if !scene.interior_activated {
            continue;
        }
        let building_id = BuildingId::new(scene.id);
        let profile_key = scene.interior_profile_id.clone().or_else(|| {
            world
                .get_building(building_id)
                .and_then(|record| building_catalog.get(&record.definition_id))
                .and_then(|definition| definition.interior_profile_id.clone())
        });
        let Some(profile_key) = profile_key else {
            continue;
        };
        if world.door_store().building_door_ids(building_id).is_empty() {
            let _ = activate_building_interior(
                world,
                building_catalog,
                interior_catalog,
                doodad_catalog,
                occupancy,
                building_id,
                &InteriorProfileId::new(profile_key),
            );
        }
        for snapshot in scene.door_states() {
            let Some(state) = parse_door_state_label(&snapshot.state) else {
                continue;
            };
            let access = parse_door_access_label(&snapshot.access);
            for door_id in world.door_store().building_door_ids(building_id).to_vec() {
                let Some(door) = world.door_store().get(door_id) else {
                    continue;
                };
                if door.definition_key != snapshot.definition_key {
                    continue;
                }
                if let Some(door) = world.door_store_mut().get_mut(door_id) {
                    door.state = state;
                    door.access = access;
                }
                let _ = crate::world::DoorStore::sync_portal_enabled(world, door_id);
            }
        }
    }
}

fn parse_door_state_label(label: &str) -> Option<DoorState> {
    Some(match label {
        "Open" => DoorState::Open,
        "Closed" => DoorState::Closed,
        "Locked" => DoorState::Locked,
        "Destroyed" => DoorState::Destroyed,
        _ => return None,
    })
}

fn parse_door_access_label(label: &str) -> DoorAccessPolicy {
    match label {
        "OwnerOnly" => DoorAccessPolicy::OwnerOnly,
        "Team" => DoorAccessPolicy::Team,
        "Locked" => DoorAccessPolicy::Locked,
        _ => DoorAccessPolicy::Everyone,
    }
}

fn scene_task_to_record(task: &SceneTaskRecord) -> Result<TaskRecord, SceneApplyError> {
    task.to_record()
        .map_err(|err| SceneApplyError::InvalidTaskRecord {
            task_id: task.id,
            reason: format!("{err:?}"),
        })
}

fn scene_building_to_record(
    building: &SceneBuildingRecord,
) -> Result<BuildingRecord, SceneApplyError> {
    building
        .to_record()
        .map_err(|err| SceneApplyError::InvalidBuildingRecord {
            building_id: building.id,
            reason: format!("{err:?}"),
        })
}

fn scene_unit_to_record(unit: &SceneUnitRecord) -> Result<UnitRecord, SceneApplyError> {
    unit.to_record()
        .map_err(|err| SceneApplyError::InvalidUnitRecord {
            unit_id: unit.id,
            reason: format!("{err:?}"),
        })
}

fn scene_settlement_to_record(
    settlement: &SceneSettlementRecord,
) -> Result<crate::world::SettlementRecord, SceneApplyError> {
    settlement
        .to_record()
        .map_err(|err| SceneApplyError::SettlementRestore {
            reason: format!("settlement {}: {err:?}", settlement.id),
        })
}

fn scene_treasury_to_record(
    treasury: &SceneTreasuryRecord,
) -> Result<crate::world::SettlementTreasuryRecord, SceneApplyError> {
    treasury
        .to_record()
        .map_err(|err| SceneApplyError::SettlementRestore {
            reason: format!("treasury {}: {err:?}", treasury.id),
        })
}

fn scene_doodad_to_record(
    doodad: &SceneDoodadRecord,
    catalog: &DoodadCatalog,
) -> Result<DoodadRecord, SceneApplyError> {
    let definition_id = crate::world::DoodadDefinitionId::new(&doodad.definition_id);
    let definition =
        catalog
            .get(&definition_id)
            .ok_or_else(|| SceneApplyError::MissingDoodadDefinition {
                doodad_id: doodad.id,
                definition_id: doodad.definition_id.clone(),
            })?;
    if !definition.enabled {
        return Err(SceneApplyError::DisabledDoodadDefinition {
            doodad_id: doodad.id,
            definition_id: doodad.definition_id.clone(),
        });
    }
    let kind =
        parse_doodad_kind(&doodad.kind).map_err(|err| SceneApplyError::InvalidDoodadRecord {
            doodad_id: doodad.id,
            reason: format!("{err:?}"),
        })?;
    doodad
        .to_record(kind)
        .map_err(|err| SceneApplyError::InvalidDoodadRecord {
            doodad_id: doodad.id,
            reason: format!("{err:?}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::scenes::{SceneCaptureContext, capture_scene};
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadDefinitionId, DoodadPlacementOverrides,
        DoodadSource, Heightfield, LocalPosition, UnitDefinitionId, UnitSource, WorldPosition,
        create_doodad, create_unit,
    };

    fn flat_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn sample_scene(world: &WorldData) -> SceneDefinition {
        let ctx = SceneCaptureContext {
            name: "test".into(),
            description: String::new(),
            tags: Vec::new(),
            created_at: 42,
            world_seed: 7,
            camera_state: None,
            debug_flags: None,
        };
        capture_scene(world, &ctx)
    }

    #[test]
    fn scene_load_restores_unit_count_exactly() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(10.0, 10.0),
            UnitSource::Dev,
        )
        .unwrap();
        create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("deer"),
            pos(20.0, 20.0),
            UnitSource::Dev,
        )
        .unwrap();
        let scene = sample_scene(&world);
        world.dev_clear_units_and_doodads();
        assert_eq!(world.sorted_unit_ids().len(), 0);

        let report = apply_scene(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &InteriorProfileCatalog::default(),
            &scene,
        )
        .unwrap();
        assert_eq!(report.units_loaded, 2);
        assert_eq!(world.sorted_unit_ids().len(), 2);
        assert!(report.transient_state_cleared);
        world.verify_instance_indexes().unwrap();
    }

    #[test]
    fn scene_load_restores_doodad_count_exactly() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        create_doodad(
            &doodad_catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            pos(12.0, 12.0),
            DoodadSource::Dev,
            DoodadPlacementOverrides::default(),
            None,
        )
        .unwrap();
        let scene = sample_scene(&world);
        apply_scene(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &InteriorProfileCatalog::default(),
            &scene,
        )
        .unwrap();
        assert_eq!(world.sorted_doodad_ids().len(), 1);
    }

    #[test]
    fn world_cleared_before_load_removes_prior_state() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Dev,
        )
        .unwrap();
        create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("deer"),
            pos(2.0, 2.0),
            UnitSource::Dev,
        )
        .unwrap();
        let scene = sample_scene(&world);
        create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(3.0, 3.0),
            UnitSource::Dev,
        )
        .unwrap();
        apply_scene(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &InteriorProfileCatalog::default(),
            &scene,
        )
        .unwrap();
        assert_eq!(world.sorted_unit_ids().len(), 2);
    }

    #[test]
    fn invalid_scene_fails_without_corruption() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(5.0, 5.0),
            UnitSource::Dev,
        )
        .unwrap();
        let before = world.sorted_unit_ids();
        let mut scene = sample_scene(&world);
        scene.unit_records[0].definition_id = "missing_unit".into();
        let err = apply_scene(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &InteriorProfileCatalog::default(),
            &scene,
        )
        .unwrap_err();
        assert!(matches!(err, SceneApplyError::MissingUnitDefinition { .. }));
        assert_eq!(world.sorted_unit_ids(), before);
    }

    #[test]
    fn duplicate_unit_id_in_scene_rejects_load() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Dev,
        )
        .unwrap();
        let before = world.sorted_unit_ids();
        let mut scene = sample_scene(&world);
        scene.unit_records.push(scene.unit_records[0].clone());
        let err = apply_scene(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &InteriorProfileCatalog::default(),
            &scene,
        )
        .unwrap_err();
        assert!(matches!(err, SceneApplyError::DuplicateUnitId { .. }));
        assert_eq!(world.sorted_unit_ids(), before);
    }

    #[test]
    fn disabled_definition_rejects_load() {
        let mut world = flat_world();
        let mut unit_catalog = UnitCatalog::default();
        let mut wolf = unit_catalog
            .get(&UnitDefinitionId::new("wolf"))
            .unwrap()
            .clone();
        wolf.enabled = false;
        unit_catalog = UnitCatalog::from_definitions(vec![wolf]).unwrap();
        let doodad_catalog = DoodadCatalog::default();
        let mut scene = sample_scene(&world);
        scene.unit_records.push(SceneUnitRecord {
            id: 99,
            definition_id: "wolf".into(),
            position: super::super::snapshot::SceneWorldPosition {
                chunk_x: 0,
                chunk_z: 0,
                local_x: 1.0,
                local_y: 0.0,
                local_z: 1.0,
            },
            rotation: super::super::snapshot::SceneQuat {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            state: super::super::snapshot::SceneUnitState::Idle,
            source: super::super::snapshot::SceneUnitSource::Dev,
            owner_id: None,
            team_id: None,
            affiliation: None,
            current_space_id: 0,
            inventory_id: None,
        });
        let err = apply_scene(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &InteriorProfileCatalog::default(),
            &scene,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            SceneApplyError::DisabledUnitDefinition { .. }
        ));
    }

    #[test]
    fn repeated_save_load_cycle_is_stable() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(8.0, 8.0),
            UnitSource::Dev,
        )
        .unwrap();
        let scene_a = sample_scene(&world);
        apply_scene(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &InteriorProfileCatalog::default(),
            &scene_a,
        )
        .unwrap();
        let scene_b = sample_scene(&world);
        assert_eq!(scene_a.unit_records, scene_b.unit_records);
        assert_eq!(scene_a.doodad_records, scene_b.doodad_records);
    }

    #[test]
    fn scene_restore_preserves_unit_ids() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let id = create_unit(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(8.0, 8.0),
            UnitSource::Dev,
        )
        .unwrap()
        .id;
        let scene = sample_scene(&world);
        apply_scene(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &InteriorProfileCatalog::default(),
            &scene,
        )
        .unwrap();
        assert!(world.get_unit(id).is_some());
    }

    #[test]
    fn building_scene_round_trip_preserves_state() {
        use crate::world::{
            BuildingLifecycleState, BuildingOwnership, OccupancyCatalogs, place_player_building,
            sync_construction_tasks,
        };
        use bevy::prelude::Quat;

        let mut world = flat_world();
        let building_catalog = BuildingCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let footprint_catalog = FootprintCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let occ = OccupancyCatalogs {
            building: &building_catalog,
            doodad: &doodad_catalog,
            footprint: &footprint_catalog,
        };
        let building_id = place_player_building(
            &building_catalog,
            &mut world,
            &crate::world::BuildingDefinitionId::new("hut"),
            pos(40.0, 40.0),
            Quat::IDENTITY,
            BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
            occ,
        )
        .unwrap()
        .id;
        sync_construction_tasks(&mut world, &building_catalog, 0);
        let before_cells = world.occupancy_cell_count();
        let scene = sample_scene(&world);
        apply_scene(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &building_catalog,
            &footprint_catalog,
            &InteriorProfileCatalog::default(),
            &scene,
        )
        .unwrap();
        let record = world.get_building(building_id).unwrap();
        assert_eq!(record.lifecycle_state, BuildingLifecycleState::Planned);
        assert_eq!(world.occupancy_cell_count(), before_cells);
        assert!(!world.task_store().building_task_ids(building_id).is_empty());
    }

    #[test]
    fn missing_building_definition_fails_without_mutation() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let building_catalog = BuildingCatalog::default();
        let footprint_catalog = FootprintCatalog::default();
        let before_buildings = world.sorted_building_ids().len();
        let mut scene = sample_scene(&world);
        scene
            .building_records
            .push(super::super::snapshot::SceneBuildingRecord {
                id: 9001,
                definition_id: "missing_building".into(),
                position: super::super::snapshot::SceneWorldPosition {
                    chunk_x: 0,
                    chunk_z: 0,
                    local_x: 10.0,
                    local_y: 0.0,
                    local_z: 10.0,
                },
                rotation: super::super::snapshot::SceneQuat {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                uniform_scale_milli: 1_000,
                lifecycle_state: "Planned".into(),
                progress_0_1: 0.0,
                current_hp: 100,
                max_hp: 100,
                source: super::super::snapshot::SceneBuildingSource::Dev,
                owner_id: None,
                team_id: None,
                affiliation: None,
                parent_building_id: None,
                interior_activated: false,
                interior_profile_id: None,
                child_doodad_ids: Vec::new(),
                child_building_ids: Vec::new(),
                door_states: Vec::new(),
                inventory_id: None,
                container_locked: false,
            });
        let err = apply_scene(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &building_catalog,
            &footprint_catalog,
            &InteriorProfileCatalog::default(),
            &scene,
        )
        .unwrap_err();
        assert!(matches!(err, SceneApplyError::BuildingRestore(_)));
        assert_eq!(world.sorted_building_ids().len(), before_buildings);
    }

    #[test]
    fn scene_v5_serializes_deterministic_task_order() {
        use crate::world::{
            BuildingOwnership, OccupancyCatalogs, place_player_building, sync_construction_tasks,
        };
        use bevy::prelude::Quat;

        let mut world = flat_world();
        let building_catalog = BuildingCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let footprint_catalog = FootprintCatalog::default();
        let occ = OccupancyCatalogs {
            building: &building_catalog,
            doodad: &doodad_catalog,
            footprint: &footprint_catalog,
        };
        let _ = place_player_building(
            &building_catalog,
            &mut world,
            &crate::world::BuildingDefinitionId::new("hut"),
            pos(48.0, 48.0),
            Quat::IDENTITY,
            BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
            occ,
        );
        sync_construction_tasks(&mut world, &building_catalog, 0);
        let scene_a = sample_scene(&world);
        let scene_b = sample_scene(&world);
        assert_eq!(scene_a.version, super::super::snapshot::SCENE_VERSION);
        assert_eq!(scene_a.task_records, scene_b.task_records);
        assert!(scene_a.next_task_id >= 1);
    }

    #[test]
    fn scene_v7_inventory_persistence_roundtrip() {
        use crate::world::{
            InventoryCatalogCtx, InventoryOwnerRef, InventoryProfileCatalog, InventoryRecord,
            ItemCatalog, ItemCategoryCatalog, physical_gold_item_id, place_stack_first_fit,
            starter_inventory_profile_definitions, starter_item_category_definitions,
            starter_item_definitions, validate_world_inventory_state,
        };

        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);

        let mut world = flat_world();
        let inventory_id = world.inventory_store_mut().allocate_inventory_id();
        let record = InventoryRecord::new(
            inventory_id,
            InventoryOwnerRef::Detached,
            crate::world::InventoryProfileId::new("unit_backpack_standard"),
            8,
            8,
        );
        world.inventory_store_mut().insert(record).unwrap();
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            &ctx,
            inventory_id,
            physical_gold_item_id(),
            99,
        )
        .unwrap();

        let scene = sample_scene(&world);
        assert_eq!(scene.version, super::super::snapshot::SCENE_VERSION);
        assert_eq!(scene.version, 8);
        assert_eq!(scene.inventory_persistence.inventory_records.len(), 1);
        assert!(
            scene
                .inventory_persistence
                .inventory_records
                .iter()
                .any(|record| record
                    .entries
                    .iter()
                    .any(|entry| entry.quantity == Some(99))),
            "expected gold stack in scene persistence"
        );

        let before_mass = world
            .inventory_store()
            .get(inventory_id)
            .unwrap()
            .total_mass_grams();

        let mut restored = flat_world();
        super::super::inventory_snapshot::restore_inventory_persistence(
            &mut restored,
            &scene.inventory_persistence,
            &ctx,
        )
        .unwrap();

        let after_mass = restored
            .inventory_store()
            .get(inventory_id)
            .unwrap()
            .total_mass_grams();
        assert_eq!(before_mass, after_mass);

        let report = validate_world_inventory_state(&restored, &ctx);
        assert!(report.is_ok(), "{report:?}");
    }
}
