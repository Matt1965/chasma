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
- Player reclaim (`PlayerControlled && planner_managed`) remains respected.

## References

- ADR-115, ADR-118, ADR-119, ADR-114, ADR-107/EP2
- ARCHITECTURE.md Settlement AI section
