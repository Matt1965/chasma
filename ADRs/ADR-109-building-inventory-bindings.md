# ADR-109: Building Inventory Bindings (EP4)

## Status

Accepted (EP4)

## Context

ADR-091 (I5) established a single optional container per building via `inventory_profile_id`
and `BuildingRecord.inventory_id`. EP3 added `OperationDefinition` future I/O seams without
runtime inventory routing. Production buildings need multiple role-tagged inventories
(input, output, fuel, waste, etc.) without hardcoding fixed struct fields.

## Decision

### Ownership

- **Buildings own inventories** through `BuildingInventoryBindingStore` on `WorldData`.
- **`InventoryRecord`** remains the sole authority for item contents.
- **`BuildingOperationState`** does not cache inventory contents.
- **`OperationDefinition`** references `BuildingInventoryBindingId`, never runtime `InventoryId`.

### Binding model

```text
BuildingId → BuildingInventoryBindingSet → BuildingInventoryBinding
                                              ├ binding_id (stable logical channel)
                                              ├ role (broad purpose)
                                              └ inventory_id (runtime)
```

- `BuildingInventoryBindingId` — stable authored key (`ore_input`, `bread_output`, etc.)
- `BuildingInventoryRole` — `General`, `Input`, `Output`, `Fuel`, `Waste`, `Catalyst`
- Multiple bindings may share the same role; role queries return all matches without picking one.

### Building definitions

- `BuildingDefinition.inventory_bindings` — authored layout (profile per binding)
- `default_inventory_binding_id` — explicit default for generic container access
- Legacy `inventory_profile_id` migrates to implicit `primary` / `General` binding when bindings are empty

### Construction lifecycle

Inventories are allocated at **building create** (including `Planned`), matching ADR-091.
Access remains gated by `building_inventory_operational` (complete + HP > 0). Construction
state transitions do not recreate inventories.

### Operation integration

- `OperationInputDefinition.source_binding` / `OperationOutputDefinition.destination_binding`
- Runtime resolution: `BuildingId` + `BuildingInventoryBindingId` → `InventoryId` (indexed O(1))
- Invalid bindings block production via `OperationalLimitingFactor::InvalidInventoryBinding`
- No item consumption or production in EP4

### Persistence

- `BuildingInventoryBindingStore` serializes binding metadata; inventory contents persist via existing inventory save.
- `BuildingRecord.inventory_id` retained as compatibility accessor for explicit default binding.

## Rejected models

- Fixed `BuildingInventories { input, output, fuel }` struct
- Exactly one inventory per role
- `OperationDefinition` storing runtime `InventoryId`
- Resolving bindings by display label or array index
- `BuildingOperationState` containing item stacks
- Inventories recreated on ECS respawn
- `InventoryRole` doubling as access permissions

## Deferred

| Phase | Scope |
|-------|--------|
| EP5 | Item consumption/production via bindings |
| Future | Access policies, hauling, fuel execution, item restrictions |

## Consequences

- Production buildings declare explicit multi-inventory layouts in catalog data.
- EP5 executes recipes against binding IDs without changing policy/state layout.
- Dev Mode and validation can inspect bindings independently of item movement.
