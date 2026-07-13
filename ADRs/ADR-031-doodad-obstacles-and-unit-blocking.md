# ADR-031: Doodad Obstacles and Unit Blocking

# Status

Accepted (U6 — movement blocking foundation; REVIEW-B6 fail-closed queries).
**Updated B3:** blocking queries delegate to generalized occupancy / passability
([ADR-080](ADR-080-generalized-occupancy-and-baked-footprints.md)).

# Context

U5 added straight-line unit movement on authoritative [`WorldData`]. Units can
walk through doodad records because no obstacle model existed.

Doodads are authoritative world data ([`DoodadRecord`] on [`WorldData`], types
in [`DoodadCatalog`], ADR-015/016). Render entities are disposable (ADR-023) and
must not define gameplay obstacles.

Pathfinding and avoidance are deferred; U6 answers only: **can a unit footprint
occupy this XZ position?**

# Decision

## Obstacle truth: WorldData + DoodadCatalog

Blocking data lives on [`DoodadDefinition`]:

- `blocks_movement: bool`
- `block_radius_meters: f32`

**Not** [`placement_radius_meters`] (generation spacing). Block radius defaults
to placement radius when Excel `Block Radius` is blank.

Kind defaults when columns absent:

| Kind | `blocks_movement` |
|------|-------------------|
| Tree, Rock, Ruin, ResourceNode | `true` |
| Bush | `false` |

Excel optional columns: `Blocks Movement`, `Block Radius` (Y/N parsing).

## World obstacle module

`src/world/obstacle/` owns the legacy API surface; **B3** delegates spatial blocking
to `src/world/occupancy/`:

- [`query_obstacle_at_position`] — structured result with diagnostics (wraps passability)
- [`is_position_blocked_by_doodads`] — convenience bool; **fail-closed on any error**
- Doodad blocking uses `FootprintShape::Circle` from `block_radius_meters`
- Building blocking uses building/doodad footprint definitions via the same passability path
- Checks owning chunk + eight neighbors (geometric overlap, not render/ECS)
- XZ circle overlap: `unit_radius + block_radius_meters` (inclusive `<=` boundary)
- Deterministic chunk and record iteration order
- No ECS / glTF / terrain mesh access

### Fail-closed missing definitions (REVIEW-B6)

When a [`DoodadRecord`] references a missing [`DoodadDefinition`]:

| Kind default | Behavior |
|--------------|----------|
| Blocking kind (tree, rock, ruin, resource node) | Conservative kind-based block radius; overlap test proceeds; [`ObstacleQueryError::MissingDoodadDefinition`] emitted |
| Non-blocking kind (bush) | Passable; no error |

[`is_position_blocked_by_doodads`] treats **any** query error as blocked — navigation
and movement must not silently walk through corrupt catalog data.

## Unit movement integration (U5 extension)

Before [`relocate_unit`] on a movement step:

1. Ground destination Y (ADR-029)
2. Validate slope (ADR-030)
3. Query doodad obstacles → [`UnitMovementError::BlockedByDoodad`] without mutation

No sliding, avoidance, or pushing. Straight-line movement simply stops.

## Pathfinding (U7 / B3)

Navigation ([ADR-032](ADR-032-chunk-grid-navigation.md)) and per-step movement consume
[`query_passability_at`] (terrain + slope + static occupancy). The obstacle module
remains a thin compatibility wrapper for doodad-specific diagnostics.

## Design direction (collision)

See [ADR-069](ADR-069-combat-design-philosophy.md), [DESIGN.md](../DESIGN.md#combat).

- Physical unit collision target: radius **smaller than Warcraft III**
- Natural front lines and chokepoints from collision + weapon reach — **no combat slot system**
- Unit-unit blocking (beyond doodad obstacles) is deferred; straight-line movement stops at
  doodads today

# Consequences

**Benefits:**

- Clear separation: placement radius vs block radius
- World-level obstacle ownership reusable by pathfinding
- Authoritative data only; render layer unchanged

**Deferred:**

- Routing around obstacles
- Unit-unit collision
- Destructible / harvestable obstacles

# References

- ADR-015, ADR-016 (doodad data)
- ADR-027–030 (units)
- ADR-023 (runtime not authoritative)
- ADR-080 (generalized occupancy, B3)

[`query_passability_at`]: ../src/world/occupancy/passability.rs

[`DoodadRecord`]: ../src/world/doodad/record.rs
[`DoodadCatalog`]: ../src/world/doodad/catalog/registry.rs
[`DoodadDefinition`]: ../src/world/doodad/catalog/definition.rs
[`query_obstacle_at_position`]: ../src/world/obstacle/query.rs
[`is_position_blocked_by_doodads`]: ../src/world/obstacle/query.rs
