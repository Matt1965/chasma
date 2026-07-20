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

## References

- ADR-115, ADR-116, ADR-117, ADR-118, ADR-114
- ARCHITECTURE.md Settlement AI section
