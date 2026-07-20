# ADR-117: Need Evaluation Runtime (SA2)

## Status

Accepted

## Context

ADR-115 defined Settlement AI around a weighted-need arbiter. ADR-116 (SA1) introduced persistent
`SettlementState` (targets, modifiers, policy) without computing anything.

SA2 teaches a settlement how to **evaluate itself**: compute current values, desired values, and
normalized pressure for each need. It does not decide actions, generate tasks, or mutate production.

## Decision

### Needs are computed, never authoritative persistent objects

`NeedDefinition` entries are authored catalog content (`NeedCatalog`). Runtime results are
`NeedSnapshot` values held in a transient `NeedEvaluationStore` on `WorldData`.

Nothing produced by Need Evaluation is persisted. After save/load the store is cleared and snapshots
rebuild on the next evaluation.

### NeedSnapshot is the evaluation output

Each snapshot carries:

- NeedId
- Current / desired values
- Deficit / surplus
- Normalized pressure `0..=100`
- Optional blocking reason
- Trend seam (unused in SA2)
- Evaluation tick + source diagnostic string

Snapshots are rebuilt whenever evaluation runs.

### Pressure is the universal output

```
pressure = clamp(round((max(0, desired - current) / desired) * 100), 0, 100)
```

When `desired <= 0`, pressure is `0`. Settlement modifiers (matching need id or `"all"`) and matching
emergency flags may adjust pressure within `0..=100`. Future systems consume pressure only — never
raw inventory counts.

### Independent evaluation

Each need computes Current → Desired → Pressure independently. No need inspects another need. No need
generates actions or mutates SettlementState / inventories / buildings / workers.

### Evaluation cadence

`step_settlement_need_evaluation` runs during the simulation tick (before EP9 production planners)
when:

- the settlement's need-store dirty flag is set, or
- no prior snapshot exists, or
- `NEED_EVAL_CADENCE_TICKS` (30) have elapsed since the last evaluation

Dirty hints come from `mark_settlement_state_dirty` (inventory/building/policy invalidation seams).
Need dirty lives on `NeedEvaluationStore` so evaluation never clears EP9 `planner.dirty`.

### First needs (architecture exercise only)

Food, Construction, Housing, Defense, Research, Expansion, Luxury — measurement stubs sufficient to
exercise the catalog/snapshot/pressure path. No Response behaviors.

### Validation

Catalog construction rejects duplicate NeedIds and unknown evaluators. Snapshot validation rejects
pressure outside `0..=100`, non-finite values, negative desired, and broken deficit accounting.

## Rejected designs

- **Persistent Need objects** — pressures are derived; persisting them violates the rebuild principle.
- **Needs generating actions** — SA2 reports state only; response selection is SA3+.
- **Cross-dependent Need calculations** — each need is independent; coupling belongs in a later arbiter
  that reads pressures, not inside evaluators.
- **Clearing SettlementState/EP9 dirty from need eval** — need dirty is a separate transient flag.

## Consequences

- Dev inspector shows need current/target/pressure/modifiers/source/diagnostics.
- SA3 Response Engine (ADR-118) reads pressures from `NeedEvaluationStore` and produces
  `CandidateResponse` options — it does not invent parallel sensors.
- Scene restore clears `NeedEvaluationStore`.

## References

- ADR-115, ADR-116, ADR-114
- ARCHITECTURE.md Settlement AI section
