//! Apply dev scenes to authoritative [`WorldData`] (ADR-045, REVIEW-A5).

use std::collections::HashSet;
use std::time::Instant;

use bevy::prelude::*;

use crate::world::{
    DoodadCatalog, DoodadRecord, DoodadRestoreError, UnitCatalog, UnitRecord, UnitRestoreError,
    WorldData, restore_doodad_record, restore_unit_record, validate_doodad_for_restore,
    validate_unit_for_restore,
};

use super::snapshot::{
    SCENE_VERSION, SceneDefinition, SceneDoodadRecord, SceneUnitRecord, parse_doodad_kind,
};

/// Outcome of applying a scene to world data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SceneApplyReport {
    pub units_loaded: u32,
    pub doodads_loaded: u32,
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
    next_unit_id: u64,
    next_doodad_id: u64,
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
        world.dev_restore_id_counters(self.next_unit_id, self.next_doodad_id);
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
    scene: &SceneDefinition,
) -> Result<SceneApplyReport, SceneApplyError> {
    let plan = build_restore_plan(unit_catalog, doodad_catalog, scene)?;
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

    world.dev_restore_id_counters(plan.next_unit_id, plan.next_doodad_id);
    world
        .verify_instance_indexes()
        .map_err(SceneApplyError::IndexConsistencyFailure)?;

    let elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let report = SceneApplyReport {
        units_loaded: plan.units.len() as u32,
        doodads_loaded: plan.doodads.len() as u32,
        world_seed: scene.world_seed,
        elapsed_ms,
        transient_state_cleared: true,
    };

    info!(
        "dev scene loaded: units={} doodads={} seed={} took={}ms transient_cleared=true",
        report.units_loaded, report.doodads_loaded, report.world_seed, report.elapsed_ms
    );

    Ok(report)
}

fn build_restore_plan(
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    scene: &SceneDefinition,
) -> Result<RestorePlan, SceneApplyError> {
    if scene.version != SCENE_VERSION {
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

    Ok(RestorePlan {
        units,
        doodads,
        next_unit_id: scene.next_unit_id,
        next_doodad_id: scene.next_doodad_id,
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
    Ok(())
}

fn scene_unit_to_record(unit: &SceneUnitRecord) -> Result<UnitRecord, SceneApplyError> {
    unit.to_record()
        .map_err(|err| SceneApplyError::InvalidUnitRecord {
            unit_id: unit.id,
            reason: format!("{err:?}"),
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

        let report = apply_scene(&mut world, &unit_catalog, &doodad_catalog, &scene).unwrap();
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
        )
        .unwrap();
        let scene = sample_scene(&world);
        apply_scene(&mut world, &unit_catalog, &doodad_catalog, &scene).unwrap();
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
        apply_scene(&mut world, &unit_catalog, &doodad_catalog, &scene).unwrap();
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
        let err = apply_scene(&mut world, &unit_catalog, &doodad_catalog, &scene).unwrap_err();
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
        let err = apply_scene(&mut world, &unit_catalog, &doodad_catalog, &scene).unwrap_err();
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
        });
        let err = apply_scene(&mut world, &unit_catalog, &doodad_catalog, &scene).unwrap_err();
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
        apply_scene(&mut world, &unit_catalog, &doodad_catalog, &scene_a).unwrap();
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
        apply_scene(&mut world, &unit_catalog, &doodad_catalog, &scene).unwrap();
        assert!(world.get_unit(id).is_some());
    }
}
