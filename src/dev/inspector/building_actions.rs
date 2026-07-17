//! Dev inspector building lifecycle shortcuts (ADR-082 B5, ADR-091 I5).

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::simulation::SimulationControlState;
use crate::world::{
    BuildingInteractionProfileCatalog, BuildingInventoryContext, BuildingInventoryRemovalPolicy,
    BuildingLifecycleState, InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog,
    ItemCategoryCatalog, ItemDefinitionId, ItemPileSettings, OccupancyCatalogs,
    TransferPlacementPolicy, add_building_construction_progress, damage_building, destroy_building,
    heal_building, place_stack_first_fit, set_building_container_locked,
    set_building_lifecycle_stage, transfer_one, validate_building_inventory_links,
};

use super::capture::capture_building_inspector_snapshot;
use super::state::WorldInspectorState;

pub fn handle_building_dev_actions(
    dev_state: Res<crate::dev::DevModeState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    doodad_catalog: Res<crate::world::DoodadCatalog>,
    building_catalog: Res<crate::world::BuildingCatalog>,
    footprint_catalog: Res<crate::world::FootprintCatalog>,
    interior_catalog: Res<crate::world::InteriorProfileCatalog>,
    interaction_catalog: Res<BuildingInteractionProfileCatalog>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
    pile_settings: Res<ItemPileSettings>,
    simulation: Res<SimulationControlState>,
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
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint_catalog,
    };
    let inventory_ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    let inventory_cleanup = BuildingInventoryContext {
        ctx: &inventory_ctx,
        pile_settings: &pile_settings,
        interaction_catalog: &interaction_catalog,
        tick: simulation.current_tick,
    };

    let mut refresh = false;
    if keyboard.just_pressed(KeyCode::KeyD) {
        let _ = damage_building(
            &mut world,
            &building_catalog,
            &doodad_catalog,
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
            &building_catalog,
            &doodad_catalog,
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
            &building_catalog,
            &interior_catalog,
            &doodad_catalog,
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
            &building_catalog,
            &interior_catalog,
            &doodad_catalog,
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
            &building_catalog,
            &interior_catalog,
            &doodad_catalog,
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

    if refresh {
        inspector.building_snapshot = capture_building_inspector_snapshot(
            &world,
            &building_catalog,
            &interaction_catalog,
            building_id,
            None,
            None,
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
