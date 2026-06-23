# ADR-035: Unit Formation System (U10)

# Status

Accepted (U10 — group move target distribution)

# Context

U9 issues a `MoveTo` order to every selected unit at the same click point, causing
clumping. U7 pathfinding routes each unit independently but cannot spread goals.
U11 will add unit-unit avoidance during movement; U10 only decomposes a group
command into spatially separated targets.

Formation planning is **world simulation intent**, not player input state (U9) and
not tactical AI.

# Decision

## Module (`src/world/formation/`)

| File | Role |
|------|------|
| [`layout.rs`](../src/world/formation/layout.rs) | [`FormationKind`] — `Line`, `Circle` |
| [`offsets.rs`](../src/world/formation/offsets.rs) | Spacing, jitter, [`FormationOffset`] |
| [`distribution.rs`](../src/world/formation/distribution.rs) | Slot generation on line/ring |
| [`planner.rs`](../src/world/formation/planner.rs) | [`FormationPlanner::plan_move`] |

## Flow (U9 → U10 → U7)

1. U9 right-click produces `SelectedUnits` + click [`WorldPosition`].
2. [`FormationPlanner::plan_move`] sorts ids by [`UnitId`], computes offsets.
3. [`issue_move_orders_to_selection`] calls [`issue_unit_order`] once per assignment.
4. U7 [`find_path`] runs per unit with no shared path state.

## Default layout

**Circle** ring around the click center (SC2-style blob spread). **Line** is
available for future explicit selection; U10 uses circle by default.

## Centering

Formation center is always the **clicked target**, not the unit centroid or camera
vector.

## Spacing

```text
slot_spacing = max(max(collision_radius * 2), FORMATION_MIN_SPACING_METERS)
```

Group spacing uses the **maximum** collision radius among selected units.

Ring radius grows with unit count so adjacent arc length ≥ spacing.

## Determinism

- Sort [`UnitId`] before slot assignment.
- Optional radial jitter is hashed from `UnitId` + target XZ (stable, small).

## Non-responsibilities

Formation planning does **not** consider obstacles, terrain slope, or doodads.
Pathfinding (U6/U7) handles per-unit reachability.

# Consequences

**Benefits:**

- SC2-like group moves without clumping at one cell
- Deterministic, testable spatial decomposition
- Pathfinding and obstacles unchanged

**Costs:**

- Line layout axis is fixed (global X) until camera-relative layout is needed
- Very tight groups may still overlap slightly until U11 swarming

# References

- ADR-033 (U8 orders)
- ADR-034 (U9 selection)
- ADR-032 (U7 pathfinding)
- ADR-031 (U6 obstacles)

[`FormationKind`]: ../src/world/formation/layout.rs
[`FormationPlanner::plan_move`]: ../src/world/formation/planner.rs
[`issue_move_orders_to_selection`]: ../src/units/input/commands.rs
[`issue_unit_order`]: ../src/world/unit/orders.rs
[`find_path`]: ../src/world/navigation/query.rs
[`UnitId`]: ../src/world/unit/id.rs
[`WorldPosition`]: ../src/world/coordinates.rs
