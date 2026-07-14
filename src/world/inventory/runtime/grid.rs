//! Grid placement queries and derived-cache rebuild (ADR-088 I2).

use super::catalog_ctx::InventoryCatalogCtx;
use super::entry::{EntryIndex, InventoryEntryContents, PlacedInventoryEntry};
use super::error::InventoryError;
use super::record::{InventoryRecord, cell_index};
use crate::world::ItemDefinitionId;

pub fn footprint_for_definition(item: &crate::world::ItemDefinition) -> (u8, u8) {
    (item.grid_width, item.grid_height)
}

pub fn cells_for_footprint(
    anchor_x: u8,
    anchor_y: u8,
    width: u8,
    height: u8,
) -> impl Iterator<Item = (u8, u8)> {
    (0..height).flat_map(move |dy| {
        (0..width).map(move |dx| (anchor_x.saturating_add(dx), anchor_y.saturating_add(dy)))
    })
}

pub fn footprint_in_bounds(
    grid_width: u8,
    grid_height: u8,
    anchor_x: u8,
    anchor_y: u8,
    width: u8,
    height: u8,
) -> bool {
    let gw = u16::from(grid_width);
    let gh = u16::from(grid_height);
    let ax = u16::from(anchor_x);
    let ay = u16::from(anchor_y);
    let w = u16::from(width);
    let h = u16::from(height);
    ax.saturating_add(w) <= gw && ay.saturating_add(h) <= gh
}

pub fn cells_for_entry(
    record: &InventoryRecord,
    entry: &PlacedInventoryEntry,
    ctx: &InventoryCatalogCtx<'_>,
) -> Result<Vec<(u8, u8)>, InventoryError> {
    let (width, height) = footprint_for_entry(entry, ctx)?;
    if !footprint_in_bounds(
        record.grid_width(),
        record.grid_height(),
        entry.anchor_x,
        entry.anchor_y,
        width,
        height,
    ) {
        return Err(InventoryError::GridOutOfBounds {
            inventory_id: record.id(),
            x: entry.anchor_x,
            y: entry.anchor_y,
        });
    }
    Ok(cells_for_footprint(entry.anchor_x, entry.anchor_y, width, height).collect())
}

pub fn footprint_for_entry(
    entry: &PlacedInventoryEntry,
    ctx: &InventoryCatalogCtx<'_>,
) -> Result<(u8, u8), InventoryError> {
    match &entry.contents {
        InventoryEntryContents::Stack {
            item_definition_id, ..
        } => {
            let def = ctx.require_item(item_definition_id)?;
            Ok(footprint_for_definition(def))
        }
        InventoryEntryContents::Unique {
            item_instance_id, ..
        } => {
            let _ = item_instance_id;
            // Caller must resolve instance definition via store in ops layer.
            Err(InventoryError::ItemInstanceNotFound(*item_instance_id))
        }
    }
}

pub fn footprint_for_entry_with_instance(
    entry: &PlacedInventoryEntry,
    ctx: &InventoryCatalogCtx<'_>,
    definition_id: &ItemDefinitionId,
) -> (u8, u8) {
    match &entry.contents {
        InventoryEntryContents::Stack { .. } => {
            if let Ok(def) = ctx.require_item(definition_id) {
                footprint_for_definition(def)
            } else {
                (1, 1)
            }
        }
        InventoryEntryContents::Unique { .. } => {
            if let Ok(def) = ctx.require_item(definition_id) {
                footprint_for_definition(def)
            } else {
                (1, 1)
            }
        }
    }
}

pub fn can_place_entry(
    record: &InventoryRecord,
    entry: &PlacedInventoryEntry,
    definition_id: &ItemDefinitionId,
    exclude_entry: Option<EntryIndex>,
    ctx: &InventoryCatalogCtx<'_>,
) -> Result<(), InventoryError> {
    let (width, height) = footprint_for_entry_with_instance(entry, ctx, definition_id);
    if !footprint_in_bounds(
        record.grid_width(),
        record.grid_height(),
        entry.anchor_x,
        entry.anchor_y,
        width,
        height,
    ) {
        return Err(InventoryError::GridOutOfBounds {
            inventory_id: record.id(),
            x: entry.anchor_x,
            y: entry.anchor_y,
        });
    }
    for (x, y) in cells_for_footprint(entry.anchor_x, entry.anchor_y, width, height) {
        let Some(cell_idx) = cell_index(record.grid_width(), x, y) else {
            return Err(InventoryError::GridOutOfBounds {
                inventory_id: record.id(),
                x,
                y,
            });
        };
        if let Some(occupant) = record.cell_owner().get(cell_idx).and_then(|v| *v) {
            if exclude_entry != Some(occupant) {
                return Err(InventoryError::CellsOccupied {
                    inventory_id: record.id(),
                });
            }
        }
    }
    Ok(())
}

pub fn can_place_footprint(
    record: &InventoryRecord,
    anchor_x: u8,
    anchor_y: u8,
    width: u8,
    height: u8,
    exclude_entry: Option<EntryIndex>,
) -> bool {
    can_place_footprint_excluding(
        record,
        anchor_x,
        anchor_y,
        width,
        height,
        exclude_entry.iter().copied().collect::<Vec<_>>().as_slice(),
    )
}

pub fn can_place_footprint_excluding(
    record: &InventoryRecord,
    anchor_x: u8,
    anchor_y: u8,
    width: u8,
    height: u8,
    exclude_entries: &[EntryIndex],
) -> bool {
    if !footprint_in_bounds(
        record.grid_width(),
        record.grid_height(),
        anchor_x,
        anchor_y,
        width,
        height,
    ) {
        return false;
    }
    for (x, y) in cells_for_footprint(anchor_x, anchor_y, width, height) {
        let Some(cell_idx) = cell_index(record.grid_width(), x, y) else {
            return false;
        };
        if let Some(occupant) = record.cell_owner().get(cell_idx).and_then(|v| *v) {
            if !exclude_entries.contains(&occupant) {
                return false;
            }
        }
    }
    true
}

pub fn first_fit_position(
    record: &InventoryRecord,
    width: u8,
    height: u8,
) -> Result<(u8, u8), InventoryError> {
    for y in 0..record.grid_height() {
        for x in 0..record.grid_width() {
            if can_place_footprint(record, x, y, width, height, None) {
                return Ok((x, y));
            }
        }
    }
    Err(InventoryError::NoFitPosition {
        inventory_id: record.id(),
    })
}

pub fn entry_mass_grams(
    entry: &PlacedInventoryEntry,
    definition_id: &ItemDefinitionId,
    ctx: &InventoryCatalogCtx<'_>,
) -> Result<u64, InventoryError> {
    let def = ctx.require_item(definition_id)?;
    let quantity = match &entry.contents {
        InventoryEntryContents::Stack { quantity, .. } => *quantity,
        InventoryEntryContents::Unique { .. } => 1,
    };
    let unit_mass = u64::from(def.mass_grams_per_unit);
    unit_mass
        .checked_mul(u64::from(quantity))
        .ok_or(InventoryError::MassOverflow)
}

pub fn rebuild_derived_state(
    record: &mut InventoryRecord,
    ctx: &InventoryCatalogCtx<'_>,
    instance_definition: impl Fn(super::id::ItemInstanceId) -> Result<ItemDefinitionId, InventoryError>,
) -> Result<(), InventoryError> {
    let cell_count = usize::from(record.grid_width()) * usize::from(record.grid_height());
    let mut cell_owner = vec![None; cell_count];
    let mut total_mass = 0u64;

    for (entry_index, entry) in record.placed_entries().iter().enumerate() {
        let definition_id = match &entry.contents {
            InventoryEntryContents::Stack {
                item_definition_id,
                quantity,
            } => {
                let def = ctx.require_item(item_definition_id)?;
                let limit = ctx.stack_limit_for(def, record.profile_id())?;
                if *quantity == 0 || *quantity > limit {
                    return Err(InventoryError::InvalidStackQuantity {
                        quantity: *quantity,
                        limit,
                    });
                }
                if def.unique_instance_required || !def.stackable {
                    return Err(InventoryError::NonStackableItem(def.id.clone()));
                }
                item_definition_id.clone()
            }
            InventoryEntryContents::Unique { item_instance_id } => {
                instance_definition(*item_instance_id)?
            }
        };

        let (width, height) = footprint_for_entry_with_instance(entry, ctx, &definition_id);
        if !footprint_in_bounds(
            record.grid_width(),
            record.grid_height(),
            entry.anchor_x,
            entry.anchor_y,
            width,
            height,
        ) {
            return Err(InventoryError::GridOutOfBounds {
                inventory_id: record.id(),
                x: entry.anchor_x,
                y: entry.anchor_y,
            });
        }

        for (x, y) in cells_for_footprint(entry.anchor_x, entry.anchor_y, width, height) {
            let idx = cell_index(record.grid_width(), x, y).expect("bounds checked");
            if cell_owner[idx].is_some() {
                return Err(InventoryError::CellsOccupied {
                    inventory_id: record.id(),
                });
            }
            cell_owner[idx] = Some(entry_index);
        }

        let mass = entry_mass_grams(entry, &definition_id, ctx)?;
        total_mass = total_mass
            .checked_add(mass)
            .ok_or(InventoryError::MassOverflow)?;
    }

    record.set_cell_owner(cell_owner);
    record.set_total_mass_grams(total_mass);
    Ok(())
}

pub fn validate_inventory_caches(
    record: &InventoryRecord,
    ctx: &InventoryCatalogCtx<'_>,
    instance_definition: impl Fn(super::id::ItemInstanceId) -> Result<ItemDefinitionId, InventoryError>,
) -> Result<(), InventoryError> {
    let mut scratch = record.clone();
    rebuild_derived_state(&mut scratch, ctx, instance_definition)?;
    if scratch.cell_owner() != record.cell_owner()
        || scratch.total_mass_grams() != record.total_mass_grams()
    {
        return Err(InventoryError::CacheInconsistent {
            inventory_id: record.id(),
        });
    }
    Ok(())
}

pub fn validate_stack_quantity(
    item: &crate::world::ItemDefinition,
    quantity: u32,
    limit: u32,
) -> Result<(), InventoryError> {
    if quantity == 0 || quantity > limit {
        return Err(InventoryError::InvalidStackQuantity { quantity, limit });
    }
    Ok(())
}

/// Half-stack quantity uses ceiling division (ADR-088 I2).
pub fn half_stack_quantity(current: u32) -> u32 {
    current.div_ceil(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, ItemCategoryId, ItemDefinition,
        ItemDefinitionId,
    };

    fn ctx() -> (
        ItemCatalog,
        ItemCategoryCatalog,
        InventoryProfileCatalog,
        InventoryCatalogCtx<'static>,
    ) {
        let categories = ItemCategoryCatalog::default();
        let items = ItemCatalog::default();
        let profiles = InventoryProfileCatalog::default();
        // SAFETY: test-only static lifetime extension via leak pattern avoided — use stack
        let items_ref: &'static ItemCatalog = Box::leak(Box::new(items));
        let categories_ref: &'static ItemCategoryCatalog = Box::leak(Box::new(categories));
        let profiles_ref: &'static InventoryProfileCatalog = Box::leak(Box::new(profiles));
        let catalog = InventoryCatalogCtx::new(items_ref, categories_ref, profiles_ref);
        (
            ItemCatalog::default(),
            ItemCategoryCatalog::default(),
            InventoryProfileCatalog::default(),
            catalog,
        )
    }

    #[test]
    fn half_stack_uses_ceiling() {
        assert_eq!(half_stack_quantity(10), 5);
        assert_eq!(half_stack_quantity(9), 5);
        assert_eq!(half_stack_quantity(2), 1);
        assert_eq!(half_stack_quantity(1), 1);
    }

    #[test]
    fn footprint_in_bounds_rejects_overflow() {
        assert!(!footprint_in_bounds(4, 4, 3, 3, 2, 2));
        assert!(footprint_in_bounds(4, 4, 2, 2, 2, 2));
    }
}
