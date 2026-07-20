# ADR-113: Multi-Building Production Chains (EP8)

## Status

Accepted

## Context

EP1–EP7 established building production runtime, operation catalog, role-tagged inventories,
generic execution, terrain extraction, and hauling logistics. EP8 must prove complete vertical
slices — Iron Ore → Smelter → Iron Bar and Flour + Water → Bread — using the same generic
runtime with no operation-specific systems.

## Decision

### Processing and crafting are ordinary operations

`smelt_iron` (Processing) and `bake_bread` (Crafting) are `OperationDefinition` catalog entries.
`execute_production_cycle` performs all inventory mutations. No `SmelterSystem`, `BakerySystem`,
or `CraftingSystem`.

### Input timing: consumption at cycle completion (EP5 preserved)

Labor accumulates while `Running`. When progress reaches one unit, the runtime:

1. Plans inputs/outputs through binding resolution and aggregation
2. Validates unreserved availability and projected output capacity
3. Atomically consumes all inputs and produces all outputs

Inputs are not consumed at cycle start. Progress may complete while inputs are missing; the cycle
blocks without consuming or incrementing `completion_count`.

### Shared reservation architecture (EP7 + EP8)

`InventoryReservationStore` is authoritative for hauling reservations. Production validation uses
`available_stack_quantity` (physical minus reserved) when checking inputs. If physical quantity
satisfies the recipe but reserved quantity prevents use, `InputReserved` is reported.

Production does not maintain a separate reservation map.

### Fuel deferred (Option A)

`smelt_iron` consumes Iron Ore only. `bake_bread` consumes Flour and Water only. `fuel_input`
bindings remain on building definitions as future seams but are not required by current operations.

### Logistics routes complete the chains

Starter routes:

| Building | Route | Item |
|----------|-------|------|
| iron_mine | output surplus → storage | iron_ore |
| smelter | input deficit ← storage | iron_ore |
| smelter | output surplus → storage | iron_bar, slag |
| workbench | input deficit ← storage | flour, water |
| workbench | output surplus → storage | bread |

Remote endpoints must `advertise_logistics_supply`; local input endpoints must
`accept_logistics_delivery`. Input-role inventories are not supply endpoints.

### Multi-input/output transaction planning

`plan_execution` aggregates duplicate `(inventory, item)` inputs, validates all inputs before
any mutation, simulates cumulative output placement across shared inventories, and commits
atomically with rollback on failure.

## Rejected designs

| Design | Reason |
|--------|--------|
| Dedicated crafting runtime | Duplicates EP5 execution; violates generic operation model |
| Smelter/Bakery-specific systems | Same |
| Fuel burn duration / heat simulation | Out of EP8 scope; half-fuel system avoided via Option A |
| Worker world-scanning for ingredients | Logistics generates demand; workers haul |
| Item teleportation into buildings | Physical inventory + hauling only |
| Separate production reservation store | Single `InventoryReservationStore` |
| Input consumption at cycle start | EP5 locked completion-time consumption |
| Maintain Stock / settlement planning | Future phase |

## Consequences

- EP8 chain tests in `ep8_chain_tests.rs` cover smelting, baking, hauling integration, reservations, repeat count, and save/load.
- Dev inspector shows physical, available, and reserved input quantities.
- `OperationalLimitingFactor::InputReserved` distinguishes reserved vs missing inputs.
