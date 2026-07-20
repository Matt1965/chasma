# ADR-107: Generic Building Production Runtime (EP2)

## Status

Accepted (EP2)

## Context

ADR-105 (TF5) introduced terrain-driven operational efficiency and workstation labor stepping
with per-building `ProductionProgress`, but production policy, lifecycle, persistence, and
the permanent runtime framework for extraction, crafting, refining, farming, and research
were deferred.

EP1 exploration established building-owned production on `WorldData`, separate policy/state,
and worker-task labor integration. EP2 formalizes this as the permanent production runtime
without concrete operation content.

## Decision

### Authority and ownership

- **Buildings own production.** `BuildingProductionStore` on `WorldData` is the single
  authoritative source, keyed by `BuildingId`.
- **Workers contribute labor only.** `TaskType::OperateWorkstation` drives
  `step_workstation_operation`; workers do not own progress or create items.
- **No separate Workstation runtime object.** Production extends the existing Building
  architecture; no parallel hierarchy.
- **One active operation per building.** Multiple interaction points may contribute labor to
  the same operation; unrelated simultaneous operation slots (furnace + anvil + oven in one
  building) are rejected — those are separate Building instances.

### State and policy

- `BuildingOperationState` — authoritative runtime: fixed-point progress, lifecycle,
  completion count, active worker count, blocking reason (transient), efficiency revision.
- `BuildingOperationPolicy` — intent: enabled, paused, selected operation ID seam,
  execution mode (`Continuous` / `RepeatCount`), priority, control source
  (`PlayerControlled` / `AIControlled`).
- `OperationDefinitionId` — typed-ID seam for EP3 `OperationCatalog`; may be absent;
  runtime handles `None` cleanly. No hardcoded operation definitions in EP2.

### Lifecycle

Generic states: `Idle`, `Running`, `Blocked`, `Paused`, `Disabled`, `Completed`.
Blocking reasons extend `OperationalLimitingFactor` (including `Paused`, `InvalidOperation`,
`MissingBuilding`) rather than a separate production error enum.

### Labor and progress

- Continuous labor per fixed tick while worker is in range with valid task/reservation.
- Progress = base progress × worker labor × operational efficiency (ADR-105).
- Completion increments `completion_count` only — no item creation or input consumption.
- `Continuous` cycles indefinitely; `RepeatCount` enters `Completed` when satisfied.

### Stepping model

Production advances via **worker-task iteration** in `step_all_worker_tasks`, not a global
per-building scan each tick. O(1) store lookup per stepped building.

### Persistence

`BuildingProductionSaveState` persists state and policy through world save and scene v10+.
Orphaned production records are detectable via `validate_production_runtime`.

### Commands and Dev Mode

Authoritative mutations use `set_production_enabled`, `set_production_paused`,
`set_production_execution_mode`, `reset_production_progress`, etc. Dev inspector exposes
production fields and testing hotkeys; advanced diagnostics are collapsible.

## Deferred

| Phase | Scope |
|-------|--------|
| EP3 | `OperationCatalog`, `OperationDefinition` content |
| EP4 | Role-tagged building inventory bindings |
| EP5 | Concrete extraction, recipes, item inputs/outputs |
| Future | Maintain Stock, settlement AI, hauling, logistics, power, tools, skills |

## Rejected models

- Worker task directly creates items
- Separate Workstation runtime object
- One building with unrelated anvil/furnace/oven operation slots
- Progress stored only on workers
- Temporary production runtime intended for later replacement
- Authoritative production in a Bevy `Resource` omitted from save

## Consequences

- Future production buildings plug into the same runtime without redesign.
- EP3 adds catalog resolution against `OperationDefinitionId` without changing store layout.
- EP4 adds inventory role bindings without hardcoded input/output fields on operation state.
- Building removal must call `BuildingProductionStore::remove` (already wired in destroy/remove paths).
