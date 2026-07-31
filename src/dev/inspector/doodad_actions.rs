//! Dev hotkeys for doodad transform editing (ADR-098 DT2).

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::dev::hotkeys::{
    DevShortcutSuppressionCtx, dev_doodad_transform_edit_options, dev_shortcuts_suppressed,
};
use crate::dev::{DevModeInputGate, DevModeState};
use crate::world::{DoodadCatalog, FootprintCatalog};
use crate::world::{
    DoodadTransformCandidate, DoodadTransformEditOptions, OccupancyCatalogs, QuantizedOrientation,
    WorldData, nudge_doodad_position, update_doodad_transform,
};

use super::state::WorldInspectorState;

const POS_STEP: f32 = 0.1;
const ROT_STEP_DEG: f32 = 5.0;
const SCALE_STEP: f32 = 0.05;

pub fn handle_doodad_transform_hotkeys(
    dev_state: Res<DevModeState>,
    mut gate: ResMut<DevModeInputGate>,
    keyboard: Res<ButtonInput<KeyCode>>,
    world_selection: Res<WorldSelectionState>,
    selected_object_ui: Res<crate::dev::selected_object::SelectedObjectUiState>,
    blueprint_inspection: Res<super::BlueprintInspectionState>,
    mut inspector: ResMut<WorldInspectorState>,
    mut world: ResMut<WorldData>,
    doodad_catalog: Res<DoodadCatalog>,
    building_catalog: Res<crate::world::BuildingCatalog>,
    footprint_catalog: Res<FootprintCatalog>,
) {
    if !dev_state.enabled {
        return;
    }
    let suppression =
        DevShortcutSuppressionCtx::new(&dev_state, &selected_object_ui, &blueprint_inspection);
    if dev_shortcuts_suppressed(suppression) {
        return;
    }
    let Some(doodad_id) = (world_selection.category == WorldSelectionCategory::Doodad)
        .then_some(world_selection.doodad_id)
        .flatten()
    else {
        return;
    };
    let Some(record) = world.get_doodad(doodad_id).cloned() else {
        inspector.last_message = format!("Doodad #{} no longer exists", doodad_id.raw());
        return;
    };

    let options = dev_doodad_transform_edit_options();
    let occ = OccupancyCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint_catalog,
    };

    let mut delta = Vec3::ZERO;
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        delta.x -= POS_STEP;
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        delta.x += POS_STEP;
    }
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        delta.z -= POS_STEP;
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        delta.z += POS_STEP;
    }
    if keyboard.just_pressed(KeyCode::PageUp) {
        delta.y += POS_STEP;
    }
    if keyboard.just_pressed(KeyCode::PageDown) {
        delta.y -= POS_STEP;
    }

    if delta != Vec3::ZERO {
        gate.block_gameplay_mouse = true;
        match nudge_doodad_position(
            &mut world,
            &doodad_catalog,
            doodad_id,
            delta,
            options,
            Some(occ),
        ) {
            Ok(report) => {
                inspector.last_message = format!(
                    "Doodad #{} moved — {} cells",
                    doodad_id.raw(),
                    report.occupied_cell_count
                );
            }
            Err(err) => inspector.last_message = format!("Transform failed: {err:?}"),
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::BracketLeft) || keyboard.just_pressed(KeyCode::BracketRight) {
        let sign = if keyboard.just_pressed(KeyCode::BracketRight) {
            1.0
        } else {
            -1.0
        };
        let yaw = record.placement.orientation.yaw_degrees() + sign * ROT_STEP_DEG;
        let orientation = QuantizedOrientation::from_degrees(yaw, 0.0, 0.0)
            .unwrap_or(record.placement.orientation);
        gate.block_gameplay_mouse = true;
        match update_doodad_transform(
            &mut world,
            &doodad_catalog,
            doodad_id,
            DoodadTransformCandidate {
                position: record.placement.position,
                orientation,
                scale: record.placement.scale,
            },
            options,
            Some(occ),
        ) {
            Ok(_) => inspector.last_message = format!("Doodad #{} yaw adjusted", doodad_id.raw()),
            Err(err) => inspector.last_message = format!("Rotate failed: {err:?}"),
        }
    }
}
