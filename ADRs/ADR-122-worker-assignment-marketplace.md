# ADR-122: Worker Assignment — Task Marketplace (SA7)

## Status

Accepted

## Context

ADR-121 places strategic (and construction) work into the shared `TaskStore`. Workers still only
received tasks through imperative player/UI `assign_*` APIs. Settlement AI must never select
individual workers; workers must discover appropriate Tasks autonomously.

## Decision

### Authority chain

```
SettlementIntent → Tasks (marketplace)
       ↓
Worker Assignment (this ADR)
       ↓
Workers execute
```

Settlement decides **what** should happen. The Task system exposes **what work exists**. Workers
decide **who does the work**.

### Task marketplace

`TaskStore` (plus open hauling requests) is a shared marketplace. Idle workers evaluate listings by:

- Task priority
- Distance
- Capability (`UnitWorkCapabilities`)
- Ownership / affiliation
- Free interaction-point capacity
- Current reservations / stick hysteresis

Future seams (not in SA7): skill, morale, preferences, professions (ADR-072).

### Assignment ownership

The Assignment layer lives under the task system (`task/marketplace`). It reuses:

- Interaction-point reservations on `TaskStore`
- Inventory reservations for haul (`assign_hauling_task_with_priority`)
- `claim_building_task` for construct/operate

It never invents a parallel reservation model.

### What is listed

- Available / open-slot `ConstructBuilding` tasks
- Available `OperateWorkstation` tasks (synced from enabled building policies)
- Pending / unassigned open hauling requests

Strategic stub kinds without execution (`StrategicConstruct`, Repair, Recruit, …) remain unlisted
until a later phase provides labor.

### Interruption and starvation avoidance

Higher-priority listings may preempt lower-priority work via
`release_unit_task_to_marketplace` (task returns to Available — not Canceled).

Hysteresis:

- Minimum stick ticks before preemption
- Priority-rank gap threshold
- Post-preempt cooldown

### Persistence

Authoritative tasks, assignments, and IP reservations persist with the scene. Marketplace reports and
stick clocks are transient and rebuild. Idle workers with persisted assignments are resumed by
re-issuing `UnitOrder::Work`.

### Validation

Detect double assignment, broken reservations, invalid capabilities, and dead workers holding tasks.
Dead workers are released each assignment step.

## Rejected designs

- **Settlement selecting workers** — violates marketplace autonomy.
- **Buildings selecting workers** — buildings expose work; they do not hire.
- **Worker-specific scripts** — no per-profession imperative AI in SA7.
- **Cancel-on-preempt** — would destroy recoverable marketplace listings.

## Consequences

- Tick order: SA6 → SA7 assignment → worker/haul labor → movement.
- Dev inspector shows evaluations, candidates, chosen task, score, reservation.
- Player `assign_*` remains valid and still uses `TaskPriority::PlayerAssigned` (immune to preempt).

## References

- ADR-115, ADR-121, ADR-085, ADR-072, ADR-112
- ARCHITECTURE.md Settlement AI section
