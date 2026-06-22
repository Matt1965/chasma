# ADR-031: Doodad Obstacles and Unit Blocking

# Status

Accepted (U6 — movement blocking foundation)

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

`src/world/obstacle/` owns spatial queries against doodad chunk stores:

- [`is_position_blocked_by_doodads`]
- Checks owning chunk + eight neighbors
- XZ circle overlap: `unit_radius + block_radius_meters`
- Deterministic chunk and record iteration order
- No ECS / glTF / terrain mesh access

## Unit movement integration (U5 extension)

Before [`relocate_unit`] on a movement step:

1. Ground destination Y (ADR-029)
2. Validate slope (ADR-030)
3. Query doodad obstacles → [`UnitMovementError::BlockedByDoodad`] without mutation

No sliding, avoidance, or pushing. Straight-line movement simply stops.

## Pathfinding (deferred)

Future navigation will consume the same obstacle query layer. U6 does not build
navgrids or A*.

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

[`DoodadRecord`]: ../src/world/doodad/record.rs
[`DoodadCatalog`]: ../src/world/doodad/catalog/registry.rs
[`DoodadDefinition`]: ../src/world/doodad/catalog/definition.rs
[`is_position_blocked_by_doodads`]: ../src/world/obstacle/query.rs
