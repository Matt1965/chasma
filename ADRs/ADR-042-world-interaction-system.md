# ADR-042: World Interaction System (U6)

# Status

Accepted (U6 — world interaction query foundation)

# Context

U-UI5 established client-side contextual commands. Movement, pathfinding, and steering
remain authoritative in their existing modules. Future economy (harvesting), combat
targeting, building placement, and AI queries need a **world-data-first** classification
layer that answers “what is at this point?” without coupling to ECS or UI.

# Decision

## Pipeline

```text
WorldData + catalogs → query_world_interaction → InteractionResult
                     → resolve_interaction_to_order → UnitOrder plan
                     → issue_unit_order (existing, unchanged)
```

Interaction **never** mutates [`WorldData`] during query. It **never** overrides A* or
steering — it only supplies destination targets for [`UnitOrder::MoveTo`].

## Module: `src/world/interaction/`

| File | Responsibility |
|------|----------------|
| [`types.rs`](../src/world/interaction/types.rs) | `InteractionType`, `InteractionResult`, metadata |
| [`query.rs`](../src/world/interaction/query.rs) | `query_world_interaction` — unified facade |
| [`resolver.rs`](../src/world/interaction/resolver.rs) | Interaction → `InteractionOrderPlan` → `UnitOrder` |
| [`mod.rs`](../src/world/interaction/mod.rs) | `InteractionDebugSnapshot` hook |

## Interaction types (U6)

| Type | U6 behavior |
|------|-------------|
| `MoveTarget` | Walkable grounded terrain |
| `ResourceNode` | Doodad kind `ResourceNode` (read-only stub) |
| `InteractableObject` | Non-blocking doodad |
| `BlockedArea` | Blocking doodad or unwalkable slope |
| `TerrainPoint` | Reserved for terrain-only samples |
| `None` | Invalid / missing terrain |

## Unified query (without merging systems)

[`query_world_interaction`](../src/world/interaction/query.rs) composes:

- [`ground_world_position`](../src/world/terrain/query.rs) — heightfield (U4/U7)
- [`blocking_doodad_at_position`](../src/world/obstacle/query.rs) — doodad obstacles (U6/ADR-031)
- Nearest doodad scan for interactable classification

Terrain, obstacle, and doodad modules remain separate; interaction only **calls** them.

## Resolver rules

| Interaction | Order plan |
|-------------|------------|
| `MoveTarget` | `MoveTo` |
| `ResourceNode` | `MoveTo` (placeholder until U13+) |
| `InteractableObject` | `MoveTo` (placeholder) |
| `BlockedArea` | `NoOp` |

## Boundaries

- No changes to movement, pathfinding, steering, UI, or intent architecture
- No combat, harvesting execution, AI, or gameplay effects
- Client command layer may consume resolver later; U6 owns world semantics only

## Debug (U-UI3 hook)

[`draw_interaction_debug_overlay`](../src/debug/overlay/interaction_overlay.rs) re-queries
the last dispatched click from [`IntentDispatchHistory`] (read-only) and draws
classification gizmos. [`InteractionDebugSnapshot`] stores the last query + resolved order.

## Future extensions

- Harvesting: `ResourceNode` → harvest order variant
- Combat: unit/doodad target refs → attack order
- Building placement: `TerrainPoint` + placement validator
- AI: same query API for decision systems
- Interaction animations: metadata-driven presentation layer

# Consequences

**Benefits:**

- Single SC2-style click-context abstraction on [`WorldData`]
- Economy/combat can extend types without rewriting movement
- Deterministic, testable classification

**Costs:**

- Nearest-doodad scan is chunk-local (acceptable for cursor radius)
- Placeholder `MoveTo` for resource/interact until gameplay systems exist

# References

- ADR-029 (terrain queries)
- ADR-031 (doodad obstacles)
- ADR-038/041 (client intent + commands)
- ADR-039 (debug overlays)

[`WorldData`]: ../src/world/data.rs
