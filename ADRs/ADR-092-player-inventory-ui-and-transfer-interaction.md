# ADR-092: Player Inventory UI and Transfer Interaction (I6)

## Status

Accepted

## Context

I1–I5 established authoritative items, grid inventories, unit/corpse ownership, world piles,
and building containers. Players still lacked a Kenshi-style interface to inspect grids, transfer
items, and use quick-transfer semantics without mutating `WorldData` from UI code.

ADR-038 requires client intents before authoritative mutation. ADR-088–091 own inventory truth on
`WorldData`; UI must reconcile from `InventoryRecord` on every update.

## Decision

### Authority rule

| Layer | May read | May write |
|-------|----------|-----------|
| Inventory UI (`InventoryUiState`, Bevy widgets) | `InventoryRecord`, catalogs, access state | Nothing on `WorldData` |
| `InventoryIntentQueue` | — | Enqueue typed intents |
| `dispatch_inventory_intents` | World + catalogs | Inventory/pile APIs only |

UI never edits stack quantity, anchor cells, `ItemInstance` location, mass, or owner directly.

### Client-local presentation state

`InventoryUiState` (`src/ui/gameplay/inventory/state.rs`):

- Open mode: closed, unit-only, dual transfer, world pile
- `left_inventory_id` / `right_inventory_id`, `actor_unit_id`, `corpse_id`, `pile_id`
- Selection, drag payload (`InventoryDragState` with entry revision), optional split dialog seam
- Feedback string; **no** item content truth

On authoritative updates, `sync_inventory_panel_contents` rebuilds grids from current records.
`reconcile_open_inventories` closes the panel when inventories, corpses, piles, or actors disappear.

### Intent types

`InventoryIntent` (`src/client/inventory_intent.rs`):

- `Open` / `Close`
- `MoveEntry`, `TransferFull`, `TransferOne`, `TransferHalf`, `TransferToCell`
- `AutoSort`, `DropEntry`, `PickupPile`, `LootAll`

`dispatch_inventory_intents` validates access, revision (stale drag), and delegates to I2/I4 APIs:
`move_entry`, `transfer_*`, `auto_sort`, `drop_unit_inventory_entry`, `pickup_pile_into_inventory`,
`loot_corpse_entry`.

Interact-armed container/corpse/pile opens queue via `try_queue_inventory_open_from_interact` in
client dispatch (ADR-041 contextual command path).

### Layout

Modal/expandable panel (`spawn_inventory_panel`), not permanent HUD clutter:

- Header: owner label, weight (kg + reference + heavy hint), physical gold summary, auto-sort, close
- Main: scrollable authoritative grid; one widget per placed entry spanning footprint cells
- Details: item name, category, size, qty, mass, value, tags, unique metadata
- Equipment: typed slots visible, disabled (“not implemented in I6”)
- Dual view: unit left, container/corpse right; world pile uses pickup affordance instead of grid

### Grid rendering

- Cell size constant (`CELL_PX`); item width/height from `ItemDefinition` footprint (no rotation)
- Stacks show `×qty`; unique items show definition display name
- Missing definition falls back to ID string
- Occupied cells dimmed; entry buttons span anchor rectangle

### Opening inventories

| Source | Mode |
|--------|------|
| **I** key + primary selected unit | `UnitOnly` |
| Interact + container building | `DualTransfer` (access-gated) |
| Interact + corpse | `DualTransfer` (loot) |
| Interact + world pile | `WorldPile` |

Remote/inaccessible targets reject at commit with `InventoryUiError` message; neutral-owner
`OwnerOnly` containers with no `owner_id` allow access per ADR-091 policy.

### Control semantics (locked)

| Input | Behavior |
|-------|----------|
| Left-click | Select entry / show details |
| Left-drag + drop on cell | `MoveEntry` (same inv) or `TransferToCell` (cross inv) |
| Right-click | `TransferFull` to other open inventory (`MergeThenFirstFit`) |
| Ctrl+click | `TransferOne` |
| Shift+click | `TransferHalf` (`half_stack_quantity` = ceil(n/2)) |
| Auto-sort button | `AutoSort` intent → authoritative `auto_sort` |
| Esc | Cancel drag/dialog first, then close panel |

Semantics live in UI input systems only, not in `world::inventory` modules.

### Access and range

`can_unit_access_inventory` / building inventory access re-run on every transfer intent.
Corpse loot allows corpse inventory when `ui.corpse_id` matches. Locked containers and wrong
ownership deny with mapped errors. Distance/space checks use building access APIs where wired.

### Weight and gold

Weight from `query_inventory_weight` (grams authoritative, kg in UI). Over-reference shown as
“heavy”; no pickup block in I6. Physical gold counted from `gold` stack entries per open inventory.

### Input focus

`inventory_panel_blocks_world_input` returns true while open; client pipeline skips world intents.
Inventory panel clicks are Bevy UI (no terrain pass-through). Text fields consume keys when focused
(future split dialog seam). Esc ordering: drag → split dialog → close.

### Equipment seam

Placeholder labels only; no equip intents, no stat effects, innate weapon unchanged.

### Performance

Grid rebuild on `InventoryUiState` or `WorldData` change; revision fields reserved for diffing.
No per-frame catalog scans beyond open panel sync.

### Dev Mode

Player inventory UI is separate from F12 Items harness (ADR-088). Building inspector (I5) and pile
harness (I4) remain authoritative test tools. Future dev overlay may inspect `InventoryUiState` and
pending intents without mutating truth.

## Non-goals (I6)

Equipment, treasury deposit, harvesting, hauling, storage filters, production, item rotation,
durability, squad pooled inventory, shop UI.

## Consequences

- Kenshi-style transfers without parallel UI-owned inventory state
- All mutations auditable through `InventoryIntent` dispatch
- Equipment and move-to-loot orders remain future phases

## References

- ADR-088 (grid), ADR-090 (transfer/pile), ADR-091 (containers)
- ADR-050 (HUD shell), ADR-041 (commands), ADR-038 (intents)
- `src/ui/gameplay/inventory/`, `src/client/inventory_intent.rs`, `src/client/inventory_dispatch.rs`
