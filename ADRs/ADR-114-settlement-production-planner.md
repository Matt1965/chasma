# ADR-114: Settlement Production Planner (EP9)

## Status

Accepted

## Context

EP1–EP8 established building production runtime, operation catalog, role-tagged inventories,
generic execution, terrain extraction, hauling logistics, and multi-building production chains.
Buildings own production; workers execute tasks; logistics moves items. No system yet decides
**what** the settlement should produce at a global level.

ADR-072 describes settlement automation philosophy. EP9 implements the production planning seam
without trading, markets, worker assignment, or production optimization.

## Decision

### Planner owns intent only

Each settlement has one authoritative `SettlementProductionPlanner` on `WorldData` via
`ProductionPlannerStore`. The planner:

- Reads settlement stock from storage buildings advertising supply
- Compares current stock to authored `StockGoal` targets
- Propagates demand through a derived production graph from `OperationCatalog`
- Updates `BuildingOperationPolicy` (enable, operation, priority, repeat, control source)

The planner never:

- Executes production (`execute_production_cycle`)
- Moves items (logistics runtime)
- Controls workers (`TaskType` assignment)
- Mutates `BuildingOperationState`

### Stock goals belong to the settlement

`StockGoal` records desired `maintain_quantity`, optional `export_threshold`, and
`ProductionPriorityCategory`. Goals are persisted; derived graphs and diagnostics are not.

### Production graph is derived, not authored

`ProductionGraph::from_catalog` builds item dependency edges from `OperationDefinition`
inputs/outputs. Demand propagates recursively. Cycle detection rejects circular recipes.

### Building enablement via policy

> **AMENDED 2026-08-28** — see [Amendment: EP9 becomes a production-planning service](#amendment-2026-08-28--ep9-becomes-a-production-planning-service).
> EP9 no longer writes `BuildingOperationPolicy` on its own authority for AI-driven settlements.

When demand exists, the planner enables operational buildings whose `supported_operations`
produce demanded items. When goals are satisfied, planner-managed buildings are disabled.
`planner_managed` and `ControlSource::AIControlled` mark planner ownership; player-controlled
buildings are skipped.

### Settlement building membership

> **SUPERSEDED 2026-08-28** by [ADR-133](ADR-133-settlement-identity-membership-and-anchor.md).
> `BuildingRecord.settlement_id` is the sole membership authority; affiliation is not membership.
> `reconcile_settlement_building_membership` is retired.

`SettlementStore` tracks building membership for inventory aggregation and producer discovery.
`reconcile_settlement_building_membership` links buildings sharing settlement affiliation
(scene restore and dev harness).

### Replanning

`step_settlement_production_planners` runs before worker tasks when `BuildingOperationParams`
is available. Replans on dirty flag or `replan_interval_ticks` (default 60), not every frame.

## Rejected designs

- Buildings deciding global production independently
- Workers or tasks owning production intent
- Workers scanning global inventories for planning
- Hardcoded production chains (iron mine → smelter, etc.)
- Persisting derived graphs or planner caches

## Consequences

- Player/dev can author stock goals; settlement enables mines, smelters, bakeries as needed
- Logistics and production runtimes unchanged; planner feeds policy intent upstream
- Dev inspector shows planner diagnostics; Shift+P force replans
- Scene format v12 persists `ProductionPlannerSaveState`

## Amendment (2026-08-28) — EP9 becomes a production-planning service

### Why

SA5 (ADR-120) and EP9 both wrote `BuildingOperationPolicy`. EP9 avoided buildings SA5 had claimed by
checking `planner_managed`. That is an avoidance convention, not an ownership boundary — two systems
solving the same problem, which the project rules forbid.

ADR-115 already says the settlement decides **what**. EP9 predates the SA pipeline and was acting as
a second brain.

### Decision

**SA5 is the sole AI writer of `BuildingOperationPolicy`.**

EP9 is a production-planning **service** invoked through settlement intent, not a peer pipeline stage
and not an independent policy owner:

- When SA5 holds a production `SettlementIntent`, it may ask EP9 to derive producers from the
  catalog-built production graph and to recommend enable / operation / priority for capable buildings.
- SA5 applies those recommendations. EP9 does not write policy on its own authority for AI-driven
  settlements.
- EP9 must **not** run as an independent `run_simulation_tick` stage that mutates policy between SA5
  and later stages.
- The `planner_managed` flag as a dual-writer ownership workaround is **removed where it is no longer
  necessary**. It is not replaced with a new "who owns this building" layer between SA5 and EP9.

### Player policy remains authoritative

`ControlSource::PlayerControlled` is a hard skip. SA5 must **not** overwrite explicit player policy.
This invariant survives and is not weakened by SA5 becoming the sole AI writer.

### Demand authority

For AI-driven production, **need pressure → SettlementIntent is the demand authority** (ADR-117 /
ADR-119). EP9 must not independently decide *whether* to produce from `StockGoal` comparison.

`StockGoal` may remain as persisted graph configuration (which items the service knows how to
satisfy, overlapping authored need targets). It must not be a second "should we produce stone?"
brain. Implementation must not leave two independent quantity authorities for the same item or
category.

### Membership

Building membership for producer discovery uses `BuildingRecord.settlement_id` (ADR-133). Affiliation
reconcile is retired.

### Preserved

- EP9 still does not execute production, move items, assign workers, or mutate `BuildingOperationState`.
- The production graph remains derived from `OperationCatalog`, not authored as a hardcoded chain.
- Diagnostics and derived graphs remain unpersisted.

### Food production chains

The production graph is the existing place to see that **intermediate** operations (wheat → flour →
bread) can still satisfy a food intent: demand walks the graph; SA5 enables producers of demanded
outputs, not only buildings whose immediate output is edible. **This milestone** does not require a
processing chain — Prispod farm → edible Prispod is enough. Direct edible farm output is a
validation simplification, not a permanent “farms always emit meals” rule.

## References

- ADR-072, ADR-107–ADR-113
- ADR-115, ADR-117, ADR-119, ADR-120, ADR-133
- ARCHITECTURE.md EP9 section
