# Items & Inventory Final Audit (I8)

## Architecture Summary

All item and inventory truth lives on **`WorldData`** — never on ECS entities or UI state.

| Layer | Owner | ADR |
|-------|-------|-----|
| Item definitions & profiles | Catalogs (read-only at runtime) | ADR-087 |
| Grid inventories & instances | `InventoryStore`, `ItemInstanceStore` | ADR-088 |
| Unit/corpse ownership | `UnitRecord`, `CorpseRecord` + `InventoryOwnerRef` | ADR-089 |
| World piles, drop/loot | `ItemPileStore` | ADR-090 |
| Building containers | `BuildingRecord.inventory_id` | ADR-091 |
| Player UI | `InventoryIntent` → dispatch (client) | ADR-092 |
| Settlement wealth | `SettlementStore` (abstract gold) | ADR-093 |
| Persistence & validation | Scene v7 + `validate_world_inventory_state` | ADR-094 |

Gold is always a physical stack item except treasury balances (abstract only).

## Implemented Systems

- Item definitions, categories, inventory profiles (I1)
- Authoritative grid, stacks, unique instances, derived caches (I2)
- Unit/corpse inventories, death transfer (I3)
- Cross-inventory transfer, piles, drop/pickup/loot (I4)
- Building containers, access policies, spill (I5)
- Kenshi-style inventory UI (I6)
- Treasury deposits, physical/abstract gold split (I7)
- Scene v7 full inventory persistence, unified validation, stress tests (I8)

## Verification Results

| Check | Status |
|-------|--------|
| `cargo fmt --check` | Pass |
| `cargo test --lib` | Pass (1143 tests; includes stress + validation) |
| `cargo test --lib --features dev` | Pass (1256 tests; includes scene v7 roundtrip) |
| `cargo check` | Pass |
| `cargo check --features dev` | Pass |
| `cargo check --features terrain-import` | Pass |

## Remaining Technical Debt

| Item | Severity | Notes |
|------|----------|-------|
| Equipment slots | Low | UI seam only; no runtime equipment (ADR-073 future) |
| Treasury withdrawals | Medium | Documented future work (ADR-093) |
| Move-to-loot orders | Low | Interact opens UI; no pathfinding loot orders |
| Production save format | Medium | Scene v7 is dev-mode; production save not yet separate |
| Settlement transaction log | Low | Dev audit only; not in scene files |
| Merchant/taxation economy | N/A | Explicitly deferred |

## Recommended Future Improvements

1. Production world save using same persistence bundle as scene v7
2. Treasury withdrawal API with same atomicity as `deposit_gold`
3. Equipment runtime on unit inventories
4. Move-to-interact orders for corpses/piles/containers
5. Wealth audit tooling (physical + treasury + piles) for economy phases

## Readiness Assessment

**The Items & Inventory branch is approved for future feature work.**

Authority boundaries are clear, validation is centralized, dev scenes round-trip inventory state,
and integer quantities/mass are enforced throughout. New economy features should extend existing
APIs (`deposit_gold`, transfer ops, spill) rather than parallel stores.

## Manual Validation Checklist

1. Spawn unit and chest (dev mode)
2. Add items and gold to unit inventory
3. Loot corpse / drop pile / pick up
4. Transfer to chest; destroy chest → spill
5. Deposit gold at settlement treasury
6. Save scene (v7) → reload → `validate_world_inventory_state` OK
7. Confirm physical gold and treasury gold remain separate in UI
