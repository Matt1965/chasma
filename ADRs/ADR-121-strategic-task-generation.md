# ADR-121: Strategic Task Generation (SA6)

## Status

Accepted

## Context

ADR-119 produces transient `SettlementIntent`. ADR-120 propagates production intents into
`BuildingOperationPolicy`. Construction, repair, recruit, and expand intents were deferred in SA5
with diagnostics only — no tasks were created.

Workers already consume tasks from the shared `TaskStore` (ADR-085). Settlement AI must contribute
strategic work into that store without owning tasks or assigning workers.

## Decision

### Ownership

```
SettlementIntent (strategy)
    ↓ Strategic Task Generation (SA6)
TaskStore / TaskRecord (WorldData-owned)
    ↓ later Worker Assignment (SA7)
Workers (execution)
```

Tasks remain owned by `WorldData`. Settlement AI inserts, refreshes, and cancels Available strategic
tasks. It never owns the store and never assigns units.

### Authored mapping

Intent → task uses **`StrategicTaskTemplate`** entries in `StrategicTaskTemplateCatalog`:

- match by `ResponseId` (preferred) or `ResponseType`
- emit a `TaskType` (ConstructBuilding, StrategicConstruct, RepairBuilding, …)

Never hardcode Need → Building (e.g. Food → Farm). Food construction is authored as
`construct_food_building` → template `construct_food_building`.

### What SA6 emits

Strategic / structural work only:

- Construct Building (sites via existing `ConstructBuilding`, or `StrategicConstruct` on anchor)
- Repair Building, Clear Rubble, Recruit Worker, Expand Storage

**Does not emit** production OperateWorkstation, Haul, crafting, or mining tasks — those remain
owned by production/logistics runtimes and SA5 policy.

### Merge / cancel

- Merge key: settlement + template + response + building + task type
- Refresh priority and origin on match; keep assignment if present
- Cancel only `Available` strategic tasks when no longer desired (Assigned/InProgress continue)

### Priority

```
SettlementIntent.priority → Need/Response pressure chain → TaskPriority
```

Mapped to High / Normal / Low. Never `PlayerAssigned`.

### Lifecycle / events

Regenerate on SettlementIntent change, settlement dirty (buildings/construction), and cadence.
Transient `StrategicTaskGenerationReport` is never persisted. Authoritative tasks (including
optional `StrategicTaskOrigin`) persist with the scene; reports rebuild after load.

### Persistence

Persist tasks only. Clear the strategic generation store on scene restore; regenerate after SA4
intent rebuilds.

## Rejected designs

- **Settlement directly controlling workers** — workers stay below the Assignment layer (SA7).
- **Planner-owned tasks** — EP9/SA5 write policy; SA6 contributes to the shared TaskStore only.
- **Hardcoded Need → Build X branches** — catalog templates only.

## Consequences

- Dev inspector shows generated tasks, priority, source intent/response/template, and cancel diag.
- SA7 (ADR-122) assigns free workers to Available executable tasks; strategic stubs await labor.
- Validation rejects duplicate emissions, unknown templates, PlayerAssigned priority, and
  production/haul emissions from SA6.

## References

- ADR-115, ADR-119, ADR-120, ADR-085, ADR-107–114
- ARCHITECTURE.md Settlement AI section
