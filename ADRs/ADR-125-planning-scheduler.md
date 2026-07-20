# ADR-125: Planning Scheduler & Incremental Evaluation (SA10)

## Status

Accepted (architecture lock — implementation follows this ADR)

## Context

SA1–SA9 delivered the Settlement AI decision pipeline as independent dirty+cadence step loops.
ADR-115 §10 already required a runtime scheduler (staggered intervals, event-driven dirty flags);
that layer was deferred while stages were built. Without it, every stage scans all settlements each
tick, coarse dirty fan-out is common, and mutation→dirty wiring is incomplete.

This ADR locks scheduling ownership and invalidation rules **before** implementation so later work
does not move planning logic into the scheduler or introduce feedback loops.

## Decision

### Runtime pipeline order (authoritative)

```
Emergency Evaluation (SA8)
    ↓
Need Evaluation (SA2)
    ↓
Response Discovery (SA3)
    ↓
Response Arbitration (SA4)
    ↓
Building Intent Propagation (SA5)
    ↓
Production Planner (EP9)   — peer policy writer; scheduled with SA stages
    ↓
Construction Planning (SA9)
    ↓
Strategic Task Generation (SA6)
    ↓
Worker Assignment (SA7)    — marketplace cadence; consumes tasks, never plans
```

**ConstructionPlans must exist before Strategic Task Generation** so SA6 can emit construct tasks
against reserved `Planned` sites. Any prompt that places construction after task generation is
incorrect for this codebase (see ADR-124).

### Scheduler ownership (when, not what)

The Planning Scheduler determines **when** planner stages execute.

- It owns evaluation cadence, dirty aggregation, priority queues, planning budgets, and optional
  incremental/time-sliced stage dispatch.
- Planner stages remain responsible for their **own evaluation logic**.
- The scheduler **never performs planning itself**. It only orchestrates execution of existing
  stage APIs (e.g. `*_now` / per-settlement evaluators).

Rejected drift: moving need scoring, arbitration, placement search, or policy writes into the
scheduler module.

### Event-driven with time fallback

Planning is event-driven. Time (fallback cadence) wakes settlements only when events have not.

Individual planner stages must not independently poll every frame; the scheduler owns due-checks.

### Dirty flags

Support directional dirty bits for (at least): Needs, Responses, Intent, Buildings/Policies,
Construction, Emergencies, Capabilities, Settlement metadata. Multiple bits may be set at once.

### Dependency graph

Changes invalidate only **downstream** systems. Example: inventory → Needs → Responses → Intent →
Building Policies → Construction (only if capacity-relevant) → Strategic Tasks. Avoid invalidating
unrelated stages.

### Invalidation rules (strict)

Planner invalidation is **strictly downstream**.

- Earlier stages may invalidate later stages.
- Later stages must **never** invalidate earlier stages.

| Stage | May invalidate |
|---|---|
| Emergency Evaluation | Needs, Responses (and downstream as needed) |
| Need Evaluation | Responses |
| Responses | Intent |
| Intent | Building Policies, Construction (when construct intents), Strategic Tasks |
| Building Policies | Construction (capacity/policy-relevant), EP9 as appropriate |
| Construction | Strategic Tasks |
| Strategic Tasks | Worker Assignment (marketplace wake only) |
| Worker Assignment | **nothing** in the planning pipeline |

Worker Assignment must never invalidate planning. This preserves a deterministic pipeline and
prevents circular invalidation.

Emergencies sit **before** Needs and may dirty Needs/Responses; that is upstream injection into the
chain, not a later stage looping back.

### Planning budgets

Configurable limits such as: max settlements evaluated per tick, max planner stages per tick,
max construction searches, max response evaluations. Large worlds must remain stable.

### Time slicing

Large planning work may continue across updates. Stage evaluators should remain restartable.
v1 may schedule whole stages per settlement; multi-frame resumable internals are seams, not a
requirement to rewrite every evaluator immediately.

### Priority

Urgent settlements evaluate before idle ones (e.g. emergency, dirty, recently modified, player,
active construction).

### Player support

Player and AI settlements use the same scheduler. Manual player actions create planner invalidations
only — not a parallel pipeline.

### Save / load

Persist only authoritative scheduling fields where necessary (e.g. lifecycle cadence on
`SettlementState`). Rebuild transient queues and dirty bits after load.

### Dev Mode

Surface: planner queue, dirty flags, pending stages, evaluation timing, skipped evaluations,
budget usage.

## Rejected designs

- Every planner polling every frame independently
- Entire settlement replanning every update
- Independent per-stage scheduling that bypasses the central scheduler
- Scheduler owning planning/evaluation logic
- Upstream invalidation from Worker Assignment or task execution into Needs/Intent
- Construction Planning after Strategic Task Generation

## Consequences

- Amend ADR-115 phase map: **SA10 = Planning Scheduler**; Expansion / Growth shifts later
- Implementation lives in `src/world/settlement/scheduler/` (or equivalent) and thins `tick.rs`
- Existing stage modules keep their evaluation code; step loops become scheduler-driven
- Wire mutation paths to directional dirty marks (inventory, building complete/destroy, policy)
