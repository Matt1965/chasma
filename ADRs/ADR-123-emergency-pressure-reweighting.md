# ADR-123: Emergency Pressure & Priority Reweighting (SA8)

## Status

Accepted

## Context

ADR-115 requires emergencies to be priority inputs into the existing Need → Response → Intent →
Task → Worker pipeline — never a parallel AI. SA1 stored emergency flags; SA2/SA4 had hardcoded
bumps that double-counted. SA7 interruption existed without settlement-aware emergency policy.

## Decision

### Flow (locked)

```
Observed World State
    ↓ Emergency Evaluation (this ADR)
Need Pressure Modifiers
    ↓
Response Arbitration
    ↓
SettlementIntent
    ↓
Building Policies and Tasks
    ↓
Worker Assignment
```

Emergencies never assign workers, move units, enable named buildings, or mutate inventories.

### Authored `EmergencyDefinition`

Catalog-driven definitions include: `EmergencyId`, evaluator reference, activation/deactivation
thresholds, minimum active duration, recovery delay, need-pressure modifiers, response score
modifiers, unlock/block lists, interruption policy, task-priority bump flags.

Detection uses evaluator kinds (food reserve ratio, hostile/fire/evacuate seams) — not
emergency-name branches in pressure/score code.

### Persistent `SettlementEmergencyState`

Authoritative `ActiveEmergencyInstance` records (id, severity, activated_tick, manual force/suppress,
acknowledged, source). Legacy boolean flags remain for scene compatibility and are synced from
instances. Computed evaluation reports are never persisted.

### Severity and hysteresis

Signals and severity are normalized `0..=1`. Activation and deactivation thresholds differ.
Minimum active duration (and optional recovery delay) prevent rapid toggling.

### Priority reweighting (no double-count)

| Layer | Emergency effect |
|-------|------------------|
| SA2 Need | Authored pressure deltas × severity |
| SA3 Response | Authored score deltas; unlock/block |
| SA4 Arbiter | **None** (uses already-adjusted pressure/scores) |
| SA6 Tasks | Optional one-tier bump from authored tags |
| SA7 Assignment | Interruption relaxation when policy allows |

### Player control

Same runtime for player and AI. Policies: `auto_emergency_response`,
`auto_production_reprioritize`, `auto_task_interruption`, plus per-instance manual force/suppress.

### Initial vertical slices

Starvation, Active Attack, Critical Fire, Evacuation — using inventory ratios and
`extension_seams` signals. No new combat/fire/evacuation simulation.

## Rejected designs

- Separate emergency AI controller / behavior trees
- Emergencies directly assigning workers or enabling named buildings
- Binary toggles without hysteresis
- Duplicating the same emergency bonus at every planning layer

## Consequences

- Tick order: SA8 emergency eval → SA2 → SA3 → …
- Dev inspector shows instances, signals, thresholds, policies, modifiers
- ADR-115 phase map: SA8 = Emergencies (this ADR); construction response remains later

## References

- ADR-115 §9, ADR-116, ADR-117, ADR-118, ADR-119, ADR-121, ADR-122
- ARCHITECTURE.md Settlement AI section
