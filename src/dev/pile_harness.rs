//! Dev Mode item pile tools (ADR-090 I4).

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::dev::inspector::WorldInspectorState;
use crate::dev::{DevModeState, DevTab};
use crate::simulation::SimulationControlState;
use crate::world::{
    Affiliation, ChunkId, InventoryCatalogCtx, InventoryEntryContents, InventoryProfileCatalog,
    ItemCatalog, ItemCategoryCatalog, ItemDefinitionId, ItemInstanceMetadata, ItemPileSettings,
    ItemPileSource, PileOwnership, SpaceId, TransferPlacementPolicy, WorldData,
    WorldItemPileRecord, create_item_instance, drop_stack_from_inventory,
    drop_unit_inventory_entry, half_stack_quantity, loot_corpse_entry, pickup_pile_into_inventory,
};

fn spawn_stack_pile_at(
    world: &mut WorldData,
    item_definition_id: ItemDefinitionId,
    quantity: u32,
    position: crate::world::WorldPosition,
    tick: u64,
) -> Result<crate::world::ItemPileId, String> {
    let chunk = ChunkId::new(position.chunk);
    let pile_id = world.item_pile_store_mut().allocate_item_pile_id();
    let record = WorldItemPileRecord::new_stack(
        pile_id,
        position,
        SpaceId::SURFACE,
        item_definition_id,
        quantity,
        None,
        None,
        Affiliation::Player,
        ItemPileSource::DevSpawned,
        tick,
    );
    world
        .item_pile_store_mut()
        .insert(chunk, record)
        .map_err(|err| err.to_string())?;
    Ok(pile_id)
}

pub fn format_pile_harness_detail(
    world: &WorldData,
    inspector: &WorldInspectorState,
    message: &str,
) -> String {
    let unit_line = inspector
        .selected_unit
        .map(|id| format!("Selected unit: {id:?}"))
        .unwrap_or_else(|| "Selected unit: none (Alt+click unit)".into());
    let pile_count = world.item_pile_store().sorted_item_pile_ids().len();
    format!(
        "{unit_line}\nPiles in world: {pile_count}\n\
         P=spawn pile · D=drop entry 0 · O=drop one · H=drop half · G=pickup pile · L=loot corpse · V=validate\n\
         {message}"
    )
}

pub fn handle_pile_harness_keyboard(
    mut dev_state: ResMut<DevModeState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut world: ResMut<WorldData>,
    inspector: Res<WorldInspectorState>,
    items: Res<ItemCatalog>,
    categories: Res<ItemCategoryCatalog>,
    profiles: Res<InventoryProfileCatalog>,
    settings: Res<ItemPileSettings>,
    simulation: Res<SimulationControlState>,
) {
    if !dev_state.enabled || dev_state.active_tab != DevTab::WorldTools {
        return;
    }
    if dev_state.has_text_focus() {
        return;
    }

    let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    let mut message = dev_state.pile_harness_message.clone();
    let tick = simulation.current_tick;

    if keyboard.just_pressed(KeyCode::KeyV) {
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let report = crate::world::validate_world_inventory_state(&world, &ctx);
        if report.is_ok() {
            message = format!(
                "World inventory validation OK ({} inventories)",
                world.inventory_store().len()
            );
        } else {
            message = format!("World inventory validation: {report:?}");
        }
    }

    let Some(unit_id) = inspector.selected_unit else {
        dev_state.pile_harness_message = message;
        return;
    };
    let Some(unit) = world.get_unit(unit_id).cloned() else {
        dev_state.pile_harness_message = "Selected unit missing".to_string();
        return;
    };

    if keyboard.just_pressed(KeyCode::KeyP) {
        match spawn_stack_pile_at(
            &mut world,
            ItemDefinitionId::new("gold"),
            5,
            unit.placement.position,
            tick,
        ) {
            Ok(pile_id) => message = format!("Spawned pile `{pile_id}` with gold x5"),
            Err(err) => message = err,
        }
    }

    if keyboard.just_pressed(KeyCode::KeyD) {
        match drop_unit_inventory_entry(&mut world, &ctx, &settings, unit_id, 0, None, tick) {
            Ok(report) => {
                message = format!(
                    "Dropped {} (merged {}, new piles {:?})",
                    report.removed_from_inventory,
                    report.merged_into_existing_piles,
                    report.created_pile_ids
                );
            }
            Err(err) => message = err.to_string(),
        }
    }

    if keyboard.just_pressed(KeyCode::KeyO) {
        match drop_unit_inventory_entry(&mut world, &ctx, &settings, unit_id, 0, Some(1), tick) {
            Ok(report) => message = format!("Dropped one: {report:?}"),
            Err(err) => message = err.to_string(),
        }
    }

    if keyboard.just_pressed(KeyCode::KeyH) {
        let inventory_id = match unit.inventory_id {
            Some(id) => id,
            None => {
                dev_state.pile_harness_message = "Unit has no inventory".to_string();
                return;
            }
        };
        let entry = world
            .inventory_store()
            .get(inventory_id)
            .and_then(|record| record.placed_entries().first())
            .map(|entry| entry.contents.clone());
        let Some(InventoryEntryContents::Stack { quantity, .. }) = entry else {
            dev_state.pile_harness_message = "Entry 0 is not a stack".to_string();
            return;
        };
        let drop_qty = half_stack_quantity(quantity);
        match drop_stack_from_inventory(
            &mut world,
            &ctx,
            &settings,
            inventory_id,
            0,
            drop_qty,
            unit.placement.position,
            unit.current_space_id,
            PileOwnership::from_unit(&unit),
            tick,
        ) {
            Ok(report) => message = format!("Dropped half ({drop_qty}): {report:?}"),
            Err(err) => message = err.to_string(),
        }
    }

    if keyboard.just_pressed(KeyCode::KeyG) {
        let pile_id = world
            .item_pile_store()
            .sorted_item_pile_ids()
            .first()
            .copied();
        let Some(pile_id) = pile_id else {
            dev_state.pile_harness_message = "No piles to pick up".to_string();
            return;
        };
        let Some(inventory_id) = unit.inventory_id else {
            dev_state.pile_harness_message = "Unit has no inventory".to_string();
            return;
        };
        match pickup_pile_into_inventory(
            &mut world,
            &ctx,
            pile_id,
            inventory_id,
            None,
            unit.owner_id,
            unit.team_id,
            unit.affiliation,
        ) {
            Ok(report) => message = format!("Pickup: {report:?}"),
            Err(err) => message = err.to_string(),
        }
    }

    if keyboard.just_pressed(KeyCode::KeyL) {
        let corpse = world
            .corpse_store()
            .sorted_corpse_ids()
            .into_iter()
            .filter_map(|id| world.corpse_store().get(id).cloned())
            .find(|corpse| corpse.inventory_id.is_some());
        let Some(corpse) = corpse else {
            dev_state.pile_harness_message = "No lootable corpse".to_string();
            return;
        };
        let corpse_inventory = corpse.inventory_id.unwrap();
        let Some(unit_inventory) = unit.inventory_id else {
            dev_state.pile_harness_message = "Unit has no inventory".to_string();
            return;
        };
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        match loot_corpse_entry(
            inventory_store,
            instance_store,
            &ctx,
            corpse_inventory,
            0,
            unit_inventory,
            None,
            TransferPlacementPolicy::MergeThenFirstFit,
        ) {
            Ok(report) => message = format!("Looted: {report:?}"),
            Err(err) => message = err.to_string(),
        }
    }

    dev_state.pile_harness_message = message;
}
