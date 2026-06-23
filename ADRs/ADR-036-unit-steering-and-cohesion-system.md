# ADR-036: Unit Steering and Cohesion System (U11)

# Status

Accepted (U11 — local avoidance steering layer)

# Context

U10 distributes group move targets into formation slots. U7 pathfinding routes each
unit independently. Without a local adjustment layer, units still stack, jitter, and
visually collapse when paths converge or formation slots are tight.

U11 adds deterministic local steering between path intent and the authoritative
movement step. It is not pathfinding, not formation planning, and not physics.

# Decision

## Pipeline position

```text
U10 formation target → U7 A* path → U11 steering → WorldData relocate
```

Steering adjusts the **movement direction** for one tick. It does not modify
[`NavigationPath`], waypoints, or [`UnitState::Moving::target`].

## Module (`src/world/movement/steering/`)

| File | Role |
|------|------|
| [`separation.rs`](../src/world/movement/steering/separation.rs) | Primary overlap repulsion |
| [`cohesion.rs`](../src/world/movement/steering/cohesion.rs) | Weak pull toward formation target centroid |
| [`alignment.rs`](../src/world/movement/steering/alignment.rs) | Minimal neighbor velocity bias |
| [`avoidance.rs`](../src/world/movement/steering/avoidance.rs) | [`SteeringContext`], composition, integration helper |

## Forces (not physics)

Vector adjustments are clamped and blended conservatively:

- **Separation** — overlap-weighted repulsion using combined collision radii + padding
- **Cohesion** — weak pull toward average of nearby formation targets (U10 slots)
- **Alignment** — very weak neighbor velocity bias (optional stabilizer)

[`SteeringSettings::DEFAULT`] caps influence and max steering angle from path direction.

## Spatial query

[`WorldData::query_units_in_radius`] scans chunk-local unit stores in a bounded
neighborhood (not O(N²) global scan). Results are sorted by [`UnitId`].

## Determinism

- Batch movement iterates sorted unit ids (existing U5 rule)
- Neighbor lists sorted by [`UnitId`]
- No randomness; identical state → identical steering output

## Non-responsibilities

Steering does **not** consider terrain, slope, doodads, enemies, or global group AI.
U6/U7 continue to own blocking and global routing.

# Consequences

**Benefits:**

- SC2-like group movement feel without clumping at arrival
- Pathfinding and formation logic remain unchanged
- Testable, deterministic local layer

**Costs:**

- Neighbor query scans several chunk stores per moving unit (bounded radius)
- Very dense crowds may still compress until separation radius engages

# References

- ADR-035 (U10 formation)
- ADR-032 (U7 pathfinding)
- ADR-030 (U5 movement)
- ADR-031 (U6 obstacles)

[`NavigationPath`]: ../src/world/navigation/path.rs
[`UnitState::Moving::target`]: ../src/world/unit/state.rs
[`SteeringContext`]: ../src/world/movement/steering/avoidance.rs
[`SteeringSettings::DEFAULT`]: ../src/world/movement/steering/mod.rs
[`WorldData::query_units_in_radius`]: ../src/world/data.rs
[`UnitId`]: ../src/world/unit/id.rs
