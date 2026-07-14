# ADR-088: Authoritative Inventory Grid and Item Identity (I2)

## Status

Accepted

## Context

ADR-087 (I1) defined item definitions, categories, and inventory profiles without runtime
containers. Gameplay systems need a single authoritative place to create inventories, place
stackable commodities and unique instances on fixed grids, mutate contents atomically, and query
weight — before inventories attach to units, buildings, corpses, or world piles.

## Decision

### Central authority on `WorldData`

`WorldData` owns:

- `InventoryStore` — `InventoryId → InventoryRecord`
- `ItemInstanceStore` — `ItemInstanceId → ItemInstance` plus `instance_location` back-references

ECS and UI hold no inventory truth. All mutations go through `src/world/inventory/runtime/ops.rs`.

### Identifiers

- `InventoryId`, `ItemInstanceId` — `u32`, invalid = 0, allocation starts at 1, monotonic,
  `restore_next_id` for future persistence, sorted iteration via `BTreeMap`.

### Owner reference (attachment seam)

`InventoryOwnerRef`: `Detached`, `Unit(UnitId)`, `Building(BuildingId)`, `Corpse(CorpseId)` (world pile deferred to I4).

I2 uses `Detached` for dev/test inventories. I3 attaches `Unit` and `Corpse` owners.

### Grid model

- `PlacedInventoryEntry { anchor_x, anchor_y, contents }`
- `InventoryEntryContents::Stack { item_definition_id, quantity }` or `Unique { item_instance_id }`
- Footprint from `ItemDefinition` (`grid_width` × `grid_height`), no rotation
- Derived caches on `InventoryRecord` (not serialized): `cell_owner`, `total_mass_grams`

### Unique instances

`ItemInstance { id, definition_id, metadata }` — metadata may include optional `quality`.
No condition, durability, or repair fields in I2.

### Soft weight

Exact `u64` gram totals are authoritative. Profile reference/comfort weights are soft — placement
never fails solely for overweight. `query_inventory_weight` exposes over-reference amount/ratio.

### Stack limits

`effective_stack_limit` (I1) applies on every stack mutation.

### Half-stack rule

`half_stack_quantity = ceil(current / 2)` for future UI bindings (Ctrl/Shift/right-click). Not
wired to input in I2.

### Atomic operations

`create_inventory`, `remove_inventory`, `create_item_instance`, `destroy_item_instance`,
`place_stack`, `place_unique`, `remove_entry`, `move_entry`, `swap_entries`, `merge_stacks`,
`split_stack`, `auto_sort`, `migrate_inventory_profile(_with_leftovers)` — validate before
commit or roll back; structured `InventoryError` outcomes.

### Auto-sort

1. Validate entries
2. Merge compatible stacks to effective limits
3. Deterministic sort (area, max dimension, category priority, ids, stable tie-break)
4. Row-major first-fit into empty grid
5. Commit or roll back unchanged on failure

### Profile migration

Oversized stacks split to legal quantities, entries repacked deterministically. Entries that do not
fit are returned as explicit leftovers (`migrate_inventory_profile_with_leftovers`). Nothing is
deleted. Spill to world piles / corpses is owned by I3/I4/I5.

### Dev harness

Items tab → Harness subtab (`H`): detached inventory create, add stack/unique, split, merge,
auto-sort, validate, delete via keyboard APIs.

## Consequences

- Units/buildings can attach inventories in I3/I5 by setting `InventoryOwnerRef` and reusing ops.
- Cross-inventory transfer, equipment, UI, and treasury remain out of scope.
- Cache rebuild must run after every successful mutation; `validate_inventory_stores` for debug.

## Related

- ADR-087 — item definitions and inventory profiles (I1)
- ADR-073 — inventory and equipment design direction
