//! Building construction, vitals, and lifecycle transitions (ADR-082 B5).
//!
//! Operates on [`crate::world::WorldData`] only. Runtime ECS mirrors presentation.

use bevy::prelude::*;

use super::catalog::BuildingCatalog;
use super::id::BuildingId;
use super::record::BuildingRecord;
use super::state::{BuildingLifecycleState, ConstructionState};
use super::vitals::BuildingVitals;
use crate::world::DoodadCatalog;
use crate::world::building::interior::InteriorProfileCatalog;
use crate::world::building::interior::{
    InteriorProfileId, activate_building_interior, deactivate_building_interior,
};
use crate::world::{
    BuildingDefinition, OccupancyCatalogs, OccupancyError, WorldData, update_building_occupancy,
};

/// Temporary B5 construction policy until worker-delivered labor (B8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildingConstructionSettings {
    /// When true, incomplete buildings advance via `build_time_seconds` each sim tick.
    pub auto_timed_progress: bool,
}

impl Default for BuildingConstructionSettings {
    fn default() -> Self {
        Self {
            auto_timed_progress: false,
        }
    }
}

/// Dev/test settings that retain B5 timed auto-progress.
impl BuildingConstructionSettings {
    pub fn dev_auto_timed() -> Self {
        Self {
            auto_timed_progress: true,
        }
    }
}

/// Why a lifecycle mutation failed.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildingLifecycleError {
    BuildingNotFound(BuildingId),
    DefinitionNotFound(super::catalog::BuildingDefinitionId),
    DefinitionDisabled(super::catalog::BuildingDefinitionId),
    InvalidProgress(f32),
    InvalidVitals {
        reason: &'static str,
    },
    Occupancy(OccupancyError),
    Interior(super::interior::InteriorError),
    InvalidTransition {
        from: BuildingLifecycleState,
        to: BuildingLifecycleState,
    },
}

/// Structured lifecycle events (bounded per tick).
#[derive(Debug, Clone, PartialEq)]
pub enum BuildingLifecycleEvent {
    ConstructionStarted {
        building_id: BuildingId,
        definition_id: super::catalog::BuildingDefinitionId,
        owner: super::ownership::BuildingOwnership,
    },
    StageChanged {
        building_id: BuildingId,
        definition_id: super::catalog::BuildingDefinitionId,
        old_state: BuildingLifecycleState,
        new_state: BuildingLifecycleState,
        progress_0_1: f32,
        owner: super::ownership::BuildingOwnership,
        reason: &'static str,
    },
    Completed {
        building_id: BuildingId,
        definition_id: super::catalog::BuildingDefinitionId,
        owner: super::ownership::BuildingOwnership,
    },
    Damaged {
        building_id: BuildingId,
        definition_id: super::catalog::BuildingDefinitionId,
        amount: u32,
        current_hp: u32,
        max_hp: u32,
        source: &'static str,
    },
    Destroyed {
        building_id: BuildingId,
        definition_id: super::catalog::BuildingDefinitionId,
        owner: super::ownership::BuildingOwnership,
        source: &'static str,
    },
    BecameRuins {
        building_id: BuildingId,
        definition_id: super::catalog::BuildingDefinitionId,
        owner: super::ownership::BuildingOwnership,
    },
    OccupancyChanged {
        building_id: BuildingId,
        lifecycle_state: BuildingLifecycleState,
    },
}

/// Outcome of one construction simulation pass.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuildingConstructionReport {
    pub advanced: u32,
    pub completed: u32,
    pub events: Vec<BuildingLifecycleEvent>,
}

/// Only [`BuildingLifecycleState::Complete`] buildings with HP > 0 are operational (ADR-082).
pub fn is_building_operational(record: &BuildingRecord) -> bool {
    record.lifecycle_state == BuildingLifecycleState::Complete && record.vitals.current_hp > 0
}

fn definition_for_record<'a>(
    catalog: &'a BuildingCatalog,
    record: &BuildingRecord,
) -> Result<&'a BuildingDefinition, BuildingLifecycleError> {
    let definition = catalog
        .get(&record.definition_id)
        .ok_or_else(|| BuildingLifecycleError::DefinitionNotFound(record.definition_id.clone()))?;
    if !definition.enabled {
        return Err(BuildingLifecycleError::DefinitionDisabled(
            record.definition_id.clone(),
        ));
    }
    Ok(definition)
}

fn validate_progress(progress: f32) -> Result<f32, BuildingLifecycleError> {
    if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
        return Err(BuildingLifecycleError::InvalidProgress(progress));
    }
    Ok(progress)
}

fn validate_vitals(vitals: &BuildingVitals) -> Result<(), BuildingLifecycleError> {
    if vitals.max_hp == 0 {
        return Err(BuildingLifecycleError::InvalidVitals {
            reason: "max_hp must be > 0",
        });
    }
    if vitals.current_hp > vitals.max_hp {
        return Err(BuildingLifecycleError::InvalidVitals {
            reason: "current_hp exceeds max_hp",
        });
    }
    Ok(())
}

/// Advance all buildings on the fixed simulation tick.
pub fn step_all_building_construction(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &InteriorProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    settings: BuildingConstructionSettings,
    delta_seconds: f32,
) -> BuildingConstructionReport {
    let mut report = BuildingConstructionReport::default();
    if !settings.auto_timed_progress || delta_seconds <= 0.0 {
        return report;
    }

    let ids = world.sorted_building_ids();
    for id in ids {
        let Some(record) = world.get_building(id).cloned() else {
            continue;
        };
        if !record.lifecycle_state.receives_construction_progress() {
            continue;
        }
        let Ok(definition) = definition_for_record(building_catalog, &record) else {
            continue;
        };

        let step_report = advance_one_building_construction(
            world,
            building_catalog,
            interior_catalog,
            doodad_catalog,
            occupancy,
            id,
            definition,
            delta_seconds,
        );
        if let Ok(step) = step_report {
            if step.advanced {
                report.advanced += 1;
            }
            if step.completed {
                report.completed += 1;
            }
            report.events.extend(step.events);
        }
    }
    report
}

struct SingleConstructionStep {
    advanced: bool,
    completed: bool,
    events: Vec<BuildingLifecycleEvent>,
}

fn advance_one_building_construction(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &InteriorProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    id: BuildingId,
    definition: &BuildingDefinition,
    delta_seconds: f32,
) -> Result<SingleConstructionStep, BuildingLifecycleError> {
    let record = world
        .get_building(id)
        .cloned()
        .ok_or(BuildingLifecycleError::BuildingNotFound(id))?;

    let mut events = Vec::new();
    let mut advanced = false;
    let mut completed = false;

    match record.lifecycle_state {
        BuildingLifecycleState::Planned => {
            events.extend(apply_stage_transition(
                world,
                occupancy,
                id,
                BuildingLifecycleState::Foundation,
                record.construction.progress_0_1,
                Some("construction_started"),
            )?);
            events.push(BuildingLifecycleEvent::ConstructionStarted {
                building_id: id,
                definition_id: record.definition_id.clone(),
                owner: record.ownership,
            });
            advanced = true;
        }
        BuildingLifecycleState::Foundation => {
            events.extend(apply_stage_transition(
                world,
                occupancy,
                id,
                BuildingLifecycleState::InProgress,
                0.0,
                Some("foundation_complete"),
            )?);
            advanced = true;
        }
        BuildingLifecycleState::InProgress => {
            let build_time = definition.build_time_seconds.max(0.01);
            let delta_progress = (delta_seconds / build_time).clamp(0.0, 1.0);
            let current = world.get_building(id).expect("building exists");
            let new_progress = (current.construction.progress_0_1 + delta_progress).clamp(0.0, 1.0);
            if (new_progress - current.construction.progress_0_1).abs() > f32::EPSILON {
                world.mutate_building(id, |record| {
                    record.construction.progress_0_1 = new_progress;
                });
                advanced = true;
            }
            if new_progress >= 1.0 {
                events.extend(complete_building(
                    world,
                    occupancy,
                    building_catalog,
                    interior_catalog,
                    doodad_catalog,
                    id,
                )?);
                completed = true;
            }
        }
        _ => {}
    }

    Ok(SingleConstructionStep {
        advanced,
        completed,
        events,
    })
}

fn complete_building(
    world: &mut WorldData,
    occupancy: OccupancyCatalogs<'_>,
    building_catalog: &BuildingCatalog,
    interior_catalog: &InteriorProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    id: BuildingId,
) -> Result<Vec<BuildingLifecycleEvent>, BuildingLifecycleError> {
    let record = world
        .get_building(id)
        .cloned()
        .ok_or(BuildingLifecycleError::BuildingNotFound(id))?;
    let definition = definition_for_record(building_catalog, &record)?;
    let mut events = apply_stage_transition(
        world,
        occupancy,
        id,
        BuildingLifecycleState::Complete,
        1.0,
        Some("construction_complete"),
    )?;
    world.mutate_building(id, |record| {
        record.vitals = BuildingVitals::full(definition.max_hp);
        record.construction.progress_0_1 = 1.0;
    });
    events.push(BuildingLifecycleEvent::Completed {
        building_id: id,
        definition_id: record.definition_id.clone(),
        owner: record.ownership,
    });
    if let Some(profile_id) = definition.interior_profile_id.as_deref() {
        activate_building_interior(
            world,
            building_catalog,
            interior_catalog,
            doodad_catalog,
            occupancy,
            id,
            &InteriorProfileId::new(profile_id),
        )
        .map_err(BuildingLifecycleError::Interior)?;
    }
    Ok(events)
}

/// Apply damage; at zero HP runs destruction → ruins pipeline.
pub fn damage_building(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    id: BuildingId,
    amount: u32,
    source: &'static str,
) -> Result<Vec<BuildingLifecycleEvent>, BuildingLifecycleError> {
    let record = world
        .get_building(id)
        .cloned()
        .ok_or(BuildingLifecycleError::BuildingNotFound(id))?;
    if record.lifecycle_state.is_terminal_damage_state() {
        return Ok(Vec::new());
    }

    let vitals = world
        .mutate_building(id, |record| {
            record.vitals.current_hp = record.vitals.current_hp.saturating_sub(amount);
        })
        .ok_or(BuildingLifecycleError::BuildingNotFound(id))?
        .vitals;

    let mut events = vec![BuildingLifecycleEvent::Damaged {
        building_id: id,
        definition_id: record.definition_id.clone(),
        amount,
        current_hp: vitals.current_hp,
        max_hp: vitals.max_hp,
        source,
    }];

    if vitals.current_hp == 0 {
        events.extend(destroy_building(
            world,
            building_catalog,
            doodad_catalog,
            occupancy,
            id,
            source,
        )?);
    }
    Ok(events)
}

/// Heal without exceeding max HP.
pub fn heal_building(
    world: &mut WorldData,
    id: BuildingId,
    amount: u32,
) -> Result<BuildingVitals, BuildingLifecycleError> {
    let vitals = world
        .mutate_building(id, |record| {
            if record.lifecycle_state.is_terminal_damage_state() {
                return;
            }
            record.vitals.current_hp = record
                .vitals
                .current_hp
                .saturating_add(amount)
                .min(record.vitals.max_hp);
        })
        .ok_or(BuildingLifecycleError::BuildingNotFound(id))?
        .vitals;
    validate_vitals(&vitals)?;
    Ok(vitals)
}

/// Force destruction and immediate transition to ruins.
pub fn destroy_building(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    id: BuildingId,
    source: &'static str,
) -> Result<Vec<BuildingLifecycleEvent>, BuildingLifecycleError> {
    let record = world
        .get_building(id)
        .cloned()
        .ok_or(BuildingLifecycleError::BuildingNotFound(id))?;
    if record.lifecycle_state == BuildingLifecycleState::Ruins {
        return Ok(Vec::new());
    }

    let _ = definition_for_record(building_catalog, &record)?;
    let mut events = Vec::new();

    if record.lifecycle_state != BuildingLifecycleState::Destroyed {
        events.extend(apply_stage_transition(
            world,
            occupancy,
            id,
            BuildingLifecycleState::Destroyed,
            record.construction.progress_0_1,
            Some(source),
        )?);
        world.mutate_building(id, |record| {
            record.vitals.current_hp = 0;
        });
        events.push(BuildingLifecycleEvent::Destroyed {
            building_id: id,
            definition_id: record.definition_id.clone(),
            owner: record.ownership,
            source,
        });
    }

    events.extend(transition_to_ruins(
        world,
        building_catalog,
        doodad_catalog,
        occupancy,
        id,
    )?);
    crate::world::prune_invalid_building_tasks(world);
    Ok(events)
}

/// Move a destroyed building into ruins occupancy policy.
pub fn transition_to_ruins(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    id: BuildingId,
) -> Result<Vec<BuildingLifecycleEvent>, BuildingLifecycleError> {
    let record = world
        .get_building(id)
        .cloned()
        .ok_or(BuildingLifecycleError::BuildingNotFound(id))?;
    if record.lifecycle_state == BuildingLifecycleState::Ruins {
        return Ok(Vec::new());
    }

    let events = apply_stage_transition(
        world,
        occupancy,
        id,
        BuildingLifecycleState::Ruins,
        record.construction.progress_0_1,
        Some("became_ruins"),
    )?;
    let mut out = events;
    out.push(BuildingLifecycleEvent::BecameRuins {
        building_id: id,
        definition_id: record.definition_id.clone(),
        owner: record.ownership,
    });
    deactivate_building_interior(world, doodad_catalog, building_catalog, Some(occupancy), id)
        .map_err(BuildingLifecycleError::Interior)?;
    Ok(out)
}

/// Dev/test helper: set lifecycle stage with occupancy update.
pub fn set_building_lifecycle_stage(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &InteriorProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    id: BuildingId,
    new_state: BuildingLifecycleState,
    progress_0_1: f32,
) -> Result<Vec<BuildingLifecycleEvent>, BuildingLifecycleError> {
    let record = world
        .get_building(id)
        .cloned()
        .ok_or(BuildingLifecycleError::BuildingNotFound(id))?;
    let definition = definition_for_record(building_catalog, &record)?;
    validate_progress(progress_0_1)?;
    let progress = if new_state == BuildingLifecycleState::Complete {
        1.0
    } else {
        progress_0_1
    };

    let events = apply_stage_transition(
        world,
        occupancy,
        id,
        new_state,
        progress,
        Some("dev_set_stage"),
    )?;

    if new_state == BuildingLifecycleState::Complete {
        world.mutate_building(id, |record| {
            record.vitals = BuildingVitals::full(definition.max_hp);
        });
        if let Some(profile_id) = definition.interior_profile_id.as_deref() {
            activate_building_interior(
                world,
                building_catalog,
                interior_catalog,
                doodad_catalog,
                occupancy,
                id,
                &InteriorProfileId::new(profile_id),
            )
            .map_err(BuildingLifecycleError::Interior)?;
        }
    } else if new_state == BuildingLifecycleState::Ruins {
        deactivate_building_interior(world, doodad_catalog, building_catalog, Some(occupancy), id)
            .map_err(BuildingLifecycleError::Interior)?;
    } else if matches!(
        new_state,
        BuildingLifecycleState::Planned
            | BuildingLifecycleState::Foundation
            | BuildingLifecycleState::InProgress
    ) {
        world.mutate_building(id, |record| {
            record.vitals = BuildingVitals::construction_vulnerable(definition.max_hp);
        });
    }

    Ok(events)
}

/// Add normalized construction progress for dev/testing.
pub fn add_building_construction_progress(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interior_catalog: &InteriorProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    id: BuildingId,
    delta_progress: f32,
) -> Result<Vec<BuildingLifecycleEvent>, BuildingLifecycleError> {
    let record = world
        .get_building(id)
        .cloned()
        .ok_or(BuildingLifecycleError::BuildingNotFound(id))?;
    if !record.lifecycle_state.receives_construction_progress()
        && record.lifecycle_state != BuildingLifecycleState::Complete
    {
        return Ok(Vec::new());
    }

    let definition = definition_for_record(building_catalog, &record)?;
    let mut events = Vec::new();

    if record.lifecycle_state == BuildingLifecycleState::Planned {
        events.extend(set_building_lifecycle_stage(
            world,
            building_catalog,
            interior_catalog,
            doodad_catalog,
            occupancy,
            id,
            BuildingLifecycleState::InProgress,
            0.0,
        )?);
    }

    let current = world.get_building(id).expect("building exists");
    let new_progress = (current.construction.progress_0_1 + delta_progress).clamp(0.0, 1.0);
    world.mutate_building(id, |record| {
        record.construction.progress_0_1 = new_progress;
        if record.lifecycle_state == BuildingLifecycleState::Planned
            || record.lifecycle_state == BuildingLifecycleState::Foundation
        {
            record.lifecycle_state = BuildingLifecycleState::InProgress;
        }
    });

    if new_progress >= 1.0 {
        events.extend(complete_building(
            world,
            occupancy,
            building_catalog,
            interior_catalog,
            doodad_catalog,
            id,
        )?);
    }
    Ok(events)
}

fn apply_stage_transition(
    world: &mut WorldData,
    occupancy: OccupancyCatalogs<'_>,
    id: BuildingId,
    new_state: BuildingLifecycleState,
    progress_0_1: f32,
    reason: Option<&'static str>,
) -> Result<Vec<BuildingLifecycleEvent>, BuildingLifecycleError> {
    let before = world
        .get_building(id)
        .cloned()
        .ok_or(BuildingLifecycleError::BuildingNotFound(id))?;
    if before.lifecycle_state == new_state {
        return Ok(Vec::new());
    }

    validate_progress(progress_0_1)?;

    let mut updated = before.clone();
    updated.lifecycle_state = new_state;
    updated.construction.progress_0_1 = progress_0_1;
    validate_vitals(&updated.vitals)?;

    update_building_occupancy(world, occupancy, &updated)
        .map_err(BuildingLifecycleError::Occupancy)?;

    world.mutate_building(id, |record| {
        record.lifecycle_state = new_state;
        record.construction.progress_0_1 = progress_0_1;
    });

    let mut events = vec![BuildingLifecycleEvent::StageChanged {
        building_id: id,
        definition_id: before.definition_id.clone(),
        old_state: before.lifecycle_state,
        new_state,
        progress_0_1,
        owner: before.ownership,
        reason: reason.unwrap_or("stage_transition"),
    }];
    events.push(BuildingLifecycleEvent::OccupancyChanged {
        building_id: id,
        lifecycle_state: new_state,
    });
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCatalog, BuildingOwnership, BuildingSource, ChunkCoord, ChunkLayout, DoodadCatalog,
        FootprintCatalog, InteriorProfileCatalog, LocalPosition, OccupancyCatalogs, WorldPosition,
        place_player_building,
    };

    fn layout_world() -> WorldData {
        WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        })
    }

    fn catalogs() -> (BuildingCatalog, DoodadCatalog, FootprintCatalog) {
        (
            BuildingCatalog::default(),
            DoodadCatalog::default(),
            FootprintCatalog::default(),
        )
    }

    fn occ<'a>(
        building: &'a BuildingCatalog,
        doodad: &'a DoodadCatalog,
        footprint: &'a FootprintCatalog,
    ) -> OccupancyCatalogs<'a> {
        OccupancyCatalogs {
            building,
            doodad,
            footprint,
        }
    }

    fn position(local_x: f32, local_z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(local_x, 0.0, local_z)),
        )
    }

    fn interior_catalog() -> InteriorProfileCatalog {
        InteriorProfileCatalog::default()
    }

    fn place_hut(world: &mut WorldData, catalogs: OccupancyCatalogs<'_>) -> BuildingId {
        place_player_building(
            catalogs.building,
            world,
            &super::super::catalog::BuildingDefinitionId::new("hut"),
            position(64.0, 64.0),
            Quat::IDENTITY,
            BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
            catalogs,
        )
        .unwrap()
        .id
    }

    #[test]
    fn planned_starts_with_zero_progress_and_vulnerable_hp() {
        let (building, doodad, footprint) = catalogs();
        let occ = occ(&building, &doodad, &footprint);
        let mut world = layout_world();
        let id = place_hut(&mut world, occ);
        let record = world.get_building(id).unwrap();
        assert_eq!(record.lifecycle_state, BuildingLifecycleState::Planned);
        assert_eq!(record.construction.progress_0_1, 0.0);
        assert_eq!(record.vitals.current_hp, 25);
        assert!(!is_building_operational(record));
    }

    #[test]
    fn deterministic_progress_over_fixed_ticks() {
        let (building, doodad, footprint) = catalogs();
        let occ = occ(&building, &doodad, &footprint);
        let mut world = layout_world();
        let id = place_hut(&mut world, occ);
        let settings = BuildingConstructionSettings::dev_auto_timed();
        let delta = 1.0 / 60.0;
        let build_time = building
            .get(&super::super::catalog::BuildingDefinitionId::new("hut"))
            .unwrap()
            .build_time_seconds;
        let ticks_needed = (build_time / delta).ceil() as u32 + 2;

        for _ in 0..ticks_needed {
            let _ = step_all_building_construction(
                &mut world,
                &building,
                &interior_catalog(),
                &doodad,
                occ,
                settings,
                delta,
            );
        }
        let record = world.get_building(id).unwrap();
        assert_eq!(record.lifecycle_state, BuildingLifecycleState::Complete);
        assert!(is_building_operational(record));
        assert_eq!(record.construction.progress_0_1, 1.0);
    }

    #[test]
    fn auto_progress_disabled_does_not_advance() {
        let (building, doodad, footprint) = catalogs();
        let occ = occ(&building, &doodad, &footprint);
        let mut world = layout_world();
        let id = place_hut(&mut world, occ);
        let report = step_all_building_construction(
            &mut world,
            &building,
            &interior_catalog(),
            &doodad,
            occ,
            BuildingConstructionSettings {
                auto_timed_progress: false,
            },
            1.0 / 60.0,
        );
        assert_eq!(report.advanced, 0);
        assert_eq!(
            world.get_building(id).unwrap().lifecycle_state,
            BuildingLifecycleState::Planned
        );
    }

    #[test]
    fn stage_transitions_emit_once_per_change() {
        let (building, doodad, footprint) = catalogs();
        let occ = occ(&building, &doodad, &footprint);
        let mut world = layout_world();
        let id = place_hut(&mut world, occ);
        let report = step_all_building_construction(
            &mut world,
            &building,
            &interior_catalog(),
            &doodad,
            occ,
            BuildingConstructionSettings::dev_auto_timed(),
            1.0 / 60.0,
        );
        let stage_changes: Vec<_> = report
            .events
            .iter()
            .filter(|event| matches!(event, BuildingLifecycleEvent::StageChanged { .. }))
            .collect();
        assert_eq!(stage_changes.len(), 1);
        assert_eq!(
            world.get_building(id).unwrap().lifecycle_state,
            BuildingLifecycleState::Foundation
        );
    }

    #[test]
    fn damage_clamps_and_triggers_ruins() {
        let (building, doodad, footprint) = catalogs();
        let occ = occ(&building, &doodad, &footprint);
        let mut world = layout_world();
        let id = place_hut(&mut world, occ);
        let events = damage_building(&mut world, &building, &doodad, occ, id, 999, "test").unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BuildingLifecycleEvent::Destroyed { .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BuildingLifecycleEvent::BecameRuins { .. }))
        );
        let record = world.get_building(id).unwrap();
        assert_eq!(record.lifecycle_state, BuildingLifecycleState::Ruins);
        assert_eq!(record.vitals.current_hp, 0);
        assert!(!is_building_operational(record));
    }

    #[test]
    fn destroyed_building_cannot_progress() {
        let (building, doodad, footprint) = catalogs();
        let occ = occ(&building, &doodad, &footprint);
        let mut world = layout_world();
        let id = place_hut(&mut world, occ);
        let _ = destroy_building(&mut world, &building, &doodad, occ, id, "test").unwrap();
        let report = step_all_building_construction(
            &mut world,
            &building,
            &interior_catalog(),
            &doodad,
            occ,
            BuildingConstructionSettings::default(),
            10.0,
        );
        assert_eq!(report.advanced, 0);
    }

    #[test]
    fn heal_clamps_at_max() {
        let (building, doodad, footprint) = catalogs();
        let occ = occ(&building, &doodad, &footprint);
        let mut world = layout_world();
        let id = place_hut(&mut world, occ);
        let _ = damage_building(&mut world, &building, &doodad, occ, id, 5, "test").unwrap();
        let vitals = heal_building(&mut world, id, 999).unwrap();
        assert_eq!(vitals.current_hp, vitals.max_hp);
    }
}
