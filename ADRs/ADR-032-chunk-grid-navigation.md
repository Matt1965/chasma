# ADR-032: Chunk Grid Navigation (A* Foundation)

# Status

Accepted (U7 — chunk-aware grid pathfinding)

# Context

U5 added straight-line movement; U6 added doodad obstacle queries. Units still
needed deterministic routes around blocking doodads without navmesh, ECS
pathfinding, or render-layer coupling.

Pathfinding must remain world-simulation owned (not under `units/`) so future
systems (AI, caravans, combat) share one navigation service.

# Decision

## World navigation module

`src/world/navigation/` owns:

| File | Responsibility |
|------|----------------|
| `grid.rs` | Cell spacing, coord mapping, walkability from terrain + obstacles |
| `astar.rs` | Deterministic 8-neighbor A* (no corner cutting) |
| `path.rs` | [`NavigationPath`] waypoint container |
| `query.rs` | [`find_path`] public API |

## Grid

- Configurable [`NavigationConfig::cell_spacing_meters`] (default **4 m**)
- Walkability from resident heightfields ([`ground_world_position`]) and
  [`is_position_blocked_by_doodads`] (U6) with agent collision radius
- No navmesh, no render mesh sampling

## A*

- 8 directions with fixed neighbor order (N, NE, E, SE, S, SW, W, NW)
- Octile heuristic; deterministic open-set tie-breaking (f, h, z, x)
- Diagonal moves require both adjacent cardinals walkable
- Search capped at 16 384 expanded nodes

## Path API

```rust
find_path(world, doodad_catalog, config, agent_radius_meters, start, goal)
    -> Result<NavigationPath, NavigationError>
```

Errors: `StartBlocked`, `GoalBlocked`, `NoPath`, `TerrainUnavailable`.

Waypoints are grounded [`WorldPosition`] samples at cell centers.

## Unit integration

[`issue_unit_order`] on `MoveTo`:

1. Resolves unit definition for `collision_radius_meters`
2. Calls [`find_path`] from current placement to target
3. Stores [`UnitState::Moving { target, path, waypoint_index }]`

[`step_unit_movement`] walks `path.waypoints[waypoint_index]`; no continuous
repathing. Per-step slope and obstacle checks remain (ADR-030/031).

## Explicit non-goals

- Navmesh / Recast / GPU pathfinding
- Dynamic repathing, flocking, unit-unit avoidance
- ECS navigation components
- Combat, AI, selection UI

# Consequences

**Benefits:**

- Shared world-owned navigation for units and future consumers
- Reuses U6 obstacle truth without duplicating spatial queries
- Deterministic paths suitable for simulation replay

**Tradeoffs:**

- Grid resolution limits path fidelity (configurable spacing)
- Paths are computed once per order; obstacles moving mid-route are not replanned
- [`UnitState`] no longer `Copy` (stores [`NavigationPath`])

# References

- ADR-030 (orders and movement)
- ADR-031 (doodad obstacles)
- [`find_path`]: ../src/world/navigation/query.rs
- [`NavigationPath`]: ../src/world/navigation/path.rs
- [`issue_unit_order`]: ../src/world/unit/orders.rs
