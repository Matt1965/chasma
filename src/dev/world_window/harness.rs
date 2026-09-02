//! Pile and treasury harness UI actions (Slice 12).

use bevy::prelude::*;

use crate::client::selection::WorldSelectionState;
use crate::dev::dev_mode::DevModeState;
use crate::dev::input::DevPanelUi;
use crate::dev::widgets::spawn_action_button;
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::simulation::SimulationControlState;
use crate::units::input::SelectedUnits;
use crate::world::{
    BuildingCatalog, BuildingInteractionProfileCatalog, InventoryCatalogCtx,
    InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, ItemPileSettings, WorldData,
};

use crate::dev::pile_harness::{
    PileHarnessAction, apply_pile_harness_action, format_pile_harness_detail,
};
use crate::dev::treasury_harness::{
    TreasuryHarnessAction, apply_treasury_harness_action, format_treasury_harness_detail,
};

#[derive(Component, Debug)]
pub(crate) struct DevWorldHarnessButtons;

#[derive(Component, Debug, Clone, Copy)]
pub struct DevPileHarnessButton {
    pub action: PileHarnessAction,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DevTreasuryHarnessButton {
    pub action: TreasuryHarnessAction,
}

pub fn spawn_harness_buttons(parent: &mut ChildSpawnerCommands<'_>) {
    parent
        .spawn((
            DevWorldHarnessButtons,
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
        ))
        .with_children(|col| {
            col.spawn((
                DevPanelUi,
                Text::new("Pile harness"),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgba(0.75, 0.85, 0.92, 1.0)),
            ));
            spawn_pile_row(
                col,
                &[
                    (PileHarnessAction::ValidateWorld, "Validate world"),
                    (PileHarnessAction::SpawnGoldPile, "Spawn gold pile"),
                    (PileHarnessAction::DropEntry, "Drop entry 0"),
                    (PileHarnessAction::DropOne, "Drop one"),
                    (PileHarnessAction::DropHalf, "Drop half"),
                    (PileHarnessAction::PickupPile, "Pickup pile"),
                    (PileHarnessAction::LootCorpse, "Loot corpse"),
                ],
            );
            col.spawn((
                DevPanelUi,
                Text::new("Treasury harness"),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgba(0.75, 0.85, 0.92, 1.0)),
            ));
            spawn_treasury_row(
                col,
                &[
                    (TreasuryHarnessAction::TransactionLog, "Transaction log"),
                    (TreasuryHarnessAction::SumWealth, "Sum wealth"),
                    (TreasuryHarnessAction::CreateSettlement, "Create settlement"),
                    (TreasuryHarnessAction::Inspect, "Inspect"),
                    (TreasuryHarnessAction::DepositGold, "Deposit 5 gold"),
                ],
            );
        });
}

fn spawn_pile_row(
    parent: &mut ChildSpawnerCommands<'_>,
    pile_actions: &[(PileHarnessAction, &str)],
) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .with_children(|row| {
            for (action, label) in pile_actions {
                spawn_action_button(
                    row,
                    *label,
                    Some(action.tooltip()),
                    DevPileHarnessButton { action: *action },
                );
            }
        });
}

fn spawn_treasury_row(
    parent: &mut ChildSpawnerCommands<'_>,
    actions: &[(TreasuryHarnessAction, &str)],
) {
    parent
        .spawn((
            DevPanelUi,
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .with_children(|row| {
            for (action, label) in actions {
                spawn_action_button(
                    row,
                    *label,
                    Some(action.tooltip()),
                    DevTreasuryHarnessButton { action: *action },
                );
            }
        });
}

pub fn handle_pile_harness_buttons(
    registry: Res<DevWindowRegistry>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut dev_state: ResMut<DevModeState>,
    mut world: ResMut<WorldData>,
    world_selection: Res<WorldSelectionState>,
    selected_units: Res<SelectedUnits>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
    settings: Res<ItemPileSettings>,
    simulation: Res<SimulationControlState>,
    buttons: Query<(&Interaction, &DevPileHarnessButton), Changed<Interaction>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        dev_state.pile_harness_message = apply_pile_harness_action(
            button.action,
            &mut world,
            &world_selection,
            &selected_units,
            &ctx,
            &settings,
            simulation.current_tick,
        );
    }
}

pub fn handle_treasury_harness_buttons(
    registry: Res<DevWindowRegistry>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut dev_state: ResMut<DevModeState>,
    mut world: ResMut<WorldData>,
    world_selection: Res<WorldSelectionState>,
    selected_units: Res<SelectedUnits>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
    building_catalog: Res<BuildingCatalog>,
    interaction_catalog: Res<BuildingInteractionProfileCatalog>,
    simulation: Res<SimulationControlState>,
    buttons: Query<(&Interaction, &DevTreasuryHarnessButton), Changed<Interaction>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        dev_state.treasury_harness_message = apply_treasury_harness_action(
            button.action,
            &mut world,
            &world_selection,
            &selected_units,
            &ctx,
            &building_catalog,
            &interaction_catalog,
            simulation.current_tick,
        );
    }
}

pub fn sync_world_harness_status(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    world: Res<WorldData>,
    world_selection: Res<WorldSelectionState>,
    selected_units: Res<SelectedUnits>,
    mut text: Query<&mut Text, With<super::panel::DevWorldHarnessText>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }
    let Ok(mut label) = text.single_mut() else {
        return;
    };
    let pile = format_pile_harness_detail(
        &world,
        &world_selection,
        &selected_units,
        &dev_state.pile_harness_message,
    );
    let treasury = format_treasury_harness_detail(
        &world,
        &world_selection,
        &selected_units,
        &dev_state.treasury_harness_message,
    );
    let settlement = crate::dev::settlement_placement::settlement_placement_status(&dev_state);
    let settlement_line = if settlement.is_empty() {
        String::new()
    } else {
        format!("\n\n{settlement}")
    };
    **label = format!("{pile}\n\n{treasury}{settlement_line}");
}
