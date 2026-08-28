# ADR-119: Settlement Response Arbiter (SA4)

## Status

Accepted

## Context

ADR-117 computes need pressures. ADR-118 discovers and scores `CandidateResponse` options. Neither
decides what the settlement will pursue.

SA4 introduces the **Settlement Response Arbiter**: a generic evaluate → rank → select loop that
produces **SettlementIntent** — strategic wishes without execution.

## Decision

### SettlementIntent is transient strategic output

`SettlementIntent` answers:

> The settlement currently wishes to pursue these responses.

Each intent carries: `IntentId`, source need, chosen response, arbitration priority, desired
persistence metadata, reasoning, diagnostics, and future AI seams.

`SettlementIntentPlan` holds chosen intents (priority-ordered), rejected candidates with reasons,
and plan diagnostics. Results live in `SettlementIntentStore` on `WorldData`.

**Nothing is serialized.** Plans rebuild after load.

### Multi-response arbitration

The arbiter does **not** pick a single response. It may pursue several simultaneously under budgets:

- Global cap (`MAX_SETTLEMENT_INTENTS`)
- Per-need slots (2 when pressure ≥ 40, else 1)
- Soft conflict rule: `IncreaseProduction` vs `DecreaseProduction` for the same need

Selection considers: candidate score, need pressure, policies, availability, workload proxy,
emergency modifiers. Unavailable / below-threshold candidates are recorded as rejected for Dev Mode.

### No execution

SettlementIntent never:

- Creates buildings
- Changes `BuildingOperationPolicy`
- Creates tasks / assigns workers
- Moves items / produces logistics

Execution belongs to later SA phases.

### Event model

`step_settlement_response_arbitration` replans when:

- Intent store dirty (settlement dirty / policy / emergency seams)
- Source response-candidate tick changed
- Source need-evaluation tick changed
- Cadence (`INTENT_ARBITRATION_CADENCE_TICKS`) expires

Avoids continuous every-frame planning.

### Validation

Rejects duplicate intent ids, unknown responses, non-finite priorities, increase/decrease conflicts,
and broken references.

## Rejected designs

- **Single-response AI** — settlements pursue multiple intents under budget.
- **Worker-driven planning** — workers remain below task assignment; they do not plan strategy.
- **Building-driven planning** — buildings report capabilities; strategy lives on the settlement.
- **Persisting intent** — violates rebuild principle; intent is derived from needs + candidates.

## Consequences

- Dev inspector shows pressures, candidates, chosen/rejected intents, priority order, diagnostics.
- SA5 Building Intent Propagation (ADR-120) consumes `SettlementIntentPlan` into
  `BuildingOperationPolicy`; SA6 Strategic Task Generation (ADR-121) contributes strategic Tasks —
  neither invents parallel selection.
- Directives (player/faction weight nudges) remain a later seam that adjusts inputs, not this layer.

## Amendment (2026-08-28) — SA4 scores the need + response pairing

### Why

ADR-118's original SA3 formula already contained need pressure, and `arbitration_score` added
`pressure * 2.0` on top of it. Pressure was counted twice, and because the SA3 term spanned
`0..=10_000` while policy bonuses spanned ±20 and the workload penalty capped at 40, **policy,
workload, and cost could not affect any ordering**. See ADR-118's amendment for the full analysis.

### Decision

SA3 now scores **response quality** only. SA4 scores the **pairing**:

> Which need is most urgent, and what is the best available response for it?

SA4 combines:

- **Urgency** — need pressure, shaped by the authored need weight (`NeedTarget.weight`, stored since
  ADR-116 and previously unused)
- **Candidate quality** — the SA3 response score
- **Settlement policy** — applied here and **only** here
- **Workload proxy** — soft penalty, unchanged in intent

Pressure enters the pipeline exactly **once**, in SA4.

### Authored weight becomes effective

`NeedTarget.weight` shapes urgency, which makes it possible to author that a settlement cares more
about one need than another at equal pressure. Weight is a settlement-level authoring lever, not a
per-response tuning knob, and it is the mechanism directives (§5 of ADR-115) will eventually nudge.

### Component scale

All components — urgency, candidate quality, policy, workload — must be scaled so each can
realistically change an ordering. No single term may dominate the others by orders of magnitude.

### Explicitly preserved

These behaviors are load-bearing and must survive the rescale:

| Behavior | Why it matters |
|---|---|
| `pressure == 0` → `ZeroPressure` rejection | This is what makes a settlement **stop** pursuing a satisfied need |
| Unavailable candidates score `0` | Availability is a gate, not a penalty |
| `MIN_ARBITRATION_SCORE` threshold | Filters noise into rejected-with-reason diagnostics |
| `IntentPersistence::UntilPressureLow` above pressure 80 | Existing intent hysteresis; prevents per-cadence flapping |
| Global cap, per-need slots, increase/decrease conflict rule | Multi-intent budgeting is unchanged |

### Explanation comes from the scorer

`arbitration_score` emits its own component breakdown. The reasoning string and the Dev inspector
render what the scorer produced; nothing recomputes a parallel explanation.

## References

- ADR-115, ADR-116, ADR-117, ADR-118, ADR-114
- ADR-132 (single-calculation-path discipline)
- ADR-133, ADR-134
- ARCHITECTURE.md Settlement AI section
