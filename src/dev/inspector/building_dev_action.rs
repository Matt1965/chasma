//! Building dev actions — authoritative paths for Selected Object UI (Slice 12).

use bevy::prelude::*;

use crate::client::selection::WorldSelectionCategory;
use crate::simulation::{BuildingSimulationParams, SimulationControlState};
use crate::ui::gameplay::primary_selected_unit;
use crate::units::input::SelectedUnits;
use crate::world::{
    AssessmentRebuildOutcome, BuildingInventoryContext, BuildingLifecycleState, ItemDefinitionId,
    LogisticsRouteTrigger, OccupancyCatalogs, PRODUCTION_PROGRESS_ONE_UNIT, ProductionProgress,
    TerrainAssessmentCatalogs, TransferPlacementPolicy, add_building_construction_progress,
    cycle_production_selected_operation, damage_building, destroy_building,
    execute_production_cycle, heal_building, place_stack_first_fit,
    rebuild_building_terrain_assessment, remove_entry, reset_production_progress,
    set_building_container_locked, set_building_lifecycle_stage, set_production_enabled,
    set_production_paused, transfer_one, validate_building_inventory_links,
    validate_production_runtime_with_catalogs,
};

use super::capture::{capture_building_inspector_snapshot, probe_building_operation};
use super::params::DevBuildingActionParams;
use super::state::WorldInspectorState;

/// Dev-only building actions exposed in Selected Object (Slice 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildingDevAction {
    Damage50,
    Heal50,
    Destroy,
    SetRuins,
    Complete,
    AddConstructionProgress,
    OpenDoor,
    LockDoor,
    LogInventory,
    AddGold,
    TransferWithUnit,
    ToggleContainerLock,
    ValidateInventoryLinks,
    ToggleProductionEnabled,
    ToggleProductionPaused,
    ResetProductionProgress,
    ToggleProductionAdvanced,
    ValidateProduction,
    CycleOperationForward,
    CycleOperationBackward,
    ForceProductionCycle,
    ClearBindingInventories,
    RebuildTerrainAssessment,
    ForceSettlementReplan,
    SpawnManualHaul,
    CancelOpenHauls,
    ForceCompleteHaul,
}

impl BuildingDevAction {
    pub const CONSTRUCTION: &[Self] = &[
        Self::Complete,
        Self::AddConstructionProgress,
        Self::SetRuins,
    ];
    pub const LIFECYCLE: &[Self] = &[Self::Damage50, Self::Heal50, Self::Destroy];
    pub const PRODUCTION: &[Self] = &[
        Self::ToggleProductionEnabled,
        Self::ToggleProductionPaused,
        Self::ResetProductionProgress,
        Self::CycleOperationForward,
        Self::CycleOperationBackward,
        Self::ForceProductionCycle,
        Self::ToggleProductionAdvanced,
        Self::ValidateProduction,
    ];
    pub const INVENTORY: &[Self] = &[
        Self::LogInventory,
        Self::AddGold,
        Self::TransferWithUnit,
        Self::ToggleContainerLock,
        Self::ValidateInventoryLinks,
        Self::ClearBindingInventories,
    ];
    pub const LOGISTICS: &[Self] = &[
        Self::SpawnManualHaul,
        Self::CancelOpenHauls,
        Self::ForceCompleteHaul,
        Self::ForceSettlementReplan,
    ];
    pub const DOORS: &[Self] = &[Self::OpenDoor, Self::LockDoor];
    pub const TERRAIN: &[Self] = &[Self::RebuildTerrainAssessment];

    pub fn label(self) -> &'static str {
        match self {
            Self::Damage50 => "Damage +50",
            Self::Heal50 => "Heal +50",
            Self::Destroy => "Destroy (dev)",
            Self::SetRuins => "Set ruins",
            Self::Complete => "Complete",
            Self::AddConstructionProgress => "+10% progress",
            Self::OpenDoor => "Open door",
            Self::LockDoor => "Lock door",
            Self::LogInventory => "Log inventory",
            Self::AddGold => "Add 5 gold",
            Self::TransferWithUnit => "Transfer w/ unit",
            Self::ToggleContainerLock => "Toggle lock",
            Self::ValidateInventoryLinks => "Validate links",
            Self::ToggleProductionEnabled => "Toggle production",
            Self::ToggleProductionPaused => "Toggle pause",
            Self::ResetProductionProgress => "Reset progress",
            Self::ToggleProductionAdvanced => "Adv. panel",
            Self::ValidateProduction => "Validate production",
            Self::CycleOperationForward => "Next operation",
            Self::CycleOperationBackward => "Prev operation",
            Self::ForceProductionCycle => "Force cycle",
            Self::ClearBindingInventories => "Clear bindings",
            Self::RebuildTerrainAssessment => "Rebuild terrain",
            Self::ForceSettlementReplan => "Force replan",
            Self::SpawnManualHaul => "Spawn haul",
            Self::CancelOpenHauls => "Cancel hauls",
            Self::ForceCompleteHaul => "Force complete haul",
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Damage50 => {
                "Apply 50 damage through the building damage API. Dev-only; affects runtime HP."
            }
            Self::Heal50 => "Heal 50 HP through the building heal API. Dev-only.",
            Self::Destroy => {
                "Destroy the building through the authoritative destroy path (inventory cleanup). \
                 Dev-only — not the same as gameplay demolition."
            }
            Self::SetRuins => "Set lifecycle to Ruins via domain API.",
            Self::Complete => "Set lifecycle to Complete (skip remaining construction).",
            Self::AddConstructionProgress => "Add 10% construction progress.",
            Self::OpenDoor => "Open the first door registered to this building.",
            Self::LockDoor => "Lock the first door registered to this building.",
            Self::LogInventory => "Print building inventory summary to the status line.",
            Self::AddGold => "Place 5 gold into the building inventory (first fit).",
            Self::TransferWithUnit => {
                "Transfer one item between the primary selected unit and building inventories."
            }
            Self::ToggleContainerLock => "Toggle building container lock flag.",
            Self::ValidateInventoryLinks => "Validate all building inventory binding links.",
            Self::ToggleProductionEnabled => "Toggle production enabled policy.",
            Self::ToggleProductionPaused => "Toggle production paused policy.",
            Self::ResetProductionProgress => "Reset in-progress production progress to zero.",
            Self::ToggleProductionAdvanced => "Expand/collapse production advanced diagnostics.",
            Self::ValidateProduction => "Run production runtime validation against catalogs.",
            Self::CycleOperationForward => "Cycle selected production operation forward.",
            Self::CycleOperationBackward => "Cycle selected production operation backward.",
            Self::ForceProductionCycle => {
                "Force-execute one production cycle (dev bypass). May fail if inputs missing."
            }
            Self::ClearBindingInventories => {
                "Clear all binding inventories for this building. Destructive — removes items."
            }
            Self::RebuildTerrainAssessment => "Rebuild terrain field assessment for this building.",
            Self::ForceSettlementReplan => {
                "Force settlement production replan for the building's settlement."
            }
            Self::SpawnManualHaul => {
                "Spawn a manual hauling request from the first logistics route."
            }
            Self::CancelOpenHauls => "Cancel all open hauling requests for this building.",
            Self::ForceCompleteHaul => "Force-complete the first open hauling request.",
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DevBuildingActionButton {
    pub action: BuildingDevAction,
}

/// Apply one dev building action. Returns true when inspector snapshot should refresh.
pub fn apply_building_dev_action(
    action: BuildingDevAction,
    building_id: crate::world::BuildingId,
    world: &mut crate::world::WorldData,
    params: &DevBuildingActionParams,
    building_sim: &mut BuildingSimulationParams,
    simulation: &SimulationControlState,
    selected_units: &SelectedUnits,
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
        BuildingDevAction::Destroy => {
            let _ = destroy_building(
                world,
                &params.building_catalog,
                &params.doodad_catalog,
                occ,
                building_id,
                "dev_destroy",
                Some(&inventory_cleanup),
            );
            inspector.last_message = format!("Destroyed building #{}", building_id.raw());
            true
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
            );
            inspector.last_message = format!("Set building #{} to ruins", building_id.raw());
            true
        }
        BuildingDevAction::Complete => {
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
            );
            inspector.last_message = format!("Completed building #{}", building_id.raw());
            true
        }
        BuildingDevAction::AddConstructionProgress => {
            let _ = add_building_construction_progress(
                world,
                &params.building_catalog,
                &params.interior_catalog,
                &params.doodad_catalog,
                occ,
                Some(&params.nav_blueprint_catalog),
                building_id,
                0.1,
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
        BuildingDevAction::LogInventory => {
            if let Some(record) = world.get_building(building_id) {
                if let Some(inventory_id) = record.inventory_id {
                    let entries = world
                        .inventory_store()
                        .get(inventory_id)
                        .map(|inv| inv.placed_entries().len())
                        .unwrap_or(0);
                    inspector.last_message = format!(
                        "Building #{:?} inventory {inventory_id:?}: {entries} entries, locked={}",
                        building_id, record.container_locked
                    );
                } else {
                    inspector.last_message =
                        format!("Building #{:?} has no inventory", building_id);
                }
                true
            } else {
                false
            }
        }
        BuildingDevAction::AddGold => {
            if let Some(inventory_id) = world.get_building(building_id).and_then(|r| r.inventory_id)
            {
                let (inventory_store, instance_store) = world.inventory_runtime_mut();
                match place_stack_first_fit(
                    inventory_store,
                    instance_store,
                    &inventory_ctx,
                    inventory_id,
                    ItemDefinitionId::new("gold"),
                    5,
                ) {
                    Ok(_) => {
                        inspector.last_message =
                            format!("Added 5 gold to building #{:?} inventory", building_id);
                        true
                    }
                    Err(error) => {
                        inspector.last_message = format!("Add gold failed: {error}");
                        false
                    }
                }
            } else {
                inspector.last_message = "Building has no inventory".into();
                false
            }
        }
        BuildingDevAction::TransferWithUnit => {
            if let (Some(unit_id), Some(building_inventory)) = (
                primary_selected_unit(selected_units),
                world.get_building(building_id).and_then(|r| r.inventory_id),
            ) {
                let unit_inventory = world.get_unit(unit_id).and_then(|u| u.inventory_id);
                if let (Some(from), Some(to)) = (unit_inventory, Some(building_inventory)) {
                    let (inventory_store, instance_store) = world.inventory_runtime_mut();
                    match transfer_one(
                        inventory_store,
                        instance_store,
                        &inventory_ctx,
                        from,
                        0,
                        to,
                        TransferPlacementPolicy::MergeThenFirstFit,
                    ) {
                        Ok(report) => {
                            inspector.last_message =
                                format!("Transferred to building: {:?}", report.status);
                            true
                        }
                        Err(error) => {
                            inspector.last_message = format!("Transfer failed: {error}");
                            false
                        }
                    }
                } else if let (Some(from), Some(to)) = (Some(building_inventory), unit_inventory) {
                    let (inventory_store, instance_store) = world.inventory_runtime_mut();
                    match transfer_one(
                        inventory_store,
                        instance_store,
                        &inventory_ctx,
                        from,
                        0,
                        to,
                        TransferPlacementPolicy::MergeThenFirstFit,
                    ) {
                        Ok(report) => {
                            inspector.last_message =
                                format!("Transferred from building: {:?}", report.status);
                            true
                        }
                        Err(error) => {
                            inspector.last_message = format!("Transfer failed: {error}");
                            false
                        }
                    }
                } else {
                    inspector.last_message =
                        "Select unit with inventory for unit↔building transfer".into();
                    false
                }
            } else {
                inspector.last_message =
                    "Select unit and building with inventories for transfer".into();
                false
            }
        }
        BuildingDevAction::ToggleContainerLock => {
            if let Some(record) = world.get_building(building_id) {
                if record.inventory_id.is_some() {
                    let locked = !record.container_locked;
                    if set_building_container_locked(world, building_id, locked).is_ok() {
                        inspector.last_message = format!(
                            "Building #{:?} container {}",
                            building_id,
                            if locked { "locked" } else { "unlocked" }
                        );
                        true
                    } else {
                        false
                    }
                } else {
                    inspector.last_message = "Building has no inventory".into();
                    false
                }
            } else {
                false
            }
        }
        BuildingDevAction::ValidateInventoryLinks => {
            let errors = validate_building_inventory_links(world);
            inspector.last_message = if errors.is_empty() {
                "Building inventory links OK".to_string()
            } else {
                format!("Building inventory errors: {errors:?}")
            };
            true
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
        BuildingDevAction::ToggleProductionPaused => {
            let paused = world
                .building_production_store()
                .get_policy(building_id)
                .map(|policy| !policy.paused)
                .unwrap_or(true);
            match set_production_paused(world, building_id, paused) {
                Ok(()) => {
                    inspector.last_message = format!(
                        "Production {} for building #{}",
                        if paused { "paused" } else { "resumed" },
                        building_id.raw()
                    );
                    true
                }
                Err(error) => {
                    inspector.last_message = format!("Production pause failed: {error}");
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
        BuildingDevAction::ToggleProductionAdvanced => {
            inspector.production_advanced_expanded = !inspector.production_advanced_expanded;
            inspector.last_message = if inspector.production_advanced_expanded {
                "Production advanced panel expanded".to_string()
            } else {
                "Production advanced panel collapsed".to_string()
            };
            true
        }
        BuildingDevAction::ValidateProduction => {
            let issues = validate_production_runtime_with_catalogs(
                world,
                Some(&params.building_catalog),
                Some(&building_sim.operation_catalog),
            );
            inspector.last_message = if issues.is_empty() {
                "Production runtime validation OK".to_string()
            } else {
                format!(
                    "Production validation: {}",
                    issues
                        .iter()
                        .map(|issue| issue.message())
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            };
            true
        }
        BuildingDevAction::CycleOperationForward => {
            match cycle_production_selected_operation(
                world,
                &params.building_catalog,
                &building_sim.operation_catalog,
                building_id,
                true,
            ) {
                Ok(Some(operation)) => {
                    inspector.last_message = format!(
                        "Selected operation {} for building #{}",
                        operation,
                        building_id.raw()
                    );
                    true
                }
                Ok(None) => {
                    inspector.last_message = "Building has no supported operations".into();
                    false
                }
                Err(error) => {
                    inspector.last_message = format!("Operation select failed: {error}");
                    false
                }
            }
        }
        BuildingDevAction::CycleOperationBackward => {
            match cycle_production_selected_operation(
                world,
                &params.building_catalog,
                &building_sim.operation_catalog,
                building_id,
                false,
            ) {
                Ok(Some(operation)) => {
                    inspector.last_message = format!(
                        "Selected operation {} for building #{}",
                        operation,
                        building_id.raw()
                    );
                    true
                }
                Ok(None) => {
                    inspector.last_message = "Building has no supported operations".into();
                    false
                }
                Err(error) => {
                    inspector.last_message = format!("Operation select failed: {error}");
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
        BuildingDevAction::ClearBindingInventories => {
            if let Some(set) = world
                .building_inventory_binding_store()
                .get(building_id)
                .cloned()
            {
                let (inventory_store, instance_store) = world.inventory_runtime_mut();
                for binding in set.bindings() {
                    while let Some(record) = inventory_store.get(binding.inventory_id) {
                        if record.placed_entries().is_empty() {
                            break;
                        }
                        let _ = remove_entry(
                            inventory_store,
                            instance_store,
                            &inventory_ctx,
                            binding.inventory_id,
                            record.placed_entries().len() - 1,
                        );
                    }
                }
                inspector.last_message = format!(
                    "Cleared binding inventories for building #{}",
                    building_id.raw()
                );
                true
            } else {
                inspector.last_message = "No binding inventories".into();
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
        BuildingDevAction::ForceSettlementReplan => {
            if let Some(settlement_id) = world
                .settlement_store()
                .settlement_for_building(building_id)
            {
                let mut planner = world
                    .production_planner_store()
                    .get(settlement_id)
                    .cloned()
                    .unwrap_or_default();
                planner.mark_dirty();
                crate::world::execute_settlement_replan(
                    world,
                    &params.building_catalog,
                    &building_sim.operation_catalog,
                    &inventory_ctx,
                    settlement_id,
                    &mut planner,
                    simulation.current_tick,
                );
                let stored = world.production_planner_store_mut().get_mut(settlement_id);
                stored.last_diagnostics = planner.last_diagnostics;
                stored.last_plan_tick = planner.last_plan_tick;
                stored.dirty = planner.dirty;
                inspector.last_message =
                    format!("Force replanned settlement #{}", settlement_id.raw());
                true
            } else {
                inspector.last_message = "Building is not linked to a settlement".into();
                false
            }
        }
        BuildingDevAction::SpawnManualHaul => {
            if let Some(definition) = world
                .get_building(building_id)
                .and_then(|record| params.building_catalog.get(&record.definition_id))
            {
                if let Some(route) = definition.logistics_routes.first() {
                    let local = world
                        .building_inventory_binding_store()
                        .resolve_inventory(building_id, &route.local_binding_id);
                    let remote = world
                        .logistics_endpoint_index()
                        .resolve(
                            &route.remote_building_definition_id,
                            &route.remote_binding_id,
                        )
                        .and_then(|candidates| candidates.first().copied())
                        .and_then(|remote_building| {
                            world
                                .building_inventory_binding_store()
                                .resolve_inventory(remote_building, &route.remote_binding_id)
                        });
                    if let (Some(local_inventory), Some(remote_inventory)) = (local, remote) {
                        let (source, destination) = match route.trigger {
                            LogisticsRouteTrigger::OutputSurplus => {
                                (local_inventory, remote_inventory)
                            }
                            LogisticsRouteTrigger::InputDeficit => {
                                (remote_inventory, local_inventory)
                            }
                        };
                        if let Some(request_id) = crate::world::spawn_manual_hauling_request(
                            world,
                            route.priority,
                            route.item_id.clone(),
                            1,
                            source,
                            destination,
                            building_id,
                            simulation.current_tick,
                            &inventory_ctx,
                        ) {
                            inspector.last_message =
                                format!("Spawned manual haul request #{request_id}");
                            true
                        } else {
                            inspector.last_message = "Failed to spawn hauling request".into();
                            false
                        }
                    } else {
                        inspector.last_message =
                            "Could not resolve logistics route inventories".into();
                        false
                    }
                } else {
                    inspector.last_message = "Building has no logistics routes".into();
                    false
                }
            } else {
                false
            }
        }
        BuildingDevAction::CancelOpenHauls => {
            let cancelled: Vec<_> = world
                .hauling_request_store()
                .requests_for_building(building_id)
                .iter()
                .copied()
                .filter(|request_id| {
                    world
                        .hauling_request_store()
                        .get(*request_id)
                        .is_some_and(|request| request.status.is_open())
                })
                .collect();
            for request_id in cancelled {
                crate::world::cancel_hauling_request(world, request_id);
            }
            inspector.last_message = format!(
                "Cancelled open hauling requests for building #{}",
                building_id.raw()
            );
            true
        }
        BuildingDevAction::ForceCompleteHaul => {
            if let Some(request_id) = world
                .hauling_request_store()
                .requests_for_building(building_id)
                .first()
                .copied()
            {
                match crate::world::force_complete_hauling_request(
                    world,
                    request_id,
                    &inventory_ctx,
                ) {
                    Ok(moved) => {
                        inspector.last_message =
                            format!("Force-completed haul #{}, moved {moved}", request_id.raw());
                        true
                    }
                    Err(reason) => {
                        inspector.last_message =
                            format!("Force-complete failed: {}", reason.label());
                        false
                    }
                }
            } else {
                inspector.last_message = "No hauling requests to complete".into();
                false
            }
        }
    }
}

pub fn handle_building_dev_action_buttons(
    dev_state: Res<crate::dev::DevModeState>,
    world_selection: Res<crate::client::selection::WorldSelectionState>,
    simulation: Res<SimulationControlState>,
    inspection: Res<super::BlueprintInspectionState>,
    selected_units: Res<SelectedUnits>,
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
            &selected_units,
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
