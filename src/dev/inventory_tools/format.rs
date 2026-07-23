//! Dev inventory panel text formatting (DV0).

use crate::dev::dev_mode::{DefinitionId, DevInventoryToolState, ItemsBrowserSubtab};
use crate::dev::DevInventoryEndpoint;
use crate::dev::inspector::WorldInspectorState;
use crate::dev::inventory_tools::endpoint::{
    resolve_inspector_endpoints, resolve_target_unit, DevInventoryEndpointInfo,
};
use crate::units::input::SelectedUnits;
use crate::world::{
    InventoryCatalogCtx, ItemInstanceStore, PlacedInventoryEntry, ItemCatalog, ItemCategoryCatalog,
    WorldData, WorldPileContents, InventoryEntryContents,
};

pub fn format_inventory_tool_panel(
    world: &WorldData,
    inspector: &WorldInspectorState,
    selection: &SelectedUnits,
    items: &ItemCatalog,
    categories: &ItemCategoryCatalog,
    ctx: &InventoryCatalogCtx<'_>,
    instance_store: &ItemInstanceStore,
    tool: &DevInventoryToolState,
    selected_item: Option<&DefinitionId>,
) -> String {
    let endpoints = resolve_inspector_endpoints(world, inspector, selection);
    let mut lines = vec![
        format!("Subtab: {:?}", tool.subtab),
        tool.message.clone(),
        String::new(),
    ];

    if endpoints.is_empty() {
        if let Some(unit_id) = resolve_target_unit(inspector, selection) {
            if world.get_unit(unit_id).is_some() {
                lines.push(format!(
                    "Target: Unit #{} has no inventory — Add attaches `unit_backpack_standard`",
                    unit_id.raw()
                ));
            } else {
                lines.push("Target: none — select a unit or inspect unit/building/pile".into());
            }
        } else {
            lines.push("Target: none — select a unit or inspect unit/building/pile".into());
        }
    } else {
        let idx = tool.selected_endpoint_index.min(endpoints.len().saturating_sub(1));
        lines.push(format!(
            "Target [{}/{}]: {}",
            idx + 1,
            endpoints.len(),
            endpoints[idx].label
        ));
        lines.extend(format_endpoint_contents(
            world,
            items,
            categories,
            ctx,
            instance_store,
            &endpoints[idx],
            tool,
        ));
    }

    if let Some(DefinitionId::Item(item_id)) = selected_item {
        if let Some(item) = items.get(item_id) {
            lines.push(String::new());
            lines.push(format!(
                "Selected item: {} (`{}`) — max stack {}",
                item.display_name,
                item.id.as_str(),
                item.max_stack
            ));
        }
    }

    if let (Some(src), Some(dst)) = (&tool.transfer_source, &tool.transfer_dest) {
        lines.push(String::new());
        lines.push(format!("Transfer: {src:?} → {dst:?}"));
    } else if tool.transfer_source.is_some() {
        lines.push("Transfer: source set — pick destination (Dst)".into());
    }

    if tool.pile_placement_armed {
        lines.push(String::new());
        lines.push("Ground pile placement armed — left-click terrain".into());
    }

    lines.join("\n")
}

fn format_endpoint_contents(
    world: &WorldData,
    items: &ItemCatalog,
    categories: &ItemCategoryCatalog,
    ctx: &InventoryCatalogCtx<'_>,
    instance_store: &ItemInstanceStore,
    endpoint: &DevInventoryEndpointInfo,
    tool: &DevInventoryToolState,
) -> Vec<String> {
    match endpoint.endpoint {
        DevInventoryEndpoint::Grid(inventory_id) => {
            let Some(record) = world.inventory_store().get(inventory_id) else {
                return vec!["(inventory missing)".into()];
            };
            let mut lines = vec![format!(
                "Grid {}x{} profile `{}`",
                record.grid_width(),
                record.grid_height(),
                record.profile_id().as_str()
            )];
            if record.placed_entries().is_empty() {
                lines.push("  (empty)".into());
                return lines;
            }
            for (index, entry) in record.placed_entries().iter().enumerate() {
                let marker = if tool.selected_entry_index == Some(index) {
                    ">"
                } else {
                    " "
                };
                let label = format_entry_label(entry, items, ctx, instance_store);
                lines.push(format!("{marker} [{index}] {label} @({}, {})", entry.anchor_x, entry.anchor_y));
            }
            lines
        }
        DevInventoryEndpoint::Pile(pile_id) => {
            let Some(pile) = world.item_pile_store().get(pile_id) else {
                return vec!["(pile missing)".into()];
            };
            let label = match &pile.contents {
                WorldPileContents::Stack {
                    item_definition_id,
                    quantity,
                } => {
                    let name = items
                        .get(item_definition_id)
                        .map(|item| item.display_name.as_str())
                        .unwrap_or(item_definition_id.as_str());
                    format!("{name} x{quantity}")
                }
                WorldPileContents::Unique { item_instance_id } => {
                    format!("unique `{item_instance_id:?}`")
                }
            };
            vec![format!("  [0] {label}")]
        }
    }
}

fn format_entry_label(
    entry: &PlacedInventoryEntry,
    items: &ItemCatalog,
    ctx: &InventoryCatalogCtx<'_>,
    instance_store: &ItemInstanceStore,
) -> String {
    match &entry.contents {
        InventoryEntryContents::Stack {
            item_definition_id,
            quantity,
        } => {
            let name = items
                .get(item_definition_id)
                .map(|item| item.display_name.as_str())
                .unwrap_or(item_definition_id.as_str());
            format!("{name} x{quantity}")
        }
        InventoryEntryContents::Unique { item_instance_id } => {
            if let Some(instance) = instance_store.get(*item_instance_id) {
                let name = items
                    .get(&instance.definition_id)
                    .map(|item| item.display_name.as_str())
                    .unwrap_or(instance.definition_id.as_str());
                format!("{name} (unique)")
            } else {
                format!("unique `{item_instance_id:?}`")
            }
        }
    }
}
