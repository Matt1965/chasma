//! Dev inspector building lifecycle shortcuts (ADR-082 B5, ADR-091 I5).

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::simulation::{BuildingSimulationParams, SimulationControlState};
use crate::world::{
    add_building_construction_progress, assess_production_execution, AssessmentRebuildOutcome,
    BuildingInventoryContext, BuildingInventoryRemovalPolicy, BuildingLifecycleState,
    damage_building, destroy_building, execute_production_cycle, heal_building, ItemDefinitionId,
    LogisticsRouteTrigger, OccupancyCatalogs, place_stack_first_fit, PRODUCTION_PROGRESS_ONE_UNIT,
    ProductionProgress, rebuild_building_terrain_assessment, remove_entry, reset_production_progress,
    set_building_container_locked, set_building_lifecycle_stage, set_production_enabled,
    set_production_execution_mode, set_production_paused, TerrainAssessmentCatalogs, transfer_one,
    TransferPlacementPolicy, validate_building_inventory_links, validate_production_runtime_with_catalogs,
    RepeatMode, cycle_production_selected_operation,
};

use super::capture::{capture_building_inspector_snapshot, probe_building_operation};
use super::params::DevBuildingActionParams;
use super::state::WorldInspectorState;

pub fn handle_building_dev_actions(
    dev_state: Res<crate::dev::DevModeState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    simulation: Res<SimulationControlState>,
    mut building_sim: BuildingSimulationParams,
    mut params: DevBuildingActionParams,
    mut world: ResMut<crate::world::WorldData>,
    mut inspector: ResMut<WorldInspectorState>,
) {
    if !dev_state.enabled {
        return;
    }
    let Some(building_id) = inspector.selected_building else {
        return;
    };

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

    let mut refresh = false;
    if keyboard.just_pressed(KeyCode::KeyD) {
        let _ = damage_building(
            &mut world,
            &params.building_catalog,
            &params.doodad_catalog,
            occ,
            building_id,
            50,
            "dev_damage",
            Some(&inventory_cleanup),
        );
        inspector.last_message = format!("Damaged building #{}", building_id.raw());
        refresh = true;
    }
    if keyboard.just_pressed(KeyCode::KeyH) {
        if heal_building(&mut world, building_id, 50).is_ok() {
            inspector.last_message = format!("Healed building #{}", building_id.raw());
            refresh = true;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyX) {
        let _ = destroy_building(
            &mut world,
            &params.building_catalog,
            &params.doodad_catalog,
            occ,
            building_id,
            "dev_destroy",
            Some(&inventory_cleanup),
        );
        inspector.last_message = format!("Destroyed building #{}", building_id.raw());
        refresh = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        let _ = set_building_lifecycle_stage(
            &mut world,
            &params.building_catalog,
            &params.interior_catalog,
            &params.doodad_catalog,
            occ,
            building_id,
            BuildingLifecycleState::Ruins,
            1.0,
        );
        inspector.last_message = format!("Set building #{} to ruins", building_id.raw());
        refresh = true;
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        let _ = set_building_lifecycle_stage(
            &mut world,
            &params.building_catalog,
            &params.interior_catalog,
            &params.doodad_catalog,
            occ,
            building_id,
            BuildingLifecycleState::Complete,
            1.0,
        );
        inspector.last_message = format!("Completed building #{}", building_id.raw());
        refresh = true;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        let _ = add_building_construction_progress(
            &mut world,
            &params.building_catalog,
            &params.interior_catalog,
            &params.doodad_catalog,
            occ,
            building_id,
            0.1,
        );
        inspector.last_message = format!("Added 10% progress to building #{}", building_id.raw());
        refresh = true;
    }

    if keyboard.just_pressed(KeyCode::KeyO) {
        if let Some(door_id) = first_building_door(&world, building_id) {
            let _ = crate::world::open_door(&mut world, door_id);
            inspector.last_message = format!("Opened door #{}", door_id.raw());
            refresh = true;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyL) {
        if let Some(door_id) = first_building_door(&world, building_id) {
            let _ = crate::world::lock_door(&mut world, door_id);
            inspector.last_message = format!("Locked door #{}", door_id.raw());
            refresh = true;
        }
    }

    if keyboard.just_pressed(KeyCode::KeyI) {
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
                inspector.last_message = format!("Building #{:?} has no inventory", building_id);
            }
            refresh = true;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyG) {
        if let Some(inventory_id) = world.get_building(building_id).and_then(|r| r.inventory_id) {
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
                    refresh = true;
                }
                Err(error) => inspector.last_message = format!("Add gold failed: {error}"),
            }
        }
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        if let (Some(unit_id), Some(building_inventory)) = (
            inspector.selected_unit,
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
                        refresh = true;
                    }
                    Err(error) => inspector.last_message = format!("Transfer failed: {error}"),
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
                        refresh = true;
                    }
                    Err(error) => inspector.last_message = format!("Transfer failed: {error}"),
                }
            } else {
                inspector.last_message =
                    "Select unit with inventory for unit↔building transfer".to_string();
            }
        } else {
            inspector.last_message =
                "Select unit and building with inventories for transfer".to_string();
        }
    }
    if keyboard.just_pressed(KeyCode::KeyU) {
        if let Some(record) = world.get_building(building_id) {
            if record.inventory_id.is_some() {
                let locked = !record.container_locked;
                if set_building_container_locked(&mut world, building_id, locked).is_ok() {
                    inspector.last_message = format!(
                        "Building #{:?} container {}",
                        building_id,
                        if locked { "locked" } else { "unlocked" }
                    );
                    refresh = true;
                }
            }
        }
    }
    if keyboard.just_pressed(KeyCode::KeyV) {
        let errors = validate_building_inventory_links(&world);
        inspector.last_message = if errors.is_empty() {
            "Building inventory links OK".to_string()
        } else {
            format!("Building inventory errors: {errors:?}")
        };
        refresh = true;
    }

    if keyboard.just_pressed(KeyCode::Comma) {
        let enabled = world
            .building_production_store()
            .get_policy(building_id)
            .map(|policy| !policy.enabled)
            .unwrap_or(true);
        match set_production_enabled(&mut world, building_id, enabled) {
            Ok(()) => {
                inspector.last_message = format!(
                    "Production {} for building #{}",
                    if enabled { "enabled" } else { "disabled" },
                    building_id.raw()
                );
                refresh = true;
            }
            Err(error) => inspector.last_message = format!("Production enable failed: {error}"),
        }
    }
    if keyboard.just_pressed(KeyCode::Period) {
        let paused = world
            .building_production_store()
            .get_policy(building_id)
            .map(|policy| !policy.paused)
            .unwrap_or(true);
        match set_production_paused(&mut world, building_id, paused) {
            Ok(()) => {
                inspector.last_message = format!(
                    "Production {} for building #{}",
                    if paused { "paused" } else { "resumed" },
                    building_id.raw()
                );
                refresh = true;
            }
            Err(error) => inspector.last_message = format!("Production pause failed: {error}"),
        }
    }
    if keyboard.just_pressed(KeyCode::Slash) {
        if let Some(definition) = world
            .get_building(building_id)
            .and_then(|record| params.building_catalog.get(&record.definition_id))
        {
            world
                .building_production_store_mut()
                .ensure_policy_for_building(
                    building_id,
                    definition,
                    &building_sim.operation_catalog,
                );
        }
        let next_mode = match world
            .building_production_store()
            .get_policy(building_id)
            .map(|policy| policy.repeat_mode)
            .unwrap_or(RepeatMode::Continuous)
        {
            RepeatMode::Continuous => RepeatMode::Count(3),
            RepeatMode::Count(_) => RepeatMode::Continuous,
        };
        match set_production_execution_mode(&mut world, building_id, next_mode) {
            Ok(()) => {
                inspector.last_message = format!(
                    "Production mode set to {} for building #{}",
                    next_mode.display_label(),
                    building_id.raw()
                );
                refresh = true;
            }
            Err(error) => inspector.last_message = format!("Production mode failed: {error}"),
        }
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        match reset_production_progress(&mut world, building_id) {
            Ok(()) => {
                inspector.last_message =
                    format!("Reset production progress for building #{}", building_id.raw());
                refresh = true;
            }
            Err(error) => inspector.last_message = format!("Production reset failed: {error}"),
        }
    }
    if keyboard.just_pressed(KeyCode::Backquote) {
        inspector.production_advanced_expanded = !inspector.production_advanced_expanded;
        inspector.last_message = if inspector.production_advanced_expanded {
            "Production advanced panel expanded".to_string()
        } else {
            "Production advanced panel collapsed".to_string()
        };
        refresh = true;
    }
    if keyboard.just_pressed(KeyCode::Backslash) {
        let issues = validate_production_runtime_with_catalogs(
            &world,
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
        refresh = true;
    }

    if keyboard.just_pressed(KeyCode::Quote) {
        match cycle_production_selected_operation(
            &mut world,
            &params.building_catalog,
            &building_sim.operation_catalog,
            building_id,
            true,
        ) {
            Ok(Some(operation)) => {
                inspector.last_message =
                    format!("Selected operation {} for building #{}", operation, building_id.raw());
                refresh = true;
            }
            Ok(None) => inspector.last_message = "Building has no supported operations".into(),
            Err(error) => inspector.last_message = format!("Operation select failed: {error}"),
        }
    }
    if keyboard.just_pressed(KeyCode::Semicolon) {
        match cycle_production_selected_operation(
            &mut world,
            &params.building_catalog,
            &building_sim.operation_catalog,
            building_id,
            false,
        ) {
            Ok(Some(operation)) => {
                inspector.last_message =
                    format!("Selected operation {} for building #{}", operation, building_id.raw());
                refresh = true;
            }
            Ok(None) => inspector.last_message = "Building has no supported operations".into(),
            Err(error) => inspector.last_message = format!("Operation select failed: {error}"),
        }
    }

    if keyboard.just_pressed(KeyCode::KeyM) {
        if let Some(record) = world.get_building(building_id) {
            if let Some(definition) = params.building_catalog.get(&record.definition_id) {
                world
                    .building_production_store_mut()
                    .ensure_policy_for_building(building_id, definition, &building_sim.operation_catalog);
                if let Some(selected) = world
                    .building_production_store()
                    .get_policy(building_id)
                    .and_then(|policy| policy.selected_operation.clone())
                {
                    if let Some(op_def) = building_sim.operation_catalog.get(&selected) {
                        world
                            .building_production_store_mut()
                            .get_state_mut(building_id)
                            .progress =
                            ProductionProgress(PRODUCTION_PROGRESS_ONE_UNIT);
                        match execute_production_cycle(
                            &mut world,
                            &inventory_ctx,
                            building_id,
                            op_def,
                            definition,
                        ) {
                            Ok(()) => {
                                let state = world
                                    .building_production_store_mut()
                                    .get_state_mut(building_id);
                                state
                                    .progress
                                    .completions_since(crate::world::PRODUCTION_PROGRESS_ONE_UNIT);
                                state.completion_count = state.completion_count.saturating_add(1);
                                inspector.last_message = format!(
                                    "Force-executed production cycle for building #{}",
                                    building_id.raw()
                                );
                                refresh = true;
                            }
                            Err(factor) => {
                                inspector.last_message =
                                    format!("Force execute blocked: {}", factor.label());
                            }
                        }
                    }
                }
            }
        }
    }
    if keyboard.just_pressed(KeyCode::KeyK) {
        if let Some(set) = world.building_inventory_binding_store().get(building_id).cloned() {
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
            inspector.last_message =
                format!("Cleared binding inventories for building #{}", building_id.raw());
            refresh = true;
        }
    }

    if keyboard.just_pressed(KeyCode::KeyF) {
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
            &world,
            &catalogs,
            &mut building_sim.assessment_store,
            building_id,
        ) {
            AssessmentRebuildOutcome::Assessed => {
                inspector.last_message =
                    format!("Refreshed terrain assessment for building #{}", building_id.raw());
                refresh = true;
            }
            outcome => {
                inspector.last_message = format!("Terrain assessment refresh: {outcome:?}");
            }
        }
    }

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    if keyboard.just_pressed(KeyCode::KeyP) && shift && !ctrl {
        if let Some(settlement_id) = world.settlement_store().settlement_for_building(building_id) {
            let mut planner = world
                .production_planner_store()
                .get(settlement_id)
                .cloned()
                .unwrap_or_default();
            planner.mark_dirty();
            crate::world::execute_settlement_replan(
                &mut world,
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
            refresh = true;
        } else {
            inspector.last_message = "Building is not linked to a settlement".to_string();
        }
    }
    if keyboard.just_pressed(KeyCode::KeyQ) {
        if ctrl {
            if let Some(request_id) = world
                .hauling_request_store()
                .requests_for_building(building_id)
                .first()
                .copied()
            {
                match crate::world::force_complete_hauling_request(
                    &mut world,
                    request_id,
                    &inventory_ctx,
                ) {
                    Ok(moved) => {
                        inspector.last_message =
                            format!("Force-completed haul #{}, moved {moved}", request_id.raw());
                        refresh = true;
                    }
                    Err(reason) => {
                        inspector.last_message =
                            format!("Force-complete failed: {}", reason.label());
                    }
                }
            } else {
                inspector.last_message = "No hauling requests to complete".to_string();
            }
        } else if shift {
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
                crate::world::cancel_hauling_request(&mut world, request_id);
            }
            inspector.last_message =
                format!("Cancelled open hauling requests for building #{}", building_id.raw());
            refresh = true;
        } else if let Some(definition) = world
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
                    use LogisticsRouteTrigger;
                    let (source, destination) = match route.trigger {
                        LogisticsRouteTrigger::OutputSurplus => (local_inventory, remote_inventory),
                        LogisticsRouteTrigger::InputDeficit => (remote_inventory, local_inventory),
                    };
                    if let Some(request_id) = crate::world::spawn_manual_hauling_request(
                        &mut world,
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
                        refresh = true;
                    } else {
                        inspector.last_message = "Failed to spawn hauling request".to_string();
                    }
                } else {
                    inspector.last_message =
                        "Could not resolve logistics route inventories".to_string();
                }
            } else {
                inspector.last_message = "Building has no logistics routes".to_string();
            }
        }
    }

    if refresh {
        let inventory_ctx = params.inventory_ctx();
        let mut operation = building_sim.operation_params(
            &params.building_catalog,
            &params.footprint_catalog,
            &inventory_ctx,
        );
        let operation_probe = probe_building_operation(
            &world,
            &params.building_catalog,
            &mut operation,
            building_id,
        );
        inspector.building_snapshot = capture_building_inspector_snapshot(
            &world,
            &params.building_catalog,
            &params.interaction_catalog,
            building_id,
            None,
            Some(operation_probe),
        );
    }
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
