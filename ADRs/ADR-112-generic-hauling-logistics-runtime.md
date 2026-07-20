# ADR-112: Generic Hauling and Logistics Runtime (EP7)

## Status

Accepted

## Context

EP1–EP6 established building production, operation catalogs, role-tagged inventories, generic
execution, and terrain-influenced extraction. Buildings can produce and consume items, but items
did not move between buildings through an authoritative logistics runtime. EP7 must deliver
generic hauling: buildings generate work, workers execute work, items move physically through
worker inventories, and reservations prevent duplication — without worker world-scanning,
teleportation, or worker-owned logistics state.

## Decision

### Buildings generate hauling work

`BuildingDefinition.logistics_routes` declares data-driven routes with
`LogisticsRouteTrigger::OutputSurplus` or `InputDeficit`, item id, local binding, remote building
definition, and remote binding. Production assessment hooks call `sync_logistics_requests_from_assessment`
and `sync_output_surplus_after_production` to upsert requests. Buildings never physically move
items.

### HaulingRequest on WorldData

`HaulingRequest` is authoritative state on `WorldData` via `HaulingRequestStore`. Workers
reference requests while executing; they do not own logistics state. Minimum fields: typed id,
priority, item, quantity, remaining quantity, source/destination inventories, owning building,
generation reason, status, reservation state, assignment, blocking reason, execution phase.

### Endpoint index — no global item scanning

Remote inventories resolve through `LogisticsEndpointIndex` (building definition + binding →
building ids). Workers never scan all inventories for work. Flow:

```
Building → HaulingRequest → Task System → Worker
```

### Reservations reuse inventory architecture

`InventoryReservationStore` on `WorldData` reserves source items and destination capacity per
request. `reserve_hauling_request` reserves destination before source. Failed reservation
blocks or cancels assignment. `release_request_reservations` runs on deposit, cancel, and
building removal.

### Atomic pickup and deposit

`pickup_haul_cargo` transfers from source to worker inventory. `deposit_haul_cargo` transfers
from worker inventory to destination. Items exist in worker inventory while hauled — no
teleportation. Partial deliveries update `remaining_quantity` and return workers to source.

### Task integration

`TaskType::Haul` with `TaskTarget::HaulRequest { request_id, owning_building_id }`.
`assign_hauling_task` creates tasks; `step_haul_worker_tasks` runs after `step_all_worker_tasks`
in the simulation tick. Construction and workstation labor skip haul tasks.

### Request consolidation and priority

Open requests with the same source, destination, and item merge quantities. Priorities
(Critical, High, Normal, Low) are stored for future AI adjustment.

### Building removal and persistence

`cancel_logistics_for_building_removal` cancels owned requests and releases reservations.
`LogisticsSaveState` persists requests and reservations; dev scenes capture/restore via
`SceneLogisticsPersistence`.

## Rejected designs

| Design | Reason |
|--------|--------|
| Worker scans world inventories for items | O(inventories) scaling; violates building-generated work model |
| Item teleportation source → destination | Breaks physical transport; incompatible with worker inventory and audit |
| Worker-owned logistics state | Workers execute tasks; buildings and WorldData own authoritative requests |
| Per-building hauling systems (mine haul, smelter haul) | Duplicates logic; routes are data on `BuildingDefinition` |
| Duplicate reservation system | `InventoryReservationStore` extends existing reservation architecture |

## Consequences

- Starter routes: iron_mine output → storage_chest, smelter ore_input ← storage_chest,
  workbench flour_input ← storage_chest.
- Settlement-scoped endpoint resolution prefers remote buildings in the same settlement.
- Dev inspector displays hauling requests; Q / Shift+Q / Ctrl+Q spawn, cancel, force-complete.
- Settlement AI, stock goals, path optimization, and multi-stop hauling remain future work.
