//! Settlement Dev window interaction handlers.

use bevy::prelude::*;

use crate::client::CameraSettlementContext;
use crate::dev::dev_mode::{DevModeInputGate, DevModeState};
use crate::dev::widgets::{DevWidgetActionButton, DevWidgetToggle};
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::units::input::SelectedUnits;
use crate::world::WorldData;

use super::model::assign_selected_units_to_settlement;
use super::panel::{DevSettlementAddUnitsButton, DevSettlementAiToggle};

pub fn handle_settlement_add_units_button(
    registry: Res<DevWindowRegistry>,
    mut gate: ResMut<DevModeInputGate>,
    mut dev_state: ResMut<DevModeState>,
    context: Res<CameraSettlementContext>,
    selected_units: Res<SelectedUnits>,
    mut world: ResMut<WorldData>,
    buttons: Query<
        (
            &Interaction,
            &DevWidgetActionButton,
            &DevSettlementAddUnitsButton,
        ),
        Changed<Interaction>,
    >,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Settlement) {
        return;
    }
    for (interaction, widget, _) in &buttons {
        if *interaction != Interaction::Pressed || widget.disabled {
            continue;
        }
        gate.block_gameplay_mouse = true;
        let Some(settlement_id) = context.focused_settlement_id else {
            dev_state.settlement_placement_message =
                "No focused settlement — look toward a player settlement".into();
            continue;
        };
        if selected_units.0.is_empty() {
            dev_state.settlement_placement_message =
                "Select unit(s) before assigning to settlement".into();
            continue;
        }
        let unit_ids: Vec<_> = selected_units.0.iter().copied().collect();
        dev_state.settlement_placement_message =
            match assign_selected_units_to_settlement(&mut world, &unit_ids, settlement_id) {
                Ok(count) => {
                    let name = world
                        .settlement_store()
                        .get_settlement(settlement_id)
                        .map(|record| record.display_name.clone())
                        .unwrap_or_else(|| "settlement".into());
                    format!("Assigned {count} unit(s) to {name}")
                }
                Err(error) => format!("Unit assignment failed: {error}"),
            };
    }
}

pub fn handle_settlement_ai_toggle(
    registry: Res<DevWindowRegistry>,
    mut gate: ResMut<DevModeInputGate>,
    mut dev_state: ResMut<DevModeState>,
    context: Res<CameraSettlementContext>,
    mut world: ResMut<WorldData>,
    toggles: Query<(&Interaction, &DevWidgetToggle, &DevSettlementAiToggle), Changed<Interaction>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::Settlement) {
        return;
    }
    for (interaction, toggle, _) in &toggles {
        if *interaction != Interaction::Pressed || toggle.disabled {
            continue;
        }
        gate.block_gameplay_mouse = true;
        let Some(settlement_id) = context.focused_settlement_id else {
            continue;
        };
        let Some(state) = world.settlement_state_store_mut().get_mut(settlement_id) else {
            dev_state.settlement_placement_message =
                "Settlement state missing for focused settlement".into();
            continue;
        };
        state.policies.automation_enabled = !state.policies.automation_enabled;
        let enabled = state.policies.automation_enabled;
        dev_state.settlement_placement_message = format!(
            "Settlement AI {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }
}
