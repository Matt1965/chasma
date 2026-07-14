//! Dev Mode settlement treasury tools (ADR-093 I7).

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::dev::inspector::WorldInspectorState;
use crate::dev::{DevModeState, DevTab};
use crate::simulation::SimulationControlState;
use crate::world::{
    BuildingCatalog, BuildingInteractionProfileCatalog, InventoryCatalogCtx,
    InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, SettlementOwnership,
    TreasuryAccessPolicy, WorldData, count_physical_gold, create_settlement_with_treasury,
    deposit_gold,
};

pub fn format_treasury_harness_detail(
    world: &WorldData,
    inspector: &WorldInspectorState,
    message: &str,
) -> String {
    let building_line = inspector
        .selected_building
        .map(|id| format!("Selected building: {id:?}"))
        .unwrap_or_else(|| "Selected building: none (Alt+click building)".into());
    let unit_line = inspector
        .selected_unit
        .map(|id| format!("Selected unit: {id:?}"))
        .unwrap_or_else(|| "Selected unit: none".into());
    let settlement_count = world.settlement_store().sorted_settlement_ids().len();
    format!(
        "{building_line}\n{unit_line}\nSettlements: {settlement_count}\n\
         C=create treasury · Y=inspect · E=deposit 5 · B=validate wealth · J=transaction log\n\
         {message}"
    )
}

pub fn handle_treasury_harness_keyboard(
    mut dev_state: ResMut<DevModeState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut world: ResMut<WorldData>,
    inspector: Res<WorldInspectorState>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
    building_catalog: Res<BuildingCatalog>,
    interaction_catalog: Res<BuildingInteractionProfileCatalog>,
    simulation: Res<SimulationControlState>,
) {
    if !dev_state.enabled || dev_state.active_tab != DevTab::WorldTools {
        return;
    }
    if dev_state.has_text_focus() {
        return;
    }

    let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    let tick = simulation.current_tick;
    let mut message = dev_state.treasury_harness_message.clone();

    if keyboard.just_pressed(KeyCode::KeyJ) {
        let log = world.settlement_store().transaction_log();
        if log.is_empty() {
            message = "Transaction log empty".to_string();
        } else {
            let tail = log.iter().rev().take(5).collect::<Vec<_>>();
            message = format!(
                "Last {} deposits: {:?}",
                tail.len(),
                tail.into_iter()
                    .map(|entry| format!(
                        "tick {} +{} -> {}",
                        entry.tick, entry.deposited_gold, entry.balance_after
                    ))
                    .collect::<Vec<_>>()
            );
        }
    }

    if keyboard.just_pressed(KeyCode::KeyB) {
        let physical: u64 = world
            .inventory_store()
            .sorted_inventory_ids()
            .iter()
            .filter_map(|id| world.inventory_store().get(*id))
            .map(count_physical_gold)
            .map(u64::from)
            .sum();
        let treasury: u64 = world
            .settlement_store()
            .sorted_treasury_ids()
            .iter()
            .filter_map(|id| world.settlement_store().get_treasury(*id))
            .map(|t| t.balance_gold)
            .sum();
        message = format!("World wealth — physical gold: {physical}, treasury gold: {treasury}");
    }

    let Some(building_id) = inspector.selected_building else {
        dev_state.treasury_harness_message = message;
        return;
    };

    if keyboard.just_pressed(KeyCode::KeyC) {
        let Some(building) = world.get_building(building_id).cloned() else {
            dev_state.treasury_harness_message = "Building missing".to_string();
            return;
        };
        match create_settlement_with_treasury(
            &mut world,
            &building_catalog,
            &interaction_catalog,
            building_id,
            "Dev Settlement",
            SettlementOwnership::player_default(),
            building.placement.position,
            tick,
        ) {
            Ok(report) => {
                message = format!(
                    "Created settlement {:?} treasury {:?}",
                    report.settlement_id, report.treasury_id
                );
            }
            Err(err) => message = err.to_string(),
        }
    }

    if keyboard.just_pressed(KeyCode::KeyY) {
        if let Some(settlement_id) = world
            .settlement_store()
            .settlement_for_building(building_id)
        {
            let settlement = world.settlement_store().get_settlement(settlement_id);
            let treasury = world
                .settlement_store()
                .treasury_for_settlement(settlement_id)
                .and_then(|id| world.settlement_store().get_treasury(id));
            message = format!("Settlement {:?} treasury {:?}", settlement, treasury);
        } else {
            message = "Building has no settlement treasury".to_string();
        }
    }

    if keyboard.just_pressed(KeyCode::KeyE) {
        let Some(unit_id) = inspector.selected_unit else {
            dev_state.treasury_harness_message =
                "Select a unit (Alt+click) to deposit gold".to_string();
            return;
        };
        let Some(settlement_id) = world
            .settlement_store()
            .settlement_for_building(building_id)
        else {
            dev_state.treasury_harness_message = "Building has no treasury".to_string();
            return;
        };
        let Some(treasury_id) = world
            .settlement_store()
            .treasury_for_settlement(settlement_id)
        else {
            dev_state.treasury_harness_message = "Treasury missing for settlement".to_string();
            return;
        };
        let Some(inventory_id) = world.get_unit(unit_id).and_then(|u| u.inventory_id) else {
            dev_state.treasury_harness_message = "Unit has no inventory".to_string();
            return;
        };
        match deposit_gold(
            &mut world,
            &building_catalog,
            &interaction_catalog,
            &ctx,
            unit_id,
            inventory_id,
            treasury_id,
            5,
            TreasuryAccessPolicy::OwnerOnly,
            tick,
        ) {
            Ok(report) => {
                message = format!(
                    "Deposited {} — treasury balance {}",
                    report.deposited_gold, report.treasury_balance_after
                );
            }
            Err(err) => message = err.to_string(),
        }
    }

    dev_state.treasury_harness_message = message;
}
