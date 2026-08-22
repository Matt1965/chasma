//! Dev inventory tool input — ground pile placement (DV0 / Slice 12 UI-only).

use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::client::selection::{
    ApplyWorldSelectionParams, WorldSelectionCategory, WorldSelectionChange,
    WorldSelectionRevision, WorldSelectionState, apply_world_selection,
};
use crate::dev::dev_mode::DevInventoryEndpoint;
use crate::dev::dev_mode::{DefinitionId, DevModeState, DevTab, DevTextFieldFocus};
use crate::dev::inspector::WorldInspectorState;
use crate::dev::inventory_tools::endpoint::{
    building_inventory_endpoint, resolve_active_endpoint, resolve_inspector_endpoints,
    resolve_target_building, resolve_target_unit, unit_inventory_endpoint,
};
use crate::dev::inventory_tools::ops::{
    dev_add_item, dev_remove_entry, dev_spawn_ground_pile, ensure_dev_unit_inventory,
};
use crate::dev::{DevPanelHoverState, input::DevSpawnClickParams};
use crate::simulation::SimulationControlState;
use crate::ui::gameplay::GameplayBuildingSelection;
use crate::units::input::{SelectedUnits, cursor_world_ray, terrain_click_to_world_position};
use crate::world::{
    InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog,
    ItemDefinitionId, ItemPileSettings, UnitCatalog, WorldConfig, WorldData,
};

use super::panel::DevItemsAction;

pub fn handle_dev_items_panel_action(
    dev_state: &mut DevModeState,
    world: &mut WorldData,
    world_selection: &WorldSelectionState,
    building_selection: &GameplayBuildingSelection,
    selection: &SelectedUnits,
    unit_catalog: &UnitCatalog,
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
        DevItemsAction::CycleEndpoint => cycle_endpoint(
            dev_state,
            world_selection,
            building_selection,
            selection,
            world,
            1,
        ),
        DevItemsAction::CycleEntry => cycle_entry(
            dev_state,
            world,
            world_selection,
            building_selection,
            selection,
            1,
        ),
        other => run_action(
            dev_state,
            world,
            world_selection,
            building_selection,
            selection,
            unit_catalog,
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
    mut world_selection: ResMut<WorldSelectionState>,
    mut selected_units: ResMut<SelectedUnits>,
    mut building_selection: ResMut<crate::ui::gameplay::GameplayBuildingSelection>,
    mut selection_revision: ResMut<WorldSelectionRevision>,
    mut inspector: ResMut<WorldInspectorState>,
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
    params.dev_state.apply_item_quantity_input();
    let quantity = params.dev_state.inventory.quantity;
    let tick = params.simulation.current_tick;
    match dev_spawn_ground_pile(
        &mut params.world,
        item_id.clone(),
        quantity,
        click.world_position,
        tick,
    ) {
        Ok(pile_id) => {
            let message = format!("Spawned ground pile #{pile_id:?} x{quantity}");
            params.dev_state.inventory.message = message.clone();
            params.dev_state.last_spawn_message = message;
            apply_world_selection(
                WorldSelectionChange::SelectItemPile { pile_id },
                &mut ApplyWorldSelectionParams {
                    world_selection: &mut world_selection,
                    selected_units: &mut selected_units,
                    building_selection: &mut building_selection,
                    hud: None,
                    revision: Some(&mut selection_revision),
                },
            );
            inspector.last_message = format!("Spawned ground pile #{pile_id:?}");
            params.dev_state.inventory.pile_placement_armed = false;
        }
        Err(err) => {
            params.dev_state.inventory.message = err.to_string();
        }
    }
}

fn cycle_endpoint(
    dev_state: &mut DevModeState,
    world_selection: &WorldSelectionState,
    building_selection: &GameplayBuildingSelection,
    selection: &SelectedUnits,
    world: &WorldData,
    delta: isize,
) {
    let count =
        resolve_inspector_endpoints(world, world_selection, selection, building_selection).len();
    if count == 0 {
        dev_state.inventory.selected_endpoint_index = 0;
        return;
    }
    let next =
        (dev_state.inventory.selected_endpoint_index as isize + delta).rem_euclid(count as isize);
    dev_state.inventory.selected_endpoint_index = next as usize;
    dev_state.inventory.selected_entry_index = Some(0);
}

fn cycle_entry(
    dev_state: &mut DevModeState,
    world: &WorldData,
    world_selection: &WorldSelectionState,
    building_selection: &GameplayBuildingSelection,
    selection: &SelectedUnits,
    delta: isize,
) {
    let Some(endpoint) = resolve_active_endpoint(
        world,
        world_selection,
        selection,
        building_selection,
        &dev_state.inventory,
    ) else {
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

fn run_action(
    dev_state: &mut DevModeState,
    world: &mut WorldData,
    world_selection: &WorldSelectionState,
    building_selection: &GameplayBuildingSelection,
    selection: &SelectedUnits,
    unit_catalog: &UnitCatalog,
    ctx: &InventoryCatalogCtx<'_>,
    pile_settings: &ItemPileSettings,
    tick: u64,
    action: DevItemsAction,
) {
    let result: Result<String, super::ops::DevInventoryOpError> = (|| match action {
        DevItemsAction::AddToUnit => {
            let unit_id = resolve_target_unit(world_selection, selection)
                .ok_or(super::ops::DevInventoryOpError::NoUnitSelected)?;
            let quantity = effective_item_quantity(dev_state);
            ensure_dev_unit_inventory(world, unit_catalog, ctx, unit_id)?;
            let grid = unit_inventory_endpoint(world, unit_id).ok_or(
                super::ops::DevInventoryOpError::Message(
                    "unit inventory missing after attach".into(),
                ),
            )?;
            let item_id = selected_item_id(dev_state)?;
            let position = world
                .get_unit(unit_id)
                .map(|unit| unit.placement.position)
                .unwrap_or_else(default_world_position);
            let item_name = ctx
                .item(&item_id)
                .map(|item| item.display_name.clone())
                .unwrap_or_else(|| item_id.as_str().to_string());
            dev_add_item(
                world,
                ctx,
                grid,
                item_id,
                quantity,
                pile_settings,
                position,
                tick,
            )
            .map(|_| format!("Added {item_name} x{quantity} to unit #{}", unit_id.raw()))
        }
        DevItemsAction::AddToContainer => {
            let building_id = resolve_target_building(world_selection, building_selection)
                .ok_or(super::ops::DevInventoryOpError::NoContainerSelected)?;
            let grid = building_inventory_endpoint(world, building_id)
                .ok_or(super::ops::DevInventoryOpError::ContainerHasNoInventory)?;
            let item_id = selected_item_id(dev_state)?;
            let quantity = effective_item_quantity(dev_state);
            let position = world
                .get_building(building_id)
                .map(|record| record.placement.position)
                .unwrap_or_else(default_world_position);
            let item_name = ctx
                .item(&item_id)
                .map(|item| item.display_name.clone())
                .unwrap_or_else(|| item_id.as_str().to_string());
            dev_add_item(
                world,
                ctx,
                grid,
                item_id,
                quantity,
                pile_settings,
                position,
                tick,
            )
            .map(|_| {
                format!(
                    "Added {item_name} x{quantity} to building #{}",
                    building_id.raw()
                )
            })
        }
        DevItemsAction::RemoveEntry => {
            let endpoint = active_endpoint_or_err(
                world,
                world_selection,
                selection,
                building_selection,
                dev_state,
            )?;
            let entry = dev_state
                .inventory
                .selected_entry_index
                .ok_or(super::ops::DevInventoryOpError::NoEntrySelected)?;
            dev_remove_entry(world, ctx, endpoint, entry)
        }
        DevItemsAction::ArmPilePlacement => {
            dev_state.inventory.pile_placement_armed = true;
            Ok("Ground pile placement armed".into())
        }
        _ => unreachable!("handled before run_action"),
    })();

    match result {
        Ok(message) if !message.is_empty() => dev_state.inventory.message = message,
        Ok(_) => {}
        Err(err) => dev_state.inventory.message = err.to_string(),
    }
}

fn effective_item_quantity(dev_state: &DevModeState) -> u32 {
    if dev_state.text_focus == DevTextFieldFocus::ItemQuantity {
        dev_state
            .inventory
            .quantity_input
            .parse::<u32>()
            .unwrap_or(dev_state.inventory.quantity)
            .clamp(1, 10_000)
    } else {
        dev_state.inventory.quantity
    }
}

fn selected_item_id(
    dev_state: &DevModeState,
) -> Result<ItemDefinitionId, super::ops::DevInventoryOpError> {
    match dev_state.selected_definition.clone() {
        Some(DefinitionId::Item(item_id)) => Ok(item_id),
        _ => Err(super::ops::DevInventoryOpError::NoItemSelected),
    }
}

fn active_endpoint_or_err(
    world: &WorldData,
    world_selection: &WorldSelectionState,
    selection: &SelectedUnits,
    building_selection: &GameplayBuildingSelection,
    dev_state: &DevModeState,
) -> Result<DevInventoryEndpoint, super::ops::DevInventoryOpError> {
    resolve_active_endpoint(
        world,
        world_selection,
        selection,
        building_selection,
        &dev_state.inventory,
    )
    .ok_or(super::ops::DevInventoryOpError::NoEndpoint)
}

fn default_world_position() -> crate::world::WorldPosition {
    crate::world::WorldPosition::new(
        crate::world::ChunkCoord::new(0, 0),
        crate::world::LocalPosition::new(Vec3::ZERO),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::selection::WorldSelectionState;
    use crate::dev::dev_mode::{DefinitionId, DevModeState, DevTab};
    use crate::simulation::SimulationControlState;
    use crate::units::input::SelectedUnits;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, Heightfield, InventoryEntryContents, ItemDefinitionId,
        LocalPosition, UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource, WorldPosition,
        create_unit_with_ownership,
    };
    use bevy::prelude::Vec3;

    fn test_world() -> WorldData {
        let mut world = WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn spawn_wolf(world: &mut WorldData) -> crate::world::UnitId {
        let unit_catalog = UnitCatalog::default();
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(10.0, 0.0, 10.0)),
        );
        create_unit_with_ownership(
            &unit_catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            position,
            UnitSource::Dev,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id
    }

    fn gold_quantity(world: &WorldData, inventory_id: crate::world::InventoryId) -> u32 {
        let gold = ItemDefinitionId::new("gold");
        world
            .inventory_store()
            .get(inventory_id)
            .unwrap()
            .placed_entries()
            .iter()
            .filter_map(|entry| match &entry.contents {
                InventoryEntryContents::Stack {
                    item_definition_id,
                    quantity,
                } if item_definition_id == &gold => Some(*quantity),
                _ => None,
            })
            .sum()
    }

    #[test]
    fn add_to_unit_panel_action_mutates_selected_unit_inventory() {
        let mut world = test_world();
        let unit_id = spawn_wolf(&mut world);
        let mut dev_state = DevModeState::default();
        dev_state.enabled = true;
        dev_state.active_tab = DevTab::Items;
        dev_state.inventory.quantity = 25;
        dev_state.select_definition(DefinitionId::Item(ItemDefinitionId::new("gold")));

        let mut selection = SelectedUnits::default();
        selection.set_single(unit_id);
        let world_selection = WorldSelectionState::default();
        let building_selection = GameplayBuildingSelection::default();
        let unit_catalog = UnitCatalog::default();
        let items = ItemCatalog::default();
        let categories = ItemCategoryCatalog::default();
        let profiles = InventoryProfileCatalog::default();
        let pile_settings = ItemPileSettings::default();
        let simulation = SimulationControlState::default();

        handle_dev_items_panel_action(
            &mut dev_state,
            &mut world,
            &world_selection,
            &building_selection,
            &selection,
            &unit_catalog,
            &items,
            &categories,
            &profiles,
            &pile_settings,
            &simulation,
            DevItemsAction::AddToUnit,
        );

        assert!(
            dev_state.inventory.message.contains("Added"),
            "expected success feedback, got `{}`",
            dev_state.inventory.message
        );
        let inventory_id = world.get_unit(unit_id).unwrap().inventory_id.unwrap();
        assert_eq!(gold_quantity(&world, inventory_id), 25);
    }

    #[test]
    fn add_to_unit_panel_action_surfaces_no_item_selected() {
        let mut world = test_world();
        let unit_id = spawn_wolf(&mut world);
        let mut dev_state = DevModeState::default();
        dev_state.enabled = true;
        dev_state.active_tab = DevTab::Items;
        dev_state.select_definition(DefinitionId::Unit(UnitDefinitionId::new("wolf")));

        let mut selection = SelectedUnits::default();
        selection.set_single(unit_id);

        handle_dev_items_panel_action(
            &mut dev_state,
            &mut world,
            &WorldSelectionState::default(),
            &GameplayBuildingSelection::default(),
            &selection,
            &UnitCatalog::default(),
            &ItemCatalog::default(),
            &ItemCategoryCatalog::default(),
            &InventoryProfileCatalog::default(),
            &ItemPileSettings::default(),
            &SimulationControlState::default(),
            DevItemsAction::AddToUnit,
        );

        assert_eq!(
            dev_state.inventory.message,
            crate::dev::inventory_tools::ops::DevInventoryOpError::NoItemSelected.to_string()
        );
        let unit = world.get_unit(unit_id).unwrap();
        let gold = unit
            .inventory_id
            .map(|id| gold_quantity(&world, id))
            .unwrap_or(0);
        assert_eq!(gold, 0);
    }

    #[test]
    fn add_to_unit_uses_uncommitted_quantity_input() {
        let mut world = test_world();
        let unit_id = spawn_wolf(&mut world);
        let mut dev_state = DevModeState::default();
        dev_state.enabled = true;
        dev_state.active_tab = DevTab::Items;
        dev_state.inventory.quantity = 10;
        dev_state.focus_item_quantity();
        dev_state.inventory.quantity_input = "25".into();
        dev_state.select_definition(DefinitionId::Item(ItemDefinitionId::new("gold")));

        let mut selection = SelectedUnits::default();
        selection.set_single(unit_id);

        handle_dev_items_panel_action(
            &mut dev_state,
            &mut world,
            &WorldSelectionState::default(),
            &GameplayBuildingSelection::default(),
            &selection,
            &UnitCatalog::default(),
            &ItemCatalog::default(),
            &ItemCategoryCatalog::default(),
            &InventoryProfileCatalog::default(),
            &ItemPileSettings::default(),
            &SimulationControlState::default(),
            DevItemsAction::AddToUnit,
        );

        let inventory_id = world.get_unit(unit_id).unwrap().inventory_id.unwrap();
        assert_eq!(gold_quantity(&world, inventory_id), 25);
    }
}
