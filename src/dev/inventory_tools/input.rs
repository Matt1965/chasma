//! Dev inventory tool input — keyboard shortcuts and ground pile placement (DV0).

use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::dev::dev_mode::{
    DefinitionId, DevInventoryToolState, DevModeInputGate, DevModeState, DevTab, DevTextFieldFocus,
    ItemsBrowserSubtab,
};
use crate::dev::inspector::WorldInspectorState;
use crate::dev::dev_mode::DevInventoryEndpoint;
use crate::dev::inventory_tools::endpoint::resolve_inspector_endpoints;
use crate::dev::inventory_tools::ops::{
    dev_add_item, dev_clear_inventory, dev_fill_inventory, dev_remove_entry, dev_set_stack_quantity,
    dev_spawn_ground_pile, dev_transfer,
};
use crate::dev::{DevPanelHoverState, input::DevSpawnClickParams};
use crate::simulation::SimulationControlState;
use crate::units::input::{cursor_world_ray, terrain_click_to_world_position};
use crate::world::{
    InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog,
    ItemDefinitionId, ItemPileSettings, WorldConfig, WorldData,
};

use super::panel::DevItemsAction;

pub fn handle_dev_items_keyboard(
    dev_state: &mut DevModeState,
    keyboard: &ButtonInput<KeyCode>,
    world: &mut WorldData,
    inspector: &WorldInspectorState,
    items: &ItemCatalog,
    categories: &ItemCategoryCatalog,
    profiles: &InventoryProfileCatalog,
    pile_settings: &ItemPileSettings,
    simulation: &SimulationControlState,
) {
    if !dev_state.enabled || dev_state.active_tab != DevTab::Items {
        return;
    }
    if dev_state.has_text_focus() {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyI) {
        dev_state.inventory.subtab = ItemsBrowserSubtab::Items;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        dev_state.inventory.subtab = ItemsBrowserSubtab::InventoryProfiles;
    }
    if keyboard.just_pressed(KeyCode::KeyH) {
        dev_state.inventory.subtab = ItemsBrowserSubtab::InventoryManage;
    }

    if dev_state.inventory.subtab != ItemsBrowserSubtab::InventoryManage {
        return;
    }

    let ctx = InventoryCatalogCtx::new(items, categories, profiles);
    let tick = simulation.current_tick;

    if keyboard.just_pressed(KeyCode::BracketLeft) {
        let current = dev_state.inventory.quantity;
        dev_state.inventory.quantity = current.saturating_div(10).max(1);
        dev_state.inventory.quantity_input = dev_state.inventory.quantity.to_string();
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        let current = dev_state.inventory.quantity;
        dev_state.inventory.quantity = current.saturating_mul(10).min(10_000);
        dev_state.inventory.quantity_input = dev_state.inventory.quantity.to_string();
    }
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        dev_state.bump_item_quantity(1);
    }
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        dev_state.bump_item_quantity(-1);
    }

    if keyboard.just_pressed(KeyCode::KeyT) {
        cycle_endpoint(dev_state, inspector, world, 1);
    }
    if keyboard.just_pressed(KeyCode::KeyY) {
        cycle_entry(dev_state, world, inspector, 1);
    }

    if keyboard.just_pressed(KeyCode::KeyA) {
        run_action(dev_state, world, inspector, &ctx, pile_settings, tick, DevItemsAction::AddItem);
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        run_action(
            dev_state,
            world,
            inspector,
            &ctx,
            pile_settings,
            tick,
            DevItemsAction::RemoveEntry,
        );
    }
    if keyboard.just_pressed(KeyCode::KeyS) {
        run_action(
            dev_state,
            world,
            inspector,
            &ctx,
            pile_settings,
            tick,
            DevItemsAction::SetQuantity,
        );
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        run_action(
            dev_state,
            world,
            inspector,
            &ctx,
            pile_settings,
            tick,
            DevItemsAction::ClearInventory,
        );
    }
    if keyboard.just_pressed(KeyCode::KeyF) {
        run_action(
            dev_state,
            world,
            inspector,
            &ctx,
            pile_settings,
            tick,
            DevItemsAction::FillInventory,
        );
    }
    if keyboard.just_pressed(KeyCode::KeyG) {
        dev_state.inventory.pile_placement_armed = !dev_state.inventory.pile_placement_armed;
        dev_state.inventory.message = if dev_state.inventory.pile_placement_armed {
            "Ground pile placement armed".into()
        } else {
            "Ground pile placement cancelled".into()
        };
    }
    if keyboard.just_pressed(KeyCode::KeyV) {
        let report = crate::world::validate_world_inventory_state(world, &ctx);
        dev_state.inventory.message = if report.is_ok() {
            "World inventory validation OK".into()
        } else {
            format!("Validation: {report:?}")
        };
    }
}

pub fn handle_dev_items_panel_action(
    dev_state: &mut DevModeState,
    world: &mut WorldData,
    inspector: &WorldInspectorState,
    items: &ItemCatalog,
    categories: &ItemCategoryCatalog,
    profiles: &InventoryProfileCatalog,
    pile_settings: &ItemPileSettings,
    simulation: &SimulationControlState,
    action: DevItemsAction,
) {
    if !dev_state.enabled || dev_state.active_tab != DevTab::Items {
        return;
    }
    let ctx = InventoryCatalogCtx::new(items, categories, profiles);
    match action {
        DevItemsAction::SubtabItems => dev_state.inventory.subtab = ItemsBrowserSubtab::Items,
        DevItemsAction::SubtabProfiles => {
            dev_state.inventory.subtab = ItemsBrowserSubtab::InventoryProfiles;
        }
        DevItemsAction::SubtabManage => {
            dev_state.inventory.subtab = ItemsBrowserSubtab::InventoryManage;
        }
        DevItemsAction::CycleEndpoint => cycle_endpoint(dev_state, inspector, world, 1),
        DevItemsAction::CycleEntry => cycle_entry(dev_state, world, inspector, 1),
        other => run_action(
            dev_state,
            world,
            inspector,
            &ctx,
            pile_settings,
            simulation.current_tick,
            other,
        ),
    }
}

pub fn handle_dev_items_ground_click(
    mut params: DevSpawnClickParams,
    panel_hovered: Res<DevPanelHoverState>,
    _inspector: Res<WorldInspectorState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
) {
    if !params.dev_state.enabled || params.dev_state.active_tab != DevTab::Items {
        return;
    }
    if !params.dev_state.inventory.pile_placement_armed {
        return;
    }
    if panel_hovered.hovered || params.gate.spawn_handled_this_frame {
        return;
    }
    if !params.mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(DefinitionId::Item(item_id)) = params.dev_state.selected_definition.clone() else {
        params.dev_state.inventory.message =
            "Select a stackable item before placing a ground pile".into();
        return;
    };
    let Some(ray) = cursor_world_ray(&windows, &camera) else {
        return;
    };
    let layout = params.config.chunk_layout();
    let vertical_scale = params
        .render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let Some(click) = terrain_click_to_world_position(&ray, &params.world, layout, vertical_scale)
    else {
        params.dev_state.inventory.message = "No terrain under cursor".into();
        return;
    };

    params.gate.block_gameplay_mouse = true;
    params.gate.spawn_handled_this_frame = true;
    let quantity = params.dev_state.inventory.quantity;
    let tick = params.simulation.current_tick;
    match dev_spawn_ground_pile(
        &mut params.world,
        item_id,
        quantity,
        click.world_position,
        tick,
    ) {
        Ok(message) => {
            params.dev_state.inventory.message = message;
            params.dev_state.inventory.pile_placement_armed = false;
        }
        Err(err) => params.dev_state.inventory.message = err.to_string(),
    }
}

fn cycle_endpoint(
    dev_state: &mut DevModeState,
    inspector: &WorldInspectorState,
    world: &WorldData,
    delta: isize,
) {
    let count = resolve_inspector_endpoints(world, inspector).len();
    if count == 0 {
        dev_state.inventory.selected_endpoint_index = 0;
        return;
    }
    let next = (dev_state.inventory.selected_endpoint_index as isize + delta).rem_euclid(count as isize);
    dev_state.inventory.selected_endpoint_index = next as usize;
    dev_state.inventory.selected_entry_index = Some(0);
}

fn cycle_entry(
    dev_state: &mut DevModeState,
    world: &WorldData,
    inspector: &WorldInspectorState,
    delta: isize,
) {
    let Some(endpoint) = selected_endpoint(world, inspector, dev_state) else {
        return;
    };
    let count = entry_count(world, endpoint);
    if count == 0 {
        dev_state.inventory.selected_entry_index = None;
        return;
    }
    let current = dev_state.inventory.selected_entry_index.unwrap_or(0) as isize;
    let next = (current + delta).rem_euclid(count as isize);
    dev_state.inventory.selected_entry_index = Some(next as usize);
}

fn entry_count(world: &WorldData, endpoint: DevInventoryEndpoint) -> usize {
    match endpoint {
        DevInventoryEndpoint::Grid(inventory_id) => world
            .inventory_store()
            .get(inventory_id)
            .map(|record| record.placed_entries().len())
            .unwrap_or(0),
        DevInventoryEndpoint::Pile(_) => 1,
    }
}

fn selected_endpoint(
    world: &WorldData,
    inspector: &WorldInspectorState,
    dev_state: &DevModeState,
) -> Option<DevInventoryEndpoint> {
    let endpoints = resolve_inspector_endpoints(world, inspector);
    if endpoints.is_empty() {
        return None;
    }
    let idx = dev_state
        .inventory
        .selected_endpoint_index
        .min(endpoints.len() - 1);
    Some(endpoints[idx].endpoint)
}

fn run_action(
    dev_state: &mut DevModeState,
    world: &mut WorldData,
    inspector: &WorldInspectorState,
    ctx: &InventoryCatalogCtx<'_>,
    pile_settings: &ItemPileSettings,
    tick: u64,
    action: DevItemsAction,
) {
    let result: Result<String, super::ops::DevInventoryOpError> = (|| {
        match action {
        DevItemsAction::AddItem => {
            let endpoint = selected_endpoint(world, inspector, dev_state)
                .ok_or(super::ops::DevInventoryOpError::NoEndpoint)?;
            let item_id = match dev_state.selected_definition.clone() {
                Some(DefinitionId::Item(item_id)) => item_id,
                _ => return Err(super::ops::DevInventoryOpError::NoItemSelected),
            };
            let position = inspector
                .selected_unit
                .and_then(|id| world.get_unit(id))
                .map(|unit| unit.placement.position)
                .or_else(|| {
                    inspector.selected_building.and_then(|id| {
                        world.get_building(id).map(|record| record.placement.position)
                    })
                })
                .unwrap_or_else(|| {
                    crate::world::WorldPosition::new(
                        crate::world::ChunkCoord::new(0, 0),
                        crate::world::LocalPosition::new(Vec3::ZERO),
                    )
                });
            dev_add_item(
                world,
                ctx,
                endpoint,
                item_id,
                dev_state.inventory.quantity,
                pile_settings,
                position,
                tick,
            )
        }
        DevItemsAction::RemoveEntry => {
            let endpoint = selected_endpoint(world, inspector, dev_state)
                .ok_or(super::ops::DevInventoryOpError::NoEndpoint)?;
            let entry = dev_state
                .inventory
                .selected_entry_index
                .ok_or(super::ops::DevInventoryOpError::NoEntrySelected)?;
            dev_remove_entry(world, ctx, endpoint, entry)
        }
        DevItemsAction::SetQuantity => {
            let endpoint = selected_endpoint(world, inspector, dev_state)
                .ok_or(super::ops::DevInventoryOpError::NoEndpoint)?;
            let entry = dev_state
                .inventory
                .selected_entry_index
                .ok_or(super::ops::DevInventoryOpError::NoEntrySelected)?;
            dev_set_stack_quantity(world, ctx, endpoint, entry, dev_state.inventory.quantity)
        }
        DevItemsAction::ClearInventory => {
            let endpoint = selected_endpoint(world, inspector, dev_state)
                .ok_or(super::ops::DevInventoryOpError::NoEndpoint)?;
            dev_clear_inventory(world, ctx, endpoint)
        }
        DevItemsAction::FillInventory => {
            let endpoint = selected_endpoint(world, inspector, dev_state)
                .ok_or(super::ops::DevInventoryOpError::NoEndpoint)?;
            let item_id = match dev_state.selected_definition.clone() {
                Some(DefinitionId::Item(item_id)) => item_id,
                _ => return Err(super::ops::DevInventoryOpError::NoItemSelected),
            };
            dev_fill_inventory(
                world,
                ctx,
                endpoint,
                item_id,
                dev_state.inventory.quantity,
            )
        }
        DevItemsAction::SetTransferSource => {
            let endpoint = selected_endpoint(world, inspector, dev_state)
                .ok_or(super::ops::DevInventoryOpError::NoEndpoint)?;
            dev_state.inventory.transfer_source = Some(endpoint);
            return Ok(String::new());
        }
        DevItemsAction::SetTransferDest => {
            let endpoint = selected_endpoint(world, inspector, dev_state)
                .ok_or(super::ops::DevInventoryOpError::NoEndpoint)?;
            dev_state.inventory.transfer_dest = Some(endpoint);
            return Ok(String::new());
        }
        DevItemsAction::ExecuteTransfer => {
            let source = dev_state
                .inventory
                .transfer_source
                .ok_or(super::ops::DevInventoryOpError::NoTransferEndpoints)?;
            let dest = dev_state
                .inventory
                .transfer_dest
                .ok_or(super::ops::DevInventoryOpError::NoTransferEndpoints)?;
            let entry = dev_state
                .inventory
                .selected_entry_index
                .unwrap_or(0);
            dev_transfer(
                world,
                ctx,
                pile_settings,
                source,
                dest,
                entry,
                Some(dev_state.inventory.quantity),
                tick,
            )
        }
        DevItemsAction::ArmPilePlacement => {
            dev_state.inventory.pile_placement_armed = true;
            return Ok("Ground pile placement armed".into());
        }
        DevItemsAction::ValidateWorld => {
            let report = crate::world::validate_world_inventory_state(world, ctx);
            if report.is_ok() {
                Ok("World inventory validation OK".into())
            } else {
                Ok(format!("Validation: {report:?}"))
            }
        }
        _ => return Ok(String::new()),
        }
    })();

    if !result.as_ref().map(String::is_empty).unwrap_or(true) {
        match result {
            Ok(message) => dev_state.inventory.message = message,
            Err(err) => dev_state.inventory.message = err.to_string(),
        }
    }
}

/// Bevy system wrapper for item tool keyboard shortcuts.
pub fn handle_dev_items_keyboard_system(
    mut dev_state: ResMut<DevModeState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut world: ResMut<WorldData>,
    inspector: Res<WorldInspectorState>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
    pile_settings: Res<ItemPileSettings>,
    simulation: Res<SimulationControlState>,
) {
    handle_dev_items_keyboard(
        &mut dev_state,
        &keyboard,
        &mut world,
        &inspector,
        &items,
        &categories,
        &profiles,
        &pile_settings,
        &simulation,
    );
}
