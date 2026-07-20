# ADR-118: Response Engine (SA3)

## Status

Accepted

## Context

ADR-115 defined Settlement AI around a weighted-need arbiter that applies data-defined Responses.
ADR-117 (SA2) computes need pressures but does not know what options exist.

Before an arbiter can choose, the settlement needs a generic layer that answers:

> Given pressure, what possible responses exist, and how attractive is each?

That layer is the **Response Engine**. It sits between Need Evaluation and future planning/selection.
It does not decide workers, generate tasks, or execute anything.

## Decision

### Responses are authored data

`ResponseDefinition` entries live in `ResponseCatalog`. Each definition carries:

- Stable `ResponseId` and display metadata
- `supported_need_ids` (discovery key — needs never list responses)
- `ResponseType` (IncreaseProduction, ConstructBuilding, Defend, Trade, …)
- `ExpectedEffect` (pressure relief + estimated cost)
- Priority modifiers, capability requirements, prerequisite ids, AI tags

Needs never know responses. Responses never know workers. Buildings never know strategy.

### Discovery is catalog-driven

```
NeedSnapshot
  → ResponseCatalog.definitions_for_need(need_id)
  → validate capability / policy / pressure
  → CandidateResponse (scored)
```

There is no runtime `if Food { build Farm }` branch. Food pressure surfaces every response whose
authored `supported_need_ids` includes `food` (farm ops, bakery, trade, construct, …). New options
are data additions.

### CandidateResponse is transient

Each candidate includes response/need ids, expected impact, estimated cost, availability, blocking
reason, priority score, supporting buildings, and diagnostics. Results live in
`ResponseCandidateStore` on `WorldData` and are never persisted.

### Scoring is intentionally simple

```
score = pressure * relief * 100 - estimated_cost + priority_modifier + policy_bonus
```

Unavailable candidates score `0`. Future modifiers can extend `score_candidate` without changing
discovery.

### No execution

The Response Engine never:

- Generates tasks
- Changes `BuildingOperationPolicy`
- Creates buildings
- Moves workers
- Changes inventories

It only evaluates options.

### Event model

`step_settlement_response_discovery` rebuilds when:

- Response store dirty (settlement dirty / capability change seams)
- Source need evaluation tick changed
- Cadence (`RESPONSE_DISCOVERY_CADENCE_TICKS`) expires

Need evaluation marks the response store dirty after recomputing snapshots.

### Validation

Catalog construction rejects duplicate ResponseIds, empty supported needs, invalid effects,
malformed capability refs, unknown prerequisites, and circular prerequisite graphs. Optional check
against `NeedCatalog` rejects unknown NeedIds.

## Rejected designs

- **Need-specific runtime code** — discovery is always catalog lookup by NeedId.
- **Response-specific worker logic** — workers remain below Response / Task layers.
- **Hardcoded production decisions** — EP9 remains a separate production planner; SA3 only scores
  options. Selection/application is a later phase.
- **Persisting candidates** — violates rebuild principle.

## Consequences

- Dev inspector shows need → candidates with score / availability / blocking / impact.
- SA4 Response Arbiter (ADR-119) consumes `CandidateResponse` scores into `SettlementIntent`.
- ADR-115 phase map: SA3 Response Engine → SA4 Arbiter (intent) → later apply/directives.

## References

- ADR-115, ADR-116, ADR-117, ADR-114
- ARCHITECTURE.md Settlement AI section
