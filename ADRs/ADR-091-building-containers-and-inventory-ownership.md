# ADR-091: Building Containers and Inventory Ownership (I5)

## Status

Accepted

## Context

ADR-087 (I1) defined `InventoryProfileId` on building definitions. ADR-088 (I2) centralized
`InventoryRecord` grids. ADR-090 (I4) added `spill_inventory_to_world_piles` for destruction seams.

Buildings must act as authoritative inventory containers using the same grid system as units,
corpses, and future workstations — without a parallel bulk-storage system or ECS-owned item truth.

## Decision

### Container model

A container is **not** a new world-object category:

```
BuildingRecord + inventory_profile (definition)
               + inventory_id (instance)
               + access policy + interaction points
               + lifecycle gating + optional lock state
```

All item truth remains in `InventoryStore` on `WorldData`. Runtime building ECS entities never own
contents.

### Definition fields (`BuildingDefinition`)

- `inventory_profile_id: Option<InventoryProfileId>` — `None` means no inventory
- `inventory_access_policy: ContainerAccessPolicy` — `Everyone | OwnerOnly | Team`
- `inventory_interaction_point_key: Option<String>` — spill/access placement
- `spill_on_destroy: bool` — default `true`; when false, destruction deletes contents without spill

No recipe/input/output roles in I5.

### Instance fields (`BuildingRecord`)

- `inventory_id: Option<InventoryId>` — centralized store reference
- `container_locked: bool` — runtime lock (serializable via scene seam)

Owner back-reference: `InventoryOwnerRef::Building(building_id)`.

### Allocation lifecycle

**Allocate at building create** (including `Planned` player placement), not at `Complete`.

Rationale: stable `InventoryId` across construction, future material-delivery seam, clean save/load.

Access is gated by `building_inventory_operational` (`Complete` + HP > 0). Incomplete buildings
cannot be used as normal storage.

APIs: `create_building_with_inventory`, `place_player_building_with_inventory`,
`attach_inventory_on_building_create`.

### Access query

`can_unit_access_inventory(world, building_catalog, unit_id, inventory_id)` routes by owner type.

`can_unit_access_building_inventory` validates requester, inventory existence, operational lifecycle,
`container_locked`, and `ContainerAccessPolicy` against runtime ownership/team — not item names or
faction tags.

### Interaction

- `BuildingCapabilities.container`
- `InteractionType::Container` + `InteractionOrderPlan::AccessContainer` (order execution deferred;
  dev transfer APIs active)
- Interaction points from `BuildingInteractionProfile` (e.g. `storage_chest` starter)

### Destruction spill

`destroy_building` accepts optional `BuildingInventoryCleanup` (inventory ctx, pile settings,
interaction catalog, tick). When present:

1. `finalize_building_inventory_removal` with `SpillToWorld`
2. `spill_inventory_to_world_piles` (I4) at interaction point / anchor
3. `remove_owned_inventory` + clear `BuildingRecord.inventory_id`

Default: no random item loss; all contents survive and spill.

### Removal policies (`BuildingInventoryRemovalPolicy`)

| Policy | Use |
|--------|-----|
| `SpillToWorld` | Destruction (default) |
| `DeleteContents` | Dev delete without ground spill |
| `TeardownWithoutSpill` | Scene clear / interior child removal |

`remove_building` takes explicit inventory cleanup + policy.

### Persistence seam

`SceneBuildingRecord` serializes `inventory_id` and `container_locked`. Full world save remains I8;
schemas are ready.

### Dev Mode

Building inspector: inspect inventory (`I`), add gold (`G`), unit↔building transfer (`T`), toggle
lock (`U`), validate links (`V`). Destroy (`X`) spills via inventory cleanup.

### Merchant seam (future)

Stable owner/access data and direct inventory APIs only. No full-world inventory aggregation scan.

## Consequences

- No storage filters, auto-hauling, recipes, treasury, or equipment in I5
- UI open/closed state stays client-local
- `validate_building_inventory_links` supports debug/test orphan detection

## Related

- [ADR-078](ADR-078-building-catalog-definitions.md) — definition catalog
- [ADR-085](ADR-085-building-interactions-tasks-and-construction-labor.md) — interaction points
- [ADR-088](ADR-088-authoritative-inventory-grid-and-item-identity.md) — inventory grid
- [ADR-090](ADR-090-item-transfers-world-piles-dropping-and-looting.md) — spill/transfer APIs
