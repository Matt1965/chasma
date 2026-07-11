# ADR-072: Settlement Automation and Production

## Status

Accepted (design direction — not implemented)

## Context

ARCHITECTURE.md describes settlements as future first-class simulation objects. Automation
philosophy — professions, tasks, building requests, and worker priorities — was not
documented. Implementation is deferred until occupancy, persistence, and unit assignment
foundations exist (ROADMAP Phases 6–8+).

Full design narrative: [DESIGN.md](../DESIGN.md#settlement-automation).

## Decision

### Professions over micromanagement

Workers are assigned **persistent professions** with ordered priorities, not per-click chores.

Example profile:

| Slot | Role |
|------|------|
| Primary | Farmer |
| Secondary | Hauler |
| Emergency | Defender |

### Jobs vs tasks

| | Jobs | Tasks |
|---|------|-------|
| **Lifetime** | Persistent profession | Temporary work unit |
| **Examples** | Farmer, Builder, Hunter, Smith | Harvest Wheat, Bake Bread, Repair Wall |
| **Source** | Worker assignment | Building / world generators |

Buildings **emit tasks**; workers **claim tasks** matching profession and priority.

### Production requests

Buildings request inputs and declare desired outputs — **Factorio-style logistics with
individual workers**, not abstract city-wide resource pools.

Example: Bakery requests flour; produces bread when tasks complete and inputs arrive.

### Worker priority fall-through

Ordered priority list (e.g. Farming → Construction → Hauling → Medicine). When no work
exists at a priority level, workers fall through to the next. **Direct player orders
temporarily override** automation.

### Automation vs chores (DESIGN.md principle)

Automation reduces repetitive busywork (auto-haul, priority fall-through) while preserving
strategic choices (profession assignment, building placement, emergency roles).

## Architectural seams (existing)

- `WorldData` authoritative instances (ADR-027)
- Occupancy layer for buildings (ROADMAP Phase 6)
- Persistence overrides (ROADMAP Phase 7)
- Settlement as first-class object (ARCHITECTURE.md) — not terrain, not doodad

## Non-goals (current phase)

- Building placement UI, recipe graphs, or economy balancing
- Abstract "city mana" production without worker assignment
- Per-item job queue micromanagement as the default UX

## Consequences

- Task system should be data-driven (building defs request task types)
- Worker AI reads profession + priority from instance state on `WorldData`
- Player override uses existing intent/command pipeline (ADR-038, ADR-041)

## References

- [DESIGN.md](../DESIGN.md)
- ARCHITECTURE.md (Settlements, Resources)
- ROADMAP.md (Phases 6–8, Future Systems)
- ADR-027, ADR-038, ADR-041
