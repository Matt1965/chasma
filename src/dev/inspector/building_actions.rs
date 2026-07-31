//! Building production repeat toggle (Slice 1 / 12).

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::simulation::BuildingSimulationParams;
use crate::world::{RepeatMode, set_production_execution_mode};

use super::capture::{capture_building_inspector_snapshot, probe_building_operation};
use super::input::BuildingProductionRepeatModeButton;
use super::params::DevBuildingActionParams;
use super::state::WorldInspectorState;

/// Toggle production repeat mode for the selected building (formerly `/` hotkey).
pub fn toggle_building_production_repeat_mode(
    world: &mut crate::world::WorldData,
    building_id: crate::world::BuildingId,
    building_catalog: &crate::world::BuildingCatalog,
    operation_catalog: &crate::world::OperationCatalog,
) -> Result<RepeatMode, String> {
    if let Some(definition) = world
        .get_building(building_id)
        .and_then(|record| building_catalog.get(&record.definition_id))
    {
        world
            .building_production_store_mut()
            .ensure_policy_for_building(building_id, definition, operation_catalog);
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
    set_production_execution_mode(world, building_id, next_mode)
        .map(|()| next_mode)
        .map_err(|error| error.to_string())
}

pub fn handle_building_production_repeat_button(
    dev_state: Res<crate::dev::DevModeState>,
    world_selection: Res<WorldSelectionState>,
    mut building_sim: BuildingSimulationParams,
    params: DevBuildingActionParams,
    mut world: ResMut<crate::world::WorldData>,
    mut inspector: ResMut<WorldInspectorState>,
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<BuildingProductionRepeatModeButton>,
        ),
    >,
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

    for (interaction, mut color) in &mut buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        *color = BackgroundColor(crate::dev::widgets::theme::BTN_BG_ACCENT);
        match toggle_building_production_repeat_mode(
            &mut world,
            building_id,
            &params.building_catalog,
            &building_sim.operation_catalog,
        ) {
            Ok(next_mode) => {
                inspector.last_message = format!(
                    "Production mode set to {} for building #{}",
                    next_mode.display_label(),
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
            Err(error) => inspector.last_message = format!("Production mode failed: {error}"),
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
