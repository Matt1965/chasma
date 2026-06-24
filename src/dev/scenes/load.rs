//! Apply dev scenes to authoritative [`WorldData`] (ADR-045).

use std::time::Instant;

use bevy::prelude::*;

use crate::world::{
    DoodadCatalog, DoodadInsertError, DoodadSource, UnitCatalog, UnitInsertError, WorldData,
};

use super::snapshot::{
    parse_doodad_kind, SceneDefinition, SceneDoodadRecord, SceneUnitRecord, SCENE_VERSION,
};

/// Outcome of applying a scene to world data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SceneApplyReport {
    pub units_loaded: u32,
    pub doodads_loaded: u32,
    pub world_seed: u64,
    pub elapsed_ms: u64,
}

/// Why scene application failed (world left unchanged when validation fails first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneApplyError {
    UnsupportedVersion { found: u32, expected: u32 },
    UnitDefinitionNotFound(String),
    UnitDefinitionDisabled(String),
    DoodadDefinitionNotFound(String),
    DoodadDefinitionDisabled(String),
    InvalidUnitRecord { id: u64, reason: String },
    InvalidDoodadRecord { id: u64, reason: String },
    UnitInsert(UnitInsertError),
    DoodadInsert(DoodadInsertError),
}

impl std::fmt::Display for SceneApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion { found, expected } => {
                write!(f, "unsupported scene version {found} (expected {expected})")
            }
            Self::UnitDefinitionNotFound(id) => write!(f, "unit definition not found: {id}"),
            Self::UnitDefinitionDisabled(id) => write!(f, "unit definition disabled: {id}"),
            Self::DoodadDefinitionNotFound(id) => write!(f, "doodad definition not found: {id}"),
            Self::DoodadDefinitionDisabled(id) => write!(f, "doodad definition disabled: {id}"),
            Self::InvalidUnitRecord { id, reason } => {
                write!(f, "invalid unit record {id}: {reason}")
            }
            Self::InvalidDoodadRecord { id, reason } => {
                write!(f, "invalid doodad record {id}: {reason}")
            }
            Self::UnitInsert(err) => write!(f, "unit insert failed: {err:?}"),
            Self::DoodadInsert(err) => write!(f, "doodad insert failed: {err:?}"),
        }
    }
}

/// Clear all unit and doodad instances from world data (dev-only).
pub fn clear_world_entities(world: &mut WorldData) {
    world.dev_clear_units_and_doodads();
}

/// Validate and apply a scene through [`WorldData`] authoring APIs only.
pub fn apply_scene(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    scene: &SceneDefinition,
) -> Result<SceneApplyReport, SceneApplyError> {
    validate_scene(scene, unit_catalog, doodad_catalog)?;

    let started = Instant::now();
    world.dev_clear_units_and_doodads();

    for unit in &scene.unit_records {
        restore_unit(world, unit)?;
    }

    for doodad in &scene.doodad_records {
        restore_doodad(world, doodad_catalog, doodad)?;
    }

    world.dev_restore_id_counters(scene.next_unit_id, scene.next_doodad_id);

    let elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let report = SceneApplyReport {
        units_loaded: scene.unit_records.len() as u32,
        doodads_loaded: scene.doodad_records.len() as u32,
        world_seed: scene.world_seed,
        elapsed_ms,
    };

    info!(
        "dev scene loaded: units={} doodads={} seed={} took={}ms",
        report.units_loaded, report.doodads_loaded, report.world_seed, report.elapsed_ms
    );

    Ok(report)
}

fn validate_scene(
    scene: &SceneDefinition,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
) -> Result<(), SceneApplyError> {
    if scene.version != SCENE_VERSION {
        return Err(SceneApplyError::UnsupportedVersion {
            found: scene.version,
            expected: SCENE_VERSION,
        });
    }

    for unit in &scene.unit_records {
        validate_unit_record(unit, unit_catalog)?;
    }
    for doodad in &scene.doodad_records {
        validate_doodad_record(doodad, doodad_catalog)?;
    }
    Ok(())
}

fn validate_unit_record(
    unit: &SceneUnitRecord,
    catalog: &UnitCatalog,
) -> Result<(), SceneApplyError> {
    let definition_id = crate::world::UnitDefinitionId::new(&unit.definition_id);
    let Some(definition) = catalog.get(&definition_id) else {
        return Err(SceneApplyError::UnitDefinitionNotFound(
            unit.definition_id.clone(),
        ));
    };
    if !definition.enabled {
        return Err(SceneApplyError::UnitDefinitionDisabled(
            unit.definition_id.clone(),
        ));
    }
    unit.to_record()
        .map_err(|err| SceneApplyError::InvalidUnitRecord {
            id: unit.id,
            reason: format!("{err:?}"),
        })?;
    Ok(())
}

fn validate_doodad_record(
    doodad: &SceneDoodadRecord,
    catalog: &DoodadCatalog,
) -> Result<(), SceneApplyError> {
    let definition_id = crate::world::DoodadDefinitionId::new(&doodad.definition_id);
    let Some(definition) = catalog.get(&definition_id) else {
        return Err(SceneApplyError::DoodadDefinitionNotFound(
            doodad.definition_id.clone(),
        ));
    };
    if !definition.enabled {
        return Err(SceneApplyError::DoodadDefinitionDisabled(
            doodad.definition_id.clone(),
        ));
    }
    parse_doodad_kind(&doodad.kind).map_err(|err| SceneApplyError::InvalidDoodadRecord {
        id: doodad.id,
        reason: format!("{err:?}"),
    })?;
    doodad
        .to_record(definition.kind)
        .map_err(|err| SceneApplyError::InvalidDoodadRecord {
            id: doodad.id,
            reason: format!("{err:?}"),
        })?;
    Ok(())
}

fn restore_unit(world: &mut WorldData, unit: &SceneUnitRecord) -> Result<(), SceneApplyError> {
    let record = unit.to_record().map_err(|err| SceneApplyError::InvalidUnitRecord {
        id: unit.id,
        reason: format!("{err:?}"),
    })?;
    let chunk = crate::world::ChunkId::new(record.placement.position.chunk);
    world
        .insert_unit(chunk, record)
        .map_err(SceneApplyError::UnitInsert)
}

fn restore_doodad(
    world: &mut WorldData,
    catalog: &DoodadCatalog,
    doodad: &SceneDoodadRecord,
) -> Result<(), SceneApplyError> {
    let definition_id = crate::world::DoodadDefinitionId::new(&doodad.definition_id);
    let definition = catalog
        .get(&definition_id)
        .expect("validated above");
    let kind = parse_doodad_kind(&doodad.kind).map_err(|err| SceneApplyError::InvalidDoodadRecord {
        id: doodad.id,
        reason: format!("{err:?}"),
    })?;
    let record = doodad.to_record(kind).map_err(|err| SceneApplyError::InvalidDoodadRecord {
        id: doodad.id,
        reason: format!("{err:?}"),
    })?;
    let _ = definition;
    let chunk = crate::world::ChunkId::new(record.placement.position.chunk);
    world
        .insert_doodad(chunk, record.clone())
        .map_err(SceneApplyError::DoodadInsert)?;
    if matches!(record.source, DoodadSource::Procedural { .. }) {
        world.dev_reregister_procedural_doodad(&record);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::scenes::{capture_scene, SceneCaptureContext};
    use crate::world::{
        create_doodad, create_unit, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadDefinitionId,
        DoodadPlacementOverrides, DoodadSource, Heightfield, LocalPosition, UnitDefinitionId,
        UnitSource, WorldPosition,
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
        let mut scene = sample_scene(&world);
        scene.unit_records[0].definition_id = "missing_unit".into();
        let err = apply_scene(&mut world, &unit_catalog, &doodad_catalog, &scene).unwrap_err();
        assert!(matches!(
            err,
            SceneApplyError::UnitDefinitionNotFound(id) if id == "missing_unit"
        ));
        assert_eq!(world.sorted_unit_ids().len(), 1);
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
}
