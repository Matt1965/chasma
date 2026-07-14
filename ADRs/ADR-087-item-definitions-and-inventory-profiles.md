# ADR-087: Item Definitions and Inventory Profiles (I1)

## Status

Accepted — I1 implementation foundation

## Context

ADR-073 established the dual equipment + grid inventory direction. Physical items,
integer quantities, centralized inventory storage, and corpse loot were planned
but had no catalog data layer.

I1 establishes definitions and container profiles only. No runtime inventory
records, transfers, UI grids, or treasury behavior.

## Decision

### Physical item model

- All physical resources are [`ItemDefinition`] catalog rows, including physical gold.
- Quantities are integers; mass is integer grams per unit.
- Items have fixed `grid_width` × `grid_height` footprints; no rotation.
- Stackable commodities use `stackable = true` with `max_stack >= 1`.
- Unique items use `unique_instance_required = true`, `stackable = false`, `max_stack = 1`.

### Hybrid future identity (deferred)

- Stackable commodities → future `ItemStack` placement in grid cells.
- Unique / quality-bearing items → future `ItemInstance` records.
- Quality is a future modifier seam (`quality_profile_id`); condition/durability are not planned.

### Categories

- [`ItemCategoryDefinition`] groups items for authoring and future rules.
- Physical gold uses the `currency` category.
- Category IDs are stable machine identifiers — not derived from display names.

### Inventory profiles

- [`InventoryProfileDefinition`] describes fixed-width × fixed-height container capacity.
- Units and buildings optionally reference `inventory_profile_id`; blank means no inventory.
- Weight fields (`reference_weight_grams`, etc.) are **soft encumbrance metadata** only.
- Placement is rejected only for grid fit, stack-limit violation, or invalid data — never solely for mass.

### Effective stack limit (defined now, enforced in I2+)

```
effective_limit = min(
    item.max_stack,
    profile.global_stack_cap?,
    category_stack_cap?,
    backpack_stack_cap?,
)
```

### Physical gold vs treasury

- Physical gold is a normal stackable item definition (`gold`).
- Abstract settlement treasury balances are **not** item definitions and are deferred.

### Catalog ownership

| Resource | Owner |
|----------|-------|
| `ItemCategoryCatalog` | World foundation plugin |
| `ItemCatalog` | World foundation plugin |
| `InventoryProfileCatalog` | World foundation plugin |

Dev builds import Excel sheets (`Item Categories`, `Items`, `Inventory Profiles`) and export RON under `assets/items/` and `assets/inventory/`. Production builds load empty catalogs (no hidden starter injection) until committed RON loading is wired.

### Excel sheets (header-based columns)

**Item Categories:** Category ID, Name, Enabled; optional Description, Sort Priority.

**Items:** Item ID, Name, Category, Width, Height, Stackable, Max Stack, Mass Grams, Enabled; optional Description, Render Key, Icon Key, Base Value, Tags, Unique Instance Required.

**Inventory Profiles:** Inventory Profile ID, Name, Grid Width, Grid Height, Enabled; optional Reference Weight Grams, Global Stack Cap, Access Type.

**Units / Buildings:** optional `Inventory Profile ID` column.

## Non-goals (I1)

- `InventoryRecord`, `ItemStack`, `ItemInstanceStore`
- Transfers, world piles, corpse pipeline changes
- Inventory UI, equipment behavior, harvesting/hauling/production
- Treasury conversion or NPC gold derivation from treasury balances

## Consequences

- I2 implements authoritative `InventoryRecord` storage — see ADR-088.
- Unit/building authoring can reserve inventory capacity through data.
- Dev mode exposes a read-only Items browser (definitions + profiles).

## References

- ADR-073, ADR-027, ADR-044, ADR-088
- DESIGN.md — inventory and equipment
