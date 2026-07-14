# ADR-089: Unit Inventories, Corpse Ownership, and Item Survival (I3)

## Status

Accepted

## Context

ADR-087 (I1) defined item and inventory profile catalogs. ADR-088 (I2) implemented authoritative
`InventoryStore`, `ItemInstanceStore`, grid placement, and soft-weight queries on `WorldData` using
`InventoryOwnerRef::Detached` for dev harnesses.

Gameplay requires units to carry inventories through data, survive death as corpse-owned loot, and
expire without ground spill until I4 world piles exist. Movement and combat must remain unchanged.

## Decision

### Unit definition integration

- `UnitDefinition.inventory_profile_id: Option<InventoryProfileId>` — `None` means no inventory.
- `UnitDefinition.corpse_lifetime_ticks: Option<u64>` — optional per-unit override; otherwise
  `CorpseSettings.default_lifetime_ticks` (9000 ticks @ 30 Hz ≈ 5 minutes).
- Validation: referenced profile must exist and be enabled. No species/faction hardcoded fallbacks.

### Unit record integration

- `UnitRecord.inventory_id: Option<InventoryId>` — references centralized `InventoryStore`.
- Unit records never embed item entries. Inventory survives chunk residency and render unload.

### Unit creation

- `create_unit_with_ownership` — no inventory (6-arg API, backward compatible).
- `create_unit_with_inventory` — attaches empty `InventoryRecord` when definition has a profile.
- Owner = `InventoryOwnerRef::Unit(unit_id)`. Roll back inventory on `insert_unit` failure.

### Removal policy

`finalize_unit_removal(world, unit_id, reason, …)`:

| Reason | Inventory behavior |
|--------|-------------------|
| `Killed` | Create `CorpseRecord`, retarget owner `Unit → Corpse`, remove unit |
| `DevDeleted`, `Cleanup`, `Unknown` | `remove_owned_inventory` (no ground spill) |

Death pipeline calls `finalize_unit_removal` when `InventoryCatalogCtx` is available; legacy callers
without ctx use direct `remove_unit_by_id` (tests only).

### Corpse model (`src/world/corpse/`)

`CorpseRecord` on `WorldData.corpse_store` (chunk-keyed spatial index):

- `CorpseId`, `origin_unit_id`, `unit_definition_id`, placement, `current_space_id`
- `inventory_id: Option<InventoryId>`
- ownership metadata (`owner_id`, `team_id`, `affiliation`) for future loot legality
- `remaining_lifetime_ticks`, `CorpseState::{Present, Expired}`
- No render entities, animation timers, or ECS truth

### Death transfer sequence

1. Unit reaches authoritative death → queued removal (`RemovalReason::Killed`)
2. `create_corpse_from_unit` at unit placement/space
3. If `inventory_id` present: `transfer_inventory_to_corpse` — same `InventoryId`, owner retarget only
4. `remove_unit_by_id` — no item copy
5. Presentation derives from existing death animation + future corpse sync (no duplicate bodies policy)

### Corpse lifetime

- Advances on fixed simulation ticks via `step_corpse_lifecycle` (after death pipeline in tick order)
- Pause freezes simulation ticks; chunk unload does not remove corpses
- Expiration: remove corpse + owned inventory + unique instances — **no ground spill** (I4 may add piles later)

### Soft weight queries

`unit_inventory_weight_grams`, `unit_reference_weight_grams`, `unit_over_reference_weight_grams`,
`unit_encumbrance_ratio` — query-only; no movement/combat penalties in I3.

### Simulation integration

`run_simulation_tick` accepts item catalogs + `CorpseSettings`. `SimulationCatalogParams` bundles
resources for Bevy system param limits.

### Future seams (not I3)

- World piles (I4), cross-inventory transfer, loot UI, equipment, encumbrance penalties
- `InteractionType::Corpse` selection and access checks
- Building/container inventories (I5)

## Consequences

- Positive: Single inventory authority through death; deterministic corpse IDs; explicit removal reasons
- Negative: Dev spawn without inventory ctx does not attach profile inventories until caller passes ctx
- Tests: bandit starter fixture uses `unit_backpack_standard` profile

## References

- ADR-087, ADR-088, ADR-059 (death pipeline), ADR-073
