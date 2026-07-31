//! Dev Mode settlement treasury tools (ADR-093 I7) — UI-driven (Slice 12).

use bevy::prelude::*;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::units::input::SelectedUnits;
use crate::world::{
    BuildingCatalog, BuildingInteractionProfileCatalog, InventoryCatalogCtx, SettlementOwnership,
    TreasuryAccessPolicy, WorldData, count_physical_gold, create_settlement_with_treasury,
    deposit_gold,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreasuryHarnessAction {
    TransactionLog,
    SumWealth,
    CreateSettlement,
    Inspect,
    DepositGold,
}

impl TreasuryHarnessAction {
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::TransactionLog => "Show the last five treasury deposit transactions.",
            Self::SumWealth => "Sum physical gold in inventories plus treasury balances.",
            Self::CreateSettlement => {
                "Create a dev settlement and treasury for the selected building."
            }
            Self::Inspect => "Print settlement and treasury state for the selected building.",
            Self::DepositGold => {
                "Deposit 5 gold from the selected unit into the building's settlement treasury."
            }
        }
    }
}

pub fn format_treasury_harness_detail(
    world: &WorldData,
    world_selection: &WorldSelectionState,
    selected_units: &SelectedUnits,
    message: &str,
) -> String {
    let building_line = (world_selection.category == WorldSelectionCategory::Building)
        .then_some(world_selection.building_id)
        .flatten()
        .map(|id| format!("Selected building: {id:?}"))
        .unwrap_or_else(|| "Selected building: none (Alt+click building)".into());
    let unit_line = world_selection
        .primary_unit(selected_units)
        .map(|id| format!("Selected unit: {id:?}"))
        .unwrap_or_else(|| "Selected unit: none".into());
    let settlement_count = world.settlement_store().sorted_settlement_ids().len();
    format!("{building_line}\n{unit_line}\nSettlements: {settlement_count}\n{message}")
}

pub fn apply_treasury_harness_action(
    action: TreasuryHarnessAction,
    world: &mut WorldData,
    world_selection: &WorldSelectionState,
    selected_units: &SelectedUnits,
    ctx: &InventoryCatalogCtx,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    tick: u64,
) -> String {
    match action {
        TreasuryHarnessAction::TransactionLog => {
            let log = world.settlement_store().transaction_log();
            if log.is_empty() {
                "Transaction log empty".to_string()
            } else {
                let tail = log.iter().rev().take(5).collect::<Vec<_>>();
                format!(
                    "Last {} deposits: {:?}",
                    tail.len(),
                    tail.into_iter()
                        .map(|entry| format!(
                            "tick {} +{} -> {}",
                            entry.tick, entry.deposited_gold, entry.balance_after
                        ))
                        .collect::<Vec<_>>()
                )
            }
        }
        TreasuryHarnessAction::SumWealth => {
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
            format!("World wealth — physical gold: {physical}, treasury gold: {treasury}")
        }
        TreasuryHarnessAction::CreateSettlement
        | TreasuryHarnessAction::Inspect
        | TreasuryHarnessAction::DepositGold => {
            let Some(building_id) = (world_selection.category == WorldSelectionCategory::Building)
                .then_some(world_selection.building_id)
                .flatten()
            else {
                return "Select a building (Alt+click)".to_string();
            };
            match action {
                TreasuryHarnessAction::CreateSettlement => {
                    let Some(building) = world.get_building(building_id).cloned() else {
                        return "Building missing".to_string();
                    };
                    match create_settlement_with_treasury(
                        world,
                        building_catalog,
                        interaction_catalog,
                        building_id,
                        "Dev Settlement",
                        SettlementOwnership::player_default(),
                        building.placement.position,
                        tick,
                    ) {
                        Ok(report) => format!(
                            "Created settlement {:?} treasury {:?}",
                            report.settlement_id, report.treasury_id
                        ),
                        Err(err) => err.to_string(),
                    }
                }
                TreasuryHarnessAction::Inspect => {
                    if let Some(settlement_id) = world
                        .settlement_store()
                        .settlement_for_building(building_id)
                    {
                        let settlement = world.settlement_store().get_settlement(settlement_id);
                        let treasury = world
                            .settlement_store()
                            .treasury_for_settlement(settlement_id)
                            .and_then(|id| world.settlement_store().get_treasury(id));
                        format!("Settlement {:?} treasury {:?}", settlement, treasury)
                    } else {
                        "Building has no settlement treasury".to_string()
                    }
                }
                TreasuryHarnessAction::DepositGold => {
                    let Some(unit_id) = world_selection.primary_unit(selected_units) else {
                        return "Select a unit (Alt+click) to deposit gold".to_string();
                    };
                    let Some(settlement_id) = world
                        .settlement_store()
                        .settlement_for_building(building_id)
                    else {
                        return "Building has no treasury".to_string();
                    };
                    let Some(treasury_id) = world
                        .settlement_store()
                        .treasury_for_settlement(settlement_id)
                    else {
                        return "Treasury missing for settlement".to_string();
                    };
                    let Some(inventory_id) = world.get_unit(unit_id).and_then(|u| u.inventory_id)
                    else {
                        return "Unit has no inventory".to_string();
                    };
                    match deposit_gold(
                        world,
                        building_catalog,
                        interaction_catalog,
                        ctx,
                        unit_id,
                        inventory_id,
                        treasury_id,
                        5,
                        TreasuryAccessPolicy::OwnerOnly,
                        tick,
                    ) {
                        Ok(report) => format!(
                            "Deposited {} — treasury balance {}",
                            report.deposited_gold, report.treasury_balance_after
                        ),
                        Err(err) => err.to_string(),
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}
