//! Client-side inventory drag target prediction (Slice 10).

use crate::client::inventory_intent::entry_revision_for_inventory;
use crate::ui::gameplay::inventory::errors::InventoryUiError;
use crate::ui::gameplay::inventory::state::{InventoryDragState, InventoryUiState};
use crate::world::{
    BuildingCatalog, BuildingInteractionProfileCatalog, EntryIndex, InventoryAccessResult,
    InventoryId, ItemDefinitionId, WorldData, can_place_footprint, can_unit_access_inventory,
};

/// Grid cell size in inventory UI pixels (must match panel layout).
pub const INVENTORY_CELL_PX: f32 = 28.0;

/// Where the dragged item would land if released now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryDropTarget {
    None,
    GridCell {
        inventory_id: InventoryId,
        anchor_x: u8,
        anchor_y: u8,
    },
    GroundDrop,
}

/// Client-local placement preview — not authoritative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryPlacementPreview {
    pub target: InventoryDropTarget,
    pub valid: bool,
    pub reason: Option<InventoryUiError>,
}

impl Default for InventoryPlacementPreview {
    fn default() -> Self {
        Self {
            target: InventoryDropTarget::None,
            valid: false,
            reason: None,
        }
    }
}

/// Build drag payload from authoritative inventory entry data.
pub fn drag_state_from_entry(
    world: &WorldData,
    items: &crate::world::ItemCatalog,
    instance_store: &crate::world::ItemInstanceStore,
    inventory_id: InventoryId,
    entry_index: usize,
    revision: u64,
) -> Option<InventoryDragState> {
    let record = world.inventory_store().get(inventory_id)?;
    let entry = record.placed_entries().get(entry_index)?;
    let (item_definition_id, quantity) = match &entry.contents {
        crate::world::InventoryEntryContents::Stack {
            item_definition_id,
            quantity,
        } => (item_definition_id.clone(), *quantity),
        crate::world::InventoryEntryContents::Unique { item_instance_id } => {
            let def = instance_store
                .get(*item_instance_id)
                .map(|i| i.definition_id.clone())
                .unwrap_or_else(|| ItemDefinitionId::new("unknown"));
            (def, 1)
        }
    };
    let (grid_width, grid_height) = items
        .get(&item_definition_id)
        .map(|d| (d.grid_width, d.grid_height))
        .unwrap_or((1, 1));
    Some(InventoryDragState {
        source_inventory_id: inventory_id,
        entry_index,
        entry_revision: revision,
        item_definition_id,
        grid_width,
        grid_height,
        quantity,
    })
}

/// Evaluate whether the current drop target is valid for the active drag.
pub fn evaluate_drop_target(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    ui: &InventoryUiState,
    drag: &InventoryDragState,
    target: InventoryDropTarget,
) -> InventoryPlacementPreview {
    if !revision_matches(world, drag) {
        return InventoryPlacementPreview {
            target,
            valid: false,
            reason: Some(InventoryUiError::ItemChanged),
        };
    }

    match target {
        InventoryDropTarget::None => InventoryPlacementPreview {
            target,
            valid: false,
            reason: None,
        },
        InventoryDropTarget::GridCell {
            inventory_id,
            anchor_x,
            anchor_y,
        } => evaluate_grid_target(
            world,
            building_catalog,
            interaction_catalog,
            ui,
            drag,
            inventory_id,
            anchor_x,
            anchor_y,
        ),
        InventoryDropTarget::GroundDrop => evaluate_ground_drop(world, ui, drag),
    }
}

fn evaluate_grid_target(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    ui: &InventoryUiState,
    drag: &InventoryDragState,
    inventory_id: InventoryId,
    anchor_x: u8,
    anchor_y: u8,
) -> InventoryPlacementPreview {
    let target = InventoryDropTarget::GridCell {
        inventory_id,
        anchor_x,
        anchor_y,
    };

    if let Some(actor) = ui.actor_unit_id {
        if drag.source_inventory_id != inventory_id
            && let Some(error) = inventory_access_error(
                world,
                building_catalog,
                interaction_catalog,
                actor,
                drag.source_inventory_id,
                ui,
            )
        {
            return invalid(target, error);
        }
        if let Some(error) = inventory_access_error(
            world,
            building_catalog,
            interaction_catalog,
            actor,
            inventory_id,
            ui,
        ) {
            return invalid(target, error);
        }
    }

    let Some(record) = world.inventory_store().get(inventory_id) else {
        return invalid(target, InventoryUiError::InventoryClosed);
    };

    let exclude = if drag.source_inventory_id == inventory_id {
        Some(drag.entry_index as EntryIndex)
    } else {
        None
    };

    if !can_place_footprint(
        record,
        anchor_x,
        anchor_y,
        drag.grid_width,
        drag.grid_height,
        exclude,
    ) {
        return invalid(target, InventoryUiError::InvalidTargetCell);
    }

    InventoryPlacementPreview {
        target,
        valid: true,
        reason: None,
    }
}

fn evaluate_ground_drop(
    world: &WorldData,
    ui: &InventoryUiState,
    drag: &InventoryDragState,
) -> InventoryPlacementPreview {
    let target = InventoryDropTarget::GroundDrop;

    if ui.treasury_id.is_some() {
        return invalid(target, InventoryUiError::InvalidTargetCell);
    }

    let Some(actor) = ui.actor_unit_id else {
        return invalid(target, InventoryUiError::AccessDenied);
    };

    let Some(unit) = world.get_unit(actor) else {
        return invalid(target, InventoryUiError::AccessDenied);
    };

    let Some(unit_inventory) = unit.inventory_id else {
        return invalid(target, InventoryUiError::UnitHasNoInventory);
    };

    if drag.source_inventory_id != unit_inventory {
        return invalid(target, InventoryUiError::InvalidTargetCell);
    }

    InventoryPlacementPreview {
        target,
        valid: true,
        reason: None,
    }
}

fn revision_matches(world: &WorldData, drag: &InventoryDragState) -> bool {
    entry_revision_for_inventory(world, drag.source_inventory_id, drag.entry_index)
        == drag.entry_revision
}

fn invalid(target: InventoryDropTarget, reason: InventoryUiError) -> InventoryPlacementPreview {
    InventoryPlacementPreview {
        target,
        valid: false,
        reason: Some(reason),
    }
}

fn inventory_access_error(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    unit_id: crate::world::UnitId,
    inventory_id: InventoryId,
    ui: &InventoryUiState,
) -> Option<InventoryUiError> {
    if ui
        .right_inventory_id
        .is_some_and(|right| right == inventory_id)
        && ui.corpse_id.is_some()
    {
        return if world.inventory_store().get(inventory_id).is_some() {
            None
        } else {
            Some(InventoryUiError::InventoryClosed)
        };
    }
    match can_unit_access_inventory(
        world,
        building_catalog,
        interaction_catalog,
        unit_id,
        inventory_id,
    ) {
        InventoryAccessResult::Allowed => None,
        InventoryAccessResult::Denied(reason) => {
            Some(InventoryUiError::from_inventory_access_denial(reason))
        }
    }
}

fn can_access_inventory(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    unit_id: crate::world::UnitId,
    inventory_id: InventoryId,
    ui: &InventoryUiState,
) -> bool {
    inventory_access_error(
        world,
        building_catalog,
        interaction_catalog,
        unit_id,
        inventory_id,
        ui,
    )
    .is_none()
}

fn can_access_pair(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    unit_id: crate::world::UnitId,
    source: InventoryId,
    destination: InventoryId,
    ui: &InventoryUiState,
) -> bool {
    inventory_access_error(
        world,
        building_catalog,
        interaction_catalog,
        unit_id,
        source,
        ui,
    )
    .or_else(|| {
        inventory_access_error(
            world,
            building_catalog,
            interaction_catalog,
            unit_id,
            destination,
            ui,
        )
    })
    .is_none()
}

/// Cells occupied by an item footprint at the given anchor.
pub fn occupied_cells(anchor_x: u8, anchor_y: u8, width: u8, height: u8) -> Vec<(u8, u8)> {
    let mut cells = Vec::new();
    for dy in 0..height {
        for dx in 0..width {
            cells.push((anchor_x.saturating_add(dx), anchor_y.saturating_add(dy)));
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCatalog, BuildingInteractionProfileCatalog, ChunkLayout, InventoryCatalogCtx,
        InventoryOwnerRef, InventoryProfileCatalog, InventoryProfileId, ItemCatalog,
        ItemCategoryCatalog, ItemDefinitionId, WorldData, create_inventory, place_stack,
        starter_inventory_profile_definitions, starter_item_category_definitions,
        starter_item_definitions,
    };

    fn test_world() -> WorldData {
        WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        })
    }

    fn test_ctx() -> InventoryCatalogCtx<'static> {
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let items = Box::leak(Box::new(items));
        let categories = Box::leak(Box::new(categories));
        let profiles = Box::leak(Box::new(profiles));
        InventoryCatalogCtx::new(items, categories, profiles)
    }

    fn sample_drag(inv: InventoryId, w: u8, h: u8) -> InventoryDragState {
        InventoryDragState {
            source_inventory_id: inv,
            entry_index: 0,
            entry_revision: 1,
            item_definition_id: ItemDefinitionId::new("iron_ore"),
            grid_width: w,
            grid_height: h,
            quantity: 1,
        }
    }

    #[test]
    fn one_by_one_targets_single_cell() {
        let cells = occupied_cells(2, 3, 1, 1);
        assert_eq!(cells, vec![(2, 3)]);
    }

    #[test]
    fn multi_cell_footprint() {
        let cells = occupied_cells(0, 0, 2, 2);
        assert_eq!(cells.len(), 4);
        assert!(cells.contains(&(0, 0)));
        assert!(cells.contains(&(1, 1)));
    }

    #[test]
    fn out_of_bounds_grid_is_invalid() {
        let mut world = test_world();
        let ctx = test_ctx();
        let (mut inv_store, mut inst_store) = world.inventory_runtime_mut();
        let inv_id = create_inventory(
            &mut inv_store,
            &ctx,
            InventoryProfileId::new("unit_backpack_standard"),
            InventoryOwnerRef::Detached,
        )
        .unwrap();
        place_stack(
            &mut inv_store,
            &mut inst_store,
            &ctx,
            inv_id,
            ItemDefinitionId::new("iron_ore"),
            1,
            0,
            0,
        )
        .unwrap();
        drop((inv_store, inst_store));

        let building_catalog = BuildingCatalog::default();
        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let mut ui = InventoryUiState::default();
        ui.left_inventory_id = Some(inv_id);

        let drag = InventoryDragState {
            source_inventory_id: inv_id,
            entry_index: 0,
            entry_revision: entry_revision_for_inventory(&world, inv_id, 0),
            item_definition_id: ItemDefinitionId::new("iron_ore"),
            grid_width: 2,
            grid_height: 2,
            quantity: 1,
        };
        let preview = evaluate_drop_target(
            &world,
            &building_catalog,
            &interaction_catalog,
            &ui,
            &drag,
            InventoryDropTarget::GridCell {
                inventory_id: inv_id,
                anchor_x: 9,
                anchor_y: 9,
            },
        );
        assert!(!preview.valid);
        assert_eq!(preview.reason, Some(InventoryUiError::InvalidTargetCell));
    }

    #[test]
    fn ground_drop_requires_unit_inventory_source() {
        let world = test_world();
        let building_catalog = BuildingCatalog::default();
        let interaction_catalog = BuildingInteractionProfileCatalog::default();
        let ui = InventoryUiState::default();
        let drag = sample_drag(InventoryId::new(1), 1, 1);
        let preview = evaluate_drop_target(
            &world,
            &building_catalog,
            &interaction_catalog,
            &ui,
            &drag,
            InventoryDropTarget::GroundDrop,
        );
        assert!(!preview.valid);
    }
}
