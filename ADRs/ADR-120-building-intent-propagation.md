# ADR-120: Building Intent Propagation (SA5)

## Status

Accepted

## Context

ADR-119 produces transient `SettlementIntent` — strategic wishes without world mutation. EP9 already
writes `BuildingOperationPolicy` from stock goals. SA5 is the first Settlement AI phase that
influences the simulation by propagating intent downward into building policies.

## Decision

### Downward authority

```
SettlementIntent (strategy)
    ↓ Building Intent Propagation
BuildingOperationPolicy (building intent)
    ↓ later task generation
Workers (execution)
```

Settlements choose intent. Buildings receive policy. Workers never see settlement strategy.

### Capability-based discovery

Capable buildings are found via authored `supported_operations` matching response
`CapabilityRequirement::SupportingOperation` — never by building display names.

### Policy-only writes

Allowed mutations on `BuildingOperationPolicy`:

- enable / disable
- priority
- repeat mode (`Continuous`)
- selected operation
- `planner_managed` + `control_source` (AI ownership)

**Never** mutate `BuildingOperationState` (lifecycle, progress, workers, blocked reason).

### Distribution and conflicts

Multiple buildings may share a capability. Propagation selects a small number per intent
(`MAX_BUILDINGS_PER_INTENT_*`) by deterministic building-id order. Higher-priority intents claim
buildings first. Non-production intents (construct/trade/defend/…) are deferred with diagnostics —
no construction plans in SA5.

### EP9 coexistence

> **SUPERSEDED 2026-08-28** — see [Amendment: SA5 is the sole AI policy writer](#amendment-2026-08-28--sa5-is-the-sole-ai-policy-writer).
> Dual-writer skip-via-`planner_managed` is retired. EP9 is invoked as a service; SA5 writes.

Buildings assigned by SA5 are skipped by EP9 `apply_planner_decisions` and
`disable_unselected_planner_buildings`, so SettlementIntent remains authority for those policies.

### Persistence

`BuildingOperationPolicy` persists as usual (scene production snapshot). Propagation reports and
assignment indexes are transient and rebuild after load.

## Rejected designs

- **Buildings choosing strategy** — buildings only advertise capabilities.
- **Workers enabling buildings** — workers remain below task assignment.
- **Planner modifying runtime state** — EP9 and SA5 write policy only; state is execution truth.
- **Hardcoded farm/quarry/lab name branches** — catalog operations drive discovery.

## Consequences

- Dev inspector shows assignments, ignored buildings, deferred intents, selection reasons.
- SA6 (ADR-121) consumes deferred construct/repair/recruit/expand intents as strategic tasks.
- Future construction response phases refine site placement; they do not invent parallel enable paths.
- Player reclaim (`PlayerControlled`) remains respected; SA5 must not overwrite it.

## Amendment (2026-08-28) — SA5 is the sole AI policy writer

### Why

ADR-114's EP9 planner and this phase both wrote `BuildingOperationPolicy`. Coexistence was implemented
as "EP9 skips buildings SA5 claimed." That workaround is the dual-authority defect, not a design.

### Decision

- **SA5 is the sole AI writer of `BuildingOperationPolicy`.**
- For production intents, SA5 **invokes EP9 as a service** (graph + producer discovery + recommended
  policy) and then writes. EP9 does not apply those recommendations itself for AI settlements.
- Non-production intents (construct / trade / defend / …) remain deferred from this phase with
  diagnostics — SA5 still does not create construction plans or tasks.
- The `planner_managed` dual-writer workaround is removed where it is no longer necessary. Do not
  keep a skip-list between two writers.
- **`ControlSource::PlayerControlled` remains a hard skip.** SA5 may not overwrite explicit player
  policy. Player reclaim is permission, not a second AI planner.

### Tick consequence

`run_simulation_tick` must not treat EP9 as a peer stage that writes policy after SA5. Production
graph work happens *through* SA5 when a production intent is held.

### Preserved

- Capability-based discovery (`supported_operations`), never building display names.
- Policy-only writes; never `BuildingOperationState`.
- Propagation reports remain transient; policy persists.

## References

- ADR-115, ADR-118, ADR-119, ADR-114, ADR-107/EP2, ADR-133
- ARCHITECTURE.md Settlement AI section
