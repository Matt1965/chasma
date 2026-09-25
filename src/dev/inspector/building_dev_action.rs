//! Building dev actions — authoritative paths for Selected Object UI (Slice 12).

use bevy::prelude::*;

use crate::client::selection::WorldSelectionCategory;
use crate::simulation::{BuildingSimulationParams, SimulationControlState};
use crate::world::{
    AssessmentRebuildOutcome, BuildingInventoryContext, BuildingLifecycleState, OccupancyCatalogs,
    PRODUCTION_PROGRESS_ONE_UNIT, ProductionProgress, TerrainAssessmentCatalogs,
    add_building_construction_progress, damage_building, execute_production_cycle, heal_building,
    rebuild_building_terrain_assessment, reset_production_progress, set_building_lifecycle_stage,
    set_production_enabled,
};

use super::capture::{capture_building_inspector_snapshot, probe_building_operation};
use super::params::DevBuildingActionParams;
use super::state::WorldInspectorState;

/// Dev-only building actions exposed in Selected Object (Slice 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildingDevAction {
    Damage50,
    Heal50,
    SetRuins,
    Complete,
    AddConstructionProgress,
    OpenDoor,
    LockDoor,
    ToggleProductionEnabled,
    ResetProductionProgress,
    ForceProductionCycle,
    RebuildTerrainAssessment,
}

impl BuildingDevAction {
    pub const CONSTRUCTION: &[Self] = &[
        Self::Complete,
        Self::AddConstructionProgress,
        Self::SetRuins,
    ];
    pub const LIFECYCLE: &[Self] = &[Self::Damage50, Self::Heal50, Self::SetRuins];
    pub const PRODUCTION_ACTIONS: &[Self] = &[
        Self::ToggleProductionEnabled,
        Self::ResetProductionProgress,
        Self::ForceProductionCycle,
    ];
    pub const DOORS: &[Self] = &[Self::OpenDoor, Self::LockDoor];
    pub const TERRAIN: &[Self] = &[Self::RebuildTerrainAssessment];

    pub fn label(self) -> &'static str {
        match self {
            Self::Damage50 => "Damage +50",
            Self::Heal50 => "Heal +50",
            Self::SetRuins => "Set ruins",
            Self::Complete => "Complete",
            Self::AddConstructionProgress => "+10% progress",
            Self::OpenDoor => "Open door",
            Self::LockDoor => "Lock door",
            Self::ToggleProductionEnabled => "Enable / disable production",
            Self::ResetProductionProgress => "Reset progress",
            Self::ForceProductionCycle => "Force cycle",
            Self::RebuildTerrainAssessment => "Rebuild terrain",
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Damage50 => {
                "Apply 50 damage through the building damage API. Dev-only; affects runtime HP."
            }
            Self::Heal50 => "Heal 50 HP through the building heal API. Dev-only.",
            Self::SetRuins => "Set lifecycle to Ruins via domain API.",
            Self::Complete => "Set lifecycle to Complete (skip remaining construction).",
            Self::AddConstructionProgress => "Add 10% construction progress.",
            Self::OpenDoor => "Open the first door registered to this building.",
            Self::LockDoor => "Lock the first door registered to this building.",
            Self::ToggleProductionEnabled => "Toggle production enabled policy.",
            Self::ResetProductionProgress => "Reset in-progress production progress to zero.",
            Self::ForceProductionCycle => {
                "Force-execute one production cycle (dev bypass). May fail if inputs missing."
            }
            Self::RebuildTerrainAssessment => "Rebuild terrain field assessment for this building.",
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DevBuildingActionButton {
    pub action: BuildingDevAction,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DevProductionOperationButton {
    pub operation_index: usize,
}

/// Apply one dev building action. Returns true when inspector snapshot should refresh.
pub fn apply_building_dev_action(
    action: BuildingDevAction,
    building_id: crate::world::BuildingId,
    world: &mut crate::world::WorldData,
    params: &DevBuildingActionParams,
    building_sim: &mut BuildingSimulationParams,
    simulation: &SimulationControlState,
    inspection_active: bool,
    inspector: &mut WorldInspectorState,
) -> bool {
    let occ = OccupancyCatalogs {
        doodad: &params.doodad_catalog,
        building: &params.building_catalog,
        footprint: &params.footprint_catalog,
    };
    let inventory_ctx = params.inventory_ctx();
    let inventory_cleanup = BuildingInventoryContext {
        ctx: &inventory_ctx,
        pile_settings: &params.pile_settings,
        interaction_catalog: &params.interaction_catalog,
        tick: simulation.current_tick,
    };

    match action {
        BuildingDevAction::Damage50 => {
            let _ = damage_building(
                world,
                &params.building_catalog,
                &params.doodad_catalog,
                occ,
                building_id,
                50,
                "dev_damage",
                Some(&inventory_cleanup),
            );
            inspector.last_message = format!("Damaged building #{}", building_id.raw());
            true
        }
        BuildingDevAction::Heal50 => {
            if heal_building(world, building_id, 50).is_ok() {
                inspector.last_message = format!("Healed building #{}", building_id.raw());
                true
            } else {
                false
            }
        }
        BuildingDevAction::SetRuins => {
            let _ = set_building_lifecycle_stage(
                world,
                &params.building_catalog,
                &params.interior_catalog,
                &params.doodad_catalog,
                occ,
                None,
                building_id,
                BuildingLifecycleState::Ruins,
                1.0,
                None,
            );
            inspector.last_message = format!("Set building #{} to ruins", building_id.raw());
            true
        }
        BuildingDevAction::Complete => {
            let inventory_ctx = params.inventory_ctx();
            let _ = set_building_lifecycle_stage(
                world,
                &params.building_catalog,
                &params.interior_catalog,
                &params.doodad_catalog,
                occ,
                Some(&params.nav_blueprint_catalog),
                building_id,
                BuildingLifecycleState::Complete,
                1.0,
                Some(&inventory_ctx),
            );
            inspector.last_message = format!("Completed building #{}", building_id.raw());
            true
        }
        BuildingDevAction::AddConstructionProgress => {
            let inventory_ctx = params.inventory_ctx();
            let _ = add_building_construction_progress(
                world,
                &params.building_catalog,
                &params.interior_catalog,
                &params.doodad_catalog,
                occ,
                Some(&params.nav_blueprint_catalog),
                building_id,
                0.1,
                Some(&inventory_ctx),
            );
            inspector.last_message =
                format!("Added 10% progress to building #{}", building_id.raw());
            true
        }
        BuildingDevAction::OpenDoor => {
            if let Some(door_id) = first_building_door(world, building_id) {
                let _ = crate::world::open_door(world, door_id);
                inspector.last_message = format!("Opened door #{}", door_id.raw());
                true
            } else {
                inspector.last_message = "Building has no doors".into();
                false
            }
        }
        BuildingDevAction::LockDoor => {
            if let Some(door_id) = first_building_door(world, building_id) {
                let _ = crate::world::lock_door(world, door_id);
                inspector.last_message = format!("Locked door #{}", door_id.raw());
                true
            } else {
                inspector.last_message = "Building has no doors".into();
                false
            }
        }
        BuildingDevAction::ToggleProductionEnabled => {
            let enabled = world
                .building_production_store()
                .get_policy(building_id)
                .map(|policy| !policy.enabled)
                .unwrap_or(true);
            match set_production_enabled(world, building_id, enabled) {
                Ok(()) => {
                    inspector.last_message = format!(
                        "Production {} for building #{}",
                        if enabled { "enabled" } else { "disabled" },
                        building_id.raw()
                    );
                    true
                }
                Err(error) => {
                    inspector.last_message = format!("Production enable failed: {error}");
                    false
                }
            }
        }
        BuildingDevAction::ResetProductionProgress => {
            if inspection_active {
                inspector.last_message =
                    "Close blueprint inspection before resetting production".into();
                return false;
            }
            match reset_production_progress(world, building_id) {
                Ok(()) => {
                    inspector.last_message = format!(
                        "Reset production progress for building #{}",
                        building_id.raw()
                    );
                    true
                }
                Err(error) => {
                    inspector.last_message = format!("Production reset failed: {error}");
                    false
                }
            }
        }
        BuildingDevAction::ForceProductionCycle => {
            if let Some(record) = world.get_building(building_id) {
                if let Some(definition) = params.building_catalog.get(&record.definition_id) {
                    world
                        .building_production_store_mut()
                        .ensure_policy_for_building(
                            building_id,
                            definition,
                            &building_sim.operation_catalog,
                        );
                    if let Some(selected) = world
                        .building_production_store()
                        .get_policy(building_id)
                        .and_then(|policy| policy.selected_operation.clone())
                    {
                        if let Some(op_def) = building_sim.operation_catalog.get(&selected) {
                            world
                                .building_production_store_mut()
                                .get_state_mut(building_id)
                                .progress = ProductionProgress(PRODUCTION_PROGRESS_ONE_UNIT);
                            match execute_production_cycle(
                                world,
                                &inventory_ctx,
                                building_id,
                                op_def,
                                definition,
                            ) {
                                Ok(()) => {
                                    let state = world
                                        .building_production_store_mut()
                                        .get_state_mut(building_id);
                                    state.progress.completions_since(
                                        crate::world::PRODUCTION_PROGRESS_ONE_UNIT,
                                    );
                                    state.completion_count =
                                        state.completion_count.saturating_add(1);
                                    inspector.last_message = format!(
                                        "Force-executed production cycle for building #{}",
                                        building_id.raw()
                                    );
                                    true
                                }
                                Err(factor) => {
                                    inspector.last_message =
                                        format!("Force execute blocked: {}", factor.label());
                                    false
                                }
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        }
        BuildingDevAction::RebuildTerrainAssessment => {
            let catalogs = TerrainAssessmentCatalogs {
                buildings: &params.building_catalog,
                requirements: &building_sim.requirement_catalog,
                profiles: &building_sim.profile_catalog,
                fields: &building_sim.field_catalog,
                footprints: &params.footprint_catalog,
                requirement_revision: building_sim.requirement_revision.0,
                profile_revision: building_sim.profile_revision.0,
            };
            match rebuild_building_terrain_assessment(
                world,
                &catalogs,
                &mut building_sim.assessment_store,
                building_id,
            ) {
                AssessmentRebuildOutcome::Assessed => {
                    inspector.last_message = format!(
                        "Refreshed terrain assessment for building #{}",
                        building_id.raw()
                    );
                    true
                }
                outcome => {
                    inspector.last_message = format!("Terrain assessment refresh: {outcome:?}");
                    false
                }
            }
        }
    }
}

pub fn handle_production_operation_buttons(
    dev_state: Res<crate::dev::DevModeState>,
    world_selection: Res<crate::client::selection::WorldSelectionState>,
    mut building_sim: BuildingSimulationParams,
    mut params: DevBuildingActionParams,
    mut world: ResMut<crate::world::WorldData>,
    mut inspector: ResMut<WorldInspectorState>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    buttons: Query<(&Interaction, &DevProductionOperationButton), Changed<Interaction>>,
) {
    if !dev_state.enabled {
        return;
    }
    let Some(building_id) = (world_selection.category == WorldSelectionCategory::Building)
        .then_some(world_selection.building_id)
        .flatten()
    else {
        return;
    };
    let Some(record) = world.get_building(building_id) else {
        return;
    };
    let Some(definition) = params.building_catalog.get(&record.definition_id) else {
        return;
    };
    let operations = &definition.supported_operations;

    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(operation_id) = operations.get(button.operation_index) else {
            continue;
        };
        gate.block_gameplay_mouse = true;
        match crate::world::set_production_selected_operation(
            &mut world,
            &params.building_catalog,
            &building_sim.operation_catalog,
            building_id,
            Some(operation_id.clone()),
        ) {
            Ok(()) => {
                inspector.last_message = format!(
                    "Selected operation {} for building #{}",
                    operation_id.as_str(),
                    building_id.raw()
                );
                refresh_building_inspector_snapshot(
                    &world,
                    &params,
                    &mut building_sim,
                    building_id,
                    &mut inspector,
                );
            }
            Err(error) => {
                inspector.last_message = format!("Operation select failed: {error}");
            }
        }
    }
}

pub fn handle_building_dev_action_buttons(
    dev_state: Res<crate::dev::DevModeState>,
    world_selection: Res<crate::client::selection::WorldSelectionState>,
    simulation: Res<SimulationControlState>,
    inspection: Res<super::BlueprintInspectionState>,
    mut building_sim: BuildingSimulationParams,
    mut params: DevBuildingActionParams,
    mut world: ResMut<crate::world::WorldData>,
    mut inspector: ResMut<WorldInspectorState>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    buttons: Query<(&Interaction, &DevBuildingActionButton), Changed<Interaction>>,
) {
    if !dev_state.enabled {
        return;
    }
    let Some(building_id) = (world_selection.category == WorldSelectionCategory::Building)
        .then_some(world_selection.building_id)
        .flatten()
    else {
        return;
    };

    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        let refresh = apply_building_dev_action(
            button.action,
            building_id,
            &mut world,
            &params,
            &mut building_sim,
            &simulation,
            inspection.active,
            &mut inspector,
        );
        if refresh {
            refresh_building_inspector_snapshot(
                &world,
                &params,
                &mut building_sim,
                building_id,
                &mut inspector,
            );
        }
    }
}

fn refresh_building_inspector_snapshot(
    world: &crate::world::WorldData,
    params: &DevBuildingActionParams,
    building_sim: &mut BuildingSimulationParams,
    building_id: crate::world::BuildingId,
    inspector: &mut WorldInspectorState,
) {
    let inventory_ctx = params.inventory_ctx();
    let mut operation = building_sim.operation_params(
        &params.building_catalog,
        &params.footprint_catalog,
        &inventory_ctx,
        0,
    );
    let operation_probe =
        probe_building_operation(world, &params.building_catalog, &mut operation, building_id);
    inspector.building_snapshot = capture_building_inspector_snapshot(
        world,
        &params.building_catalog,
        &params.interaction_catalog,
        building_id,
        None,
        Some(operation_probe),
        &building_sim.operation_catalog,
        &params.items,
        &params.inventory_profiles,
        building_sim.assessment_store.get(building_id),
    );
}

fn first_building_door(
    world: &crate::world::WorldData,
    building_id: crate::world::BuildingId,
) -> Option<crate::world::DoorId> {
    world
        .door_store()
        .building_door_ids(building_id)
        .first()
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exposed_action_sections() -> Vec<BuildingDevAction> {
        [
            BuildingDevAction::CONSTRUCTION,
            BuildingDevAction::LIFECYCLE,
            BuildingDevAction::PRODUCTION_ACTIONS,
            BuildingDevAction::DOORS,
            BuildingDevAction::TERRAIN,
        ]
        .into_iter()
        .flatten()
        .copied()
        .collect()
    }

    #[test]
    fn all_variants_are_exposed_in_primary_sections() {
        let exposed = exposed_action_sections();
        for action in [
            BuildingDevAction::Damage50,
            BuildingDevAction::Heal50,
            BuildingDevAction::SetRuins,
            BuildingDevAction::Complete,
            BuildingDevAction::AddConstructionProgress,
            BuildingDevAction::OpenDoor,
            BuildingDevAction::LockDoor,
            BuildingDevAction::ToggleProductionEnabled,
            BuildingDevAction::ResetProductionProgress,
            BuildingDevAction::ForceProductionCycle,
            BuildingDevAction::RebuildTerrainAssessment,
        ] {
            assert!(
                exposed.contains(&action),
                "{action:?} should appear in a primary UI section"
            );
        }
    }

    #[test]
    fn production_section_contains_actions_only() {
        assert_eq!(
            BuildingDevAction::PRODUCTION_ACTIONS,
            &[
                BuildingDevAction::ToggleProductionEnabled,
                BuildingDevAction::ResetProductionProgress,
                BuildingDevAction::ForceProductionCycle,
            ]
        );
    }

    #[test]
    fn lifecycle_section_excludes_destroy() {
        assert_eq!(
            BuildingDevAction::LIFECYCLE,
            &[
                BuildingDevAction::Damage50,
                BuildingDevAction::Heal50,
                BuildingDevAction::SetRuins,
            ]
        );
    }
}
