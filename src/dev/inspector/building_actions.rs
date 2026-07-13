//! Dev inspector building lifecycle shortcuts (ADR-082 B5).

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::world::{
    BuildingLifecycleState, OccupancyCatalogs, add_building_construction_progress, damage_building,
    destroy_building, heal_building, set_building_lifecycle_stage,
};

use super::capture::capture_building_inspector_snapshot;
use super::params::InspectorCaptureParams;
use super::state::WorldInspectorState;

pub fn handle_building_dev_actions(
    dev_state: Res<crate::dev::DevModeState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    capture: InspectorCaptureParams,
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
        doodad: &capture.doodad_catalog,
        building: &capture.building_catalog,
        footprint: &capture.footprint_catalog,
    };

    let mut refresh = false;
    if keyboard.just_pressed(KeyCode::KeyD) {
        let _ = damage_building(
            &mut world,
            &capture.building_catalog,
            &capture.doodad_catalog,
            occ,
            building_id,
            50,
            "dev_damage",
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
            &capture.building_catalog,
            &capture.doodad_catalog,
            occ,
            building_id,
            "dev_destroy",
        );
        inspector.last_message = format!("Destroyed building #{}", building_id.raw());
        refresh = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        let _ = set_building_lifecycle_stage(
            &mut world,
            &capture.building_catalog,
            &capture.interior_catalog,
            &capture.doodad_catalog,
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
            &capture.building_catalog,
            &capture.interior_catalog,
            &capture.doodad_catalog,
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
            &capture.building_catalog,
            &capture.interior_catalog,
            &capture.doodad_catalog,
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

    if refresh {
        inspector.building_snapshot =
            capture_building_inspector_snapshot(&world, &capture.building_catalog, building_id);
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
