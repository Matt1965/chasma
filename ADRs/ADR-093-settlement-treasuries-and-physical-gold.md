# ADR-093: Settlement Treasuries and Physical Gold (I7)

## Status

Accepted

## Context

I1–I6 established physical items, inventories, containers, and player inventory UI. Gold is a normal
`ItemDefinition` (`gold`) that may exist in unit inventories, corpses, chests, world piles, and future
merchant inventories.

Settlement wealth must be tracked separately from physical item stacks. Treasuries must not store
`InventoryId`s or spawn gold from abstract balances. Players need a deposit path that removes physical
gold and credits settlement wealth atomically.

Future economy systems (taxation, payroll, shops, banking) are explicitly out of scope for I7.

## Decision

### Physical gold rule

Gold remains a physical stack item everywhere in the world. Treasury balances are never converted back
into items in I7 (no withdrawals). Loot and world gold are never derived from treasury records.

### Settlement treasury records

`SettlementTreasuryRecord` (`src/world/settlement/record.rs`):

- `TreasuryId`, `SettlementId`, ownership mirror, `balance_gold`, `created_tick`, metadata
- Stores **abstract wealth only** — never `InventoryId`

`SettlementRecord` anchors a legitimate settlement to a building with `settlement_treasury` capability
(`settlement_core` starter building). Normal chests cannot host settlements.

`SettlementStore` on `WorldData` owns settlements, treasuries, building/settlement indexes, and a
dev/audit transaction log.

### Deposit API

`deposit_gold(...)` (`src/world/settlement/deposit.rs`):

1. Validate access (`can_unit_deposit_to_treasury`: policy, space, range)
2. Validate source inventory owned by depositor unit
3. Validate quantity and overflow
4. `consume_stack_item` for physical `gold`
5. Increment `balance_gold`
6. Log `TreasuryTransactionRecord`

Failure rolls back inventory mutation. No partial commits.

### Interaction

`InteractionType::Treasury` on settlement-capable buildings (checked before container classification).
Interact + in-range authorized unit opens `InventoryOpenMode::TreasuryDeposit` via intent queue.

### UI (ADR-092 extension)

Treasury deposit panel shows:

- **Physical Gold** — sum of `gold` stacks in the actor inventory
- **Treasury Gold** — `SettlementTreasuryRecord.balance_gold`

Values are never combined. Deposit buttons enqueue `InventoryIntent::DepositGold` (one / half / all).

### Dev mode

World Tools treasury harness (`src/dev/treasury_harness.rs`):

- Create settlement on selected building
- Inspect treasury
- Deposit gold
- Validate physical vs treasury totals
- Transaction log tail

### Persistence

Scene format v6 captures `settlement_records`, `treasury_records`, and ID counters. Derived indexes
rebuild on load via `SettlementStore::restore_snapshot`.

## Future seams (not implemented)

- Treasury withdrawals (abstract → physical item)
- Remote deposits
- Taxation, payroll, upkeep, shops, banking, faction budgets, caravan finance
- Merchant economy rules

## Consequences

- World wealth conservation: physical gold + treasury balances are tracked separately; deposits move
  value between buckets without duplication.
- Economy features must call `deposit_gold` (or future withdrawal APIs) rather than mutating balances
  ad hoc.
- Scene v5 files load with empty settlement state; v6 round-trips treasuries.

## References

- [ADR-087](ADR-087-item-definitions-and-inventory-profiles.md) — gold item definition
- [ADR-088](ADR-088-authoritative-inventory-grid-and-item-identity.md) — inventory authority
- [ADR-092](ADR-092-player-inventory-ui-and-transfer-interaction.md) — inventory UI intents
