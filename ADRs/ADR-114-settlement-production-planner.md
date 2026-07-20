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

When demand exists, the planner enables operational buildings whose `supported_operations`
produce demanded items. When goals are satisfied, planner-managed buildings are disabled.
`planner_managed` and `ControlSource::AIControlled` mark planner ownership; player-controlled
buildings are skipped.

### Settlement building membership

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

## References

- ADR-072, ADR-107–ADR-113
- ARCHITECTURE.md EP9 section
