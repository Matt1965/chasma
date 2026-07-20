# ADR-124: Strategic Construction Planning (SA9)

## Status

Accepted

## Context

ADR-115 defines Settlement AI as Need → Response → Intent → execution layers. Construction must
participate as one response family — not a parallel settlement brain. SA5 defers construct intents;
SA6 can attach ConstructBuilding tasks to existing incomplete sites but does not choose *what* to
build or *where*.

## Decision

### Flow (locked)

```
NeedSnapshot
  ↓
CandidateResponse
  ↓
SettlementIntent
  ↓
Construction Response Mapping (authored)
  ↓
ConstructionPlan (authoritative, persisted)
  ↓
Construction Tasks (SA6 / existing construct runtime)
  ↓
Workers and Logistics
```

`SettlementIntent` remains transient. `ConstructionPlan` is authoritative once the settlement commits
to building. Brief need-pressure dips do not cancel committed plans.

### Capability-based mapping

Authored `ConstructionResponseMapping` selects buildings via capability (supporting operation,
category, or explicit allow-list) — never hardcoded Need→Building branches.

### Capacity gap

Planning estimates existing + planned capable capacity against a target before creating new plans.

### Placement

Bounded search near the settlement anchor. Hard validity uses existing placement validation.
Soft preference ranks valid sites only. Site commit places a `Planned` building (occupancy =
spatial reservation) and links it to the settlement.

### Player / AI

One `ConstructionPlan` model. Policies control autonomy, approval, concurrency, and search budgets.

### Non-goals

Boundary expansion, demolition, upgrades, roads/districts, advanced city optimization, spawning
Complete buildings, assigning workers, or moving materials from the planner.

## Rejected designs

- Hardcoded Need-to-Building mappings
- Construction directly from NeedSnapshot
- Planner spawning completed buildings
- Separate player construction pipeline
- Unbounded world-wide placement scanning
- Cancelling plans whenever pressure declines
- Buildings choosing their own strategic construction
- Workers choosing where buildings should be placed

## Consequences

- Scene version 14 persists `ConstructionPlan` records
- Tick order: SA4 → SA5 → SA9 → SA6 → SA7
- Dev inspector surfaces plan lifecycle, capacity notes, candidate scores, and rejected sites
