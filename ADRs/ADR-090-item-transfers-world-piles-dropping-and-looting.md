# ADR-090: Item Transfers, World Piles, Dropping, and Looting (I4)

## Status

Accepted

## Context

ADR-088 (I2) centralized inventories and unique instances. ADR-089 (I3) attached inventories to
units and corpses but deferred cross-inventory movement and ground representation.

Gameplay needs one authoritative pipeline for moving items between owners and the world, with
explicit quantity semantics for future UI (full / one / half) and deterministic pile behavior.

## Decision

### Cross-inventory transfer (`src/world/inventory/runtime/transfer.rs`)

Single atomic API surface:

- `transfer_stack_quantity`, `transfer_unique_item`, `transfer_entry_full`
- `transfer_one`, `transfer_half` (`half = ceil(q/2)`)
- `loot_corpse_entry` — corpse inventory is a normal source inventory

Placement policies: `ExactCell`, `MergeThenFirstFit`, `FirstFitOnly` (no silent auto-sort).

Partial moves default **off** (`allow_partial: false`). Full-stack right-click behavior must not
silently fragment unless the caller opts in.

Every transfer validates, simulates placement, commits both sides, or rolls back with structured
`TransferError` / `TransferReport`.

### Item instance location index

`ItemInstanceLocation`: `Detached | Inventory { … } | WorldPile(ItemPileId)`.

Maintained in `ItemInstanceStore`; rebuildable from inventories and piles. No ECS entity IDs in
authoritative location.

### World item piles (`src/world/item_pile/`)

- `WorldItemPileRecord` on `WorldData.item_pile_store` (chunk-keyed, corpse/doodad pattern)
- **One entry per pile**: `WorldPileContents::Stack { … }` or `Unique { … }` — no pile grid in I4
- Merge: same definition, same `SpaceId`, within merge radius, compatible ownership; order by
  quantized distance² then `ItemPileId`
- Overflow: fill merge candidates to `ItemDefinition.max_stack`, spawn additional piles at
  deterministic `OVERFLOW_PILE_OFFSETS`
- Chunk unload does not delete pile truth

### Drop / pickup / spill (`item_pile/authoring.rs`)

- `drop_stack_from_inventory`, `drop_unique_from_inventory`, `drop_unit_inventory_entry`
- `pickup_pile_into_inventory` — failure leaves pile unchanged
- `spill_inventory_to_world_piles` — container destruction seam (no random loss in I4)

### Interaction seams

`InteractionType::ItemPile` and `Corpse` added; order resolution via I6 player inventory UI (ADR-092).

### Runtime presentation (`src/item_piles/`)

Spawn/despawn render entities from chunk residency + `WorldItemPileRecord`; no quantity truth on ECS.

### Dev Mode

World Tools tab: pile spawn, drop (full/one/half), pickup, corpse loot, validation (`pile_harness.rs`).

### Future explicit loss

Container destruction may supply a deterministic loss plan before `spill_inventory_to_world_piles`.
No hidden RNG inside transfer/pile code.

## Consequences

- Full Kenshi inventory UI, equipment, hauling, and movement-to-loot are later phases
- Persistence: pile records + `next_item_pile_id` are serialization-ready; full save in I8
- Building/container inventories: see ADR-091 (I5)

## Related

- ADR-088 — inventory grid and instances
- ADR-089 — unit/corpse inventory ownership
- ADR-042 — interaction types (ItemPile/Corpse seams)
