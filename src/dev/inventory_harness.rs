//! Dev Mode inventory test harness (ADR-088 I2).

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::dev::{DefinitionId, DevModeState, DevTab, ItemsBrowserSubtab};
use crate::world::{
    InventoryCatalogCtx, InventoryOwnerRef, InventoryProfileCatalog, InventoryProfileId,
    ItemCatalog, ItemCategoryCatalog, ItemDefinitionId, ItemInstanceMetadata, WorldData, auto_sort,
    create_inventory, create_item_instance, merge_stacks, place_stack_first_fit,
    place_unique_first_fit, query_inventory_weight, remove_inventory, split_stack_half,
};

pub fn format_inventory_harness_detail(
    world: &WorldData,
    items: &ItemCatalog,
    categories: &ItemCategoryCatalog,
    profiles: &InventoryProfileCatalog,
    harness_id: Option<crate::world::InventoryId>,
) -> String {
    let Some(inventory_id) = harness_id else {
        return "Inventory harness — N=new detached inventory (select profile) · A=add stack · U=add unique · O=auto-sort · V=validate · Del=delete".to_string();
    };
    let Some(record) = world.inventory_store().get(inventory_id) else {
        return format!("Harness inventory #{inventory_id:?} missing");
    };
    let ctx = InventoryCatalogCtx::new(items, categories, profiles);
    let weight = query_inventory_weight(record, &ctx)
        .map(|query| {
            format!(
                "mass={}g ref={:?} over={}g",
                query.total_mass_grams, query.reference_weight_grams, query.over_reference_grams
            )
        })
        .unwrap_or_else(|err| format!("weight error: {err}"));
    let mut lines = vec![
        format!("Detached inventory #{inventory_id:?}"),
        format!(
            "profile={} grid={}x{} entries={}",
            record.profile_id().as_str(),
            record.grid_width(),
            record.grid_height(),
            record.placed_entries().len()
        ),
        weight,
        "occupancy:".to_string(),
    ];
    for y in 0..record.grid_height() {
        let mut row = String::new();
        for x in 0..record.grid_width() {
            let cell = if record.entry_at_cell(x, y).is_some() {
                '#'
            } else {
                '.'
            };
            row.push(cell);
        }
        lines.push(row);
    }
    lines.join("\n")
}

pub fn handle_inventory_harness_input(
    dev_state: &mut DevModeState,
    keyboard: &ButtonInput<KeyCode>,
    world: &mut WorldData,
    items: &ItemCatalog,
    categories: &ItemCategoryCatalog,
    profiles: &InventoryProfileCatalog,
) {
    if !dev_state.enabled || dev_state.active_tab != DevTab::Items {
        return;
    }
    if dev_state.items_browser_subtab != ItemsBrowserSubtab::InventoryHarness {
        return;
    }
    if dev_state.has_text_focus() {
        return;
    }

    let ctx = InventoryCatalogCtx::new(items, categories, profiles);

    if keyboard.just_pressed(KeyCode::KeyN) {
        let profile_id = match &dev_state.selected_definition {
            Some(DefinitionId::InventoryProfile(id)) => id.clone(),
            _ => InventoryProfileId::new("unit_backpack_standard"),
        };
        match create_inventory(
            world.inventory_store_mut(),
            &ctx,
            profile_id.clone(),
            InventoryOwnerRef::Detached,
        ) {
            Ok(id) => {
                dev_state.inventory_harness_id = Some(id);
                dev_state.inventory_harness_message = format!(
                    "Created detached inventory #{id:?} profile `{}`",
                    profile_id.as_str()
                );
            }
            Err(err) => dev_state.inventory_harness_message = err.to_string(),
        }
    }

    let Some(inventory_id) = dev_state.inventory_harness_id else {
        return;
    };

    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        match remove_inventory(inventory_store, instance_store, inventory_id) {
            Ok(_) => {
                dev_state.inventory_harness_id = None;
                dev_state.inventory_harness_message =
                    format!("Deleted detached inventory #{inventory_id:?}");
            }
            Err(err) => dev_state.inventory_harness_message = err.to_string(),
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyA) {
        let Some(DefinitionId::Item(item_id)) = dev_state.selected_definition.clone() else {
            dev_state.inventory_harness_message =
                "Select a stackable item in the catalog list first".to_string();
            return;
        };
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        match place_stack_first_fit(
            inventory_store,
            instance_store,
            &ctx,
            inventory_id,
            item_id,
            5,
        ) {
            Ok(index) => {
                dev_state.inventory_harness_message = format!("Placed stack at entry {index}");
            }
            Err(err) => dev_state.inventory_harness_message = err.to_string(),
        }
    }

    if keyboard.just_pressed(KeyCode::KeyU) {
        let Some(DefinitionId::Item(item_id)) = dev_state.selected_definition.clone() else {
            dev_state.inventory_harness_message =
                "Select a unique item in the catalog list first".to_string();
            return;
        };
        let instance_id = match create_item_instance(
            world.item_instance_store_mut(),
            &ctx,
            item_id,
            ItemInstanceMetadata::default(),
        ) {
            Ok(instance_id) => instance_id,
            Err(err) => {
                dev_state.inventory_harness_message = err.to_string();
                return;
            }
        };
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        match place_unique_first_fit(
            inventory_store,
            instance_store,
            &ctx,
            inventory_id,
            instance_id,
        ) {
            Ok(index) => {
                dev_state.inventory_harness_message =
                    format!("Placed unique `{instance_id:?}` at entry {index}");
            }
            Err(err) => dev_state.inventory_harness_message = err.to_string(),
        }
    }

    if keyboard.just_pressed(KeyCode::KeyO) {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        match auto_sort(inventory_store, instance_store, &ctx, inventory_id) {
            Ok(()) => dev_state.inventory_harness_message = "Auto-sorted".to_string(),
            Err(err) => dev_state.inventory_harness_message = err.to_string(),
        }
    }

    if keyboard.just_pressed(KeyCode::KeyV) {
        let report = crate::world::validate_world_inventory_state(&world, &ctx);
        if report.is_ok() {
            dev_state.inventory_harness_message = "World inventory validation OK".to_string();
        } else {
            dev_state.inventory_harness_message = format!("Validation: {report:?}");
        }
    }

    if keyboard.just_pressed(KeyCode::KeyH) {
        let entry_count = world
            .inventory_store()
            .get(inventory_id)
            .map(|record| record.placed_entries().len())
            .unwrap_or(0);
        if entry_count == 0 {
            dev_state.inventory_harness_message = "No entries to split".to_string();
            return;
        }
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        match split_stack_half(inventory_store, instance_store, &ctx, inventory_id, 0) {
            Ok(outcome) => {
                dev_state.inventory_harness_message = format!(
                    "Split half moved {} (remaining {})",
                    outcome.moved, outcome.source_remaining
                );
            }
            Err(err) => dev_state.inventory_harness_message = err.to_string(),
        }
    }

    if keyboard.just_pressed(KeyCode::KeyM) {
        let entry_count = world
            .inventory_store()
            .get(inventory_id)
            .map(|record| record.placed_entries().len())
            .unwrap_or(0);
        if entry_count < 2 {
            dev_state.inventory_harness_message = "Need two stack entries to merge".to_string();
            return;
        }
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        match merge_stacks(inventory_store, instance_store, &ctx, inventory_id, 1, 0) {
            Ok(outcome) => {
                dev_state.inventory_harness_message = format!(
                    "Merged {} (source remaining {})",
                    outcome.merged, outcome.remaining_in_source
                );
            }
            Err(err) => dev_state.inventory_harness_message = err.to_string(),
        }
    }
}

/// Bevy system wrapper for harness keyboard shortcuts.
pub fn handle_inventory_harness_keyboard(
    mut dev_state: ResMut<DevModeState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut world: ResMut<WorldData>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
) {
    handle_inventory_harness_input(
        &mut dev_state,
        &keyboard,
        &mut world,
        &items,
        &categories,
        &profiles,
    );
}
