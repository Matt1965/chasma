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
  → validate capability / prerequisites
  → CandidateResponse (scored for response quality — see amendment)
```

> **AMENDED 2026-08-28.** Discovery still uses the need id. It must **not** fold need pressure or
> settlement policy into SA3 scoring. Policy and urgency belong to SA4 (ADR-119 amendment).

There is no runtime `if Food { build Farm }` branch. Food pressure surfaces every response whose
authored `supported_need_ids` includes `food` (farm ops, bakery, trade, construct, …). New options
are data additions.

### CandidateResponse is transient

Each candidate includes response/need ids, expected impact, estimated cost, availability, blocking
reason, priority score, supporting buildings, and diagnostics. Results live in
`ResponseCandidateStore` on `WorldData` and are never persisted.

### Scoring is intentionally simple

> **SUPERSEDED 2026-08-28** — see [Amendment: SA3 scores response quality](#amendment-2026-08-28--sa3-scores-response-quality)
> below. The formula recorded here counted need pressure inside SA3, which SA4 then counted again.
> Retained as historical record; **do not implement this formula.**

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
- **Hardcoded production decisions** — SA3 only scores options. Selection is SA4; application is SA5.
  EP9 is a production-graph service invoked through settlement intent (ADR-114 / ADR-120 amendments),
  not a second decision engine.
- **Persisting candidates** — violates rebuild principle.

## Consequences

- Dev inspector shows need → candidates with score / availability / blocking / impact.
- SA4 Response Arbiter (ADR-119) consumes `CandidateResponse` scores into `SettlementIntent`.
- ADR-115 phase map: SA3 Response Engine → SA4 Arbiter (intent) → later apply/directives.

## Amendment (2026-08-28) — SA3 scores response quality

### Why

The original formula multiplied need pressure by relief and by 100, producing a term in the range
`0..=10_000`. SA4 (ADR-119) then added pressure a second time. Two consequences:

1. **Pressure was counted twice** across two stages, once multiplicatively and once additively.
2. **Every other input became inert.** Estimated cost, priority modifiers, policy bonuses (±5–20), and
   SA4's workload penalty (≤40) cannot influence an ordering dominated by a 10,000-wide term. In
   practice pressure alone decided outcomes, and an authored need weight would have been equally
   powerless.

Both files already carried explicit comments forbidding double-counting of *emergency* effects, so the
hazard was understood; pressure itself simply escaped the same scrutiny.

### Decision

**Need pressure does not belong in SA3.** SA3 answers one question only:

> How good is this response, if the settlement chooses to act on this need?

SA3 therefore scores **response-intrinsic factors only**: expected pressure relief, estimated cost,
capability availability, and authored priority modifiers. It does not read `NeedSnapshot.pressure`, and
it does not apply settlement policy — policy is applied exactly once, in SA4.

Urgency (pressure, and the authored need weight that shapes it) belongs to the need, and therefore
belongs to selection in SA4.

### Preserved

- Unavailable candidates score `0`.
- Discovery remains catalog lookup by `NeedId`; scoring changes do not touch discovery.
- Candidates remain transient and unpersisted.

### Component scale

Remaining components must be scaled so each can realistically change an ordering. A component that
cannot alter a decision at any plausible input is not a factor — it is decoration, and it should
either be scaled to matter or removed.

### Explanation comes from the scorer

`score_candidate` emits its own component breakdown alongside the score. Diagnostics render what the
scorer produced; no consumer recomputes a parallel explanation. This is the same discipline ADR-132
imposes on relationship values.

## References

- ADR-115, ADR-116, ADR-117, ADR-114
- ADR-119 (arbiter — receives urgency and applies policy), ADR-132 (single-calculation-path discipline)
- ADR-133, ADR-134
- ARCHITECTURE.md Settlement AI section
