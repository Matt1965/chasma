# ADR-094: Inventory Persistence, Validation, and Final Audit (I8)

## Status

Accepted

## Context

I1–I7 implemented authoritative items, inventories, corpses, world piles, building containers,
player UI, and settlement treasuries. Scene v6 persisted settlements/treasuries and building
`inventory_id` references, but not inventory contents. Multiple granular validators existed without
a single world-level entry point.

I8 completes the Items & Inventory roadmap: production-ready persistence, validation, stress coverage,
and documentation — no new gameplay systems.

## Decision

### Scene v7 persistence

Bump `SCENE_VERSION` to **7**. Flatten `SceneInventoryPersistence` onto `SceneDefinition`:

| Persisted | Omitted (rebuilt on load) |
|-----------|---------------------------|
| `InventoryRecord` entries, owner, profile, grid | `cell_owner`, `total_mass_grams` |
| `ItemInstance` + `ItemInstanceLocation` | — |
| `CorpseRecord`, `WorldItemPileRecord` | chunk indexes (rebuilt on insert) |
| `next_inventory_id`, `next_item_instance_id`, `next_corpse_id`, `next_item_pile_id` | — |
| Unit `inventory_id`, building `inventory_id` | ECS, UI, occupancy caches |

Capture: `capture_inventory_persistence` in `dev/scenes/inventory_snapshot.rs`.

Restore: `restore_inventory_persistence` after units/buildings/settlements; `rebuild_all_inventory_derived`
recomputes occupancy and mass; post-apply `validate_world_inventory_state`.

v1–v6 scenes load with empty inventory state (backward compatible).

### Unified validation

`validate_world_inventory_state(world, ctx) -> WorldInventoryValidationReport` orchestrates:

1. `validate_inventory_stores`
2. `validate_item_pile_store`
3. `validate_item_instance_locations`
4. Owner link checks (unit/building/corpse ↔ inventory)
5. Settlement treasury anchor checks

Dev harness **V** keys and scene apply call this entry point.

### Stress tests

`world/inventory/stress.rs`: many inventories, split/merge/auto-sort, store round-trip.

### Dev inspector

Unit inspector shows `inventory_summary` (entry count, carried mass). Building inspector unchanged.
World Tools **V** runs full world validation.

## Consequences

- Dev scenes round-trip full inventory world state.
- Derived caches never serialized; load always rebuilds.
- Future economy features must preserve validation invariants.
- Transaction log remains dev-only (not in scene files).

## References

- [ADR-045](ADR-045-dev-scene-snapshots.md) — scene architecture
- [ADR-088](ADR-088-authoritative-inventory-grid-and-item-identity.md) — inventory authority
- [ADR-090](ADR-090-item-transfers-world-piles-dropping-and-looting.md) — piles
- [ADR-093](ADR-093-settlement-treasuries-and-physical-gold.md) — treasuries
