# ADR-030: Unit Orders and Straight-Line Movement

# Status

Accepted (U5 — movement prototype)

# Context

U2 placed authoritative [`UnitRecord`] instances on [`WorldData`]. U3 synced
disposable render entities from placement. U4 grounded Y to resident heightfields.

A minimal movement prototype is needed to prove the simulation pipeline:

```text
UnitOrder::MoveTo → step → terrain grounding → WorldData placement → runtime sync
```

Pathfinding, collision, and AI are explicitly deferred.

# Decision

## WorldData simulation only

Movement is authoritative world-side logic in `src/world/unit/movement.rs` and
`orders.rs`. No ECS systems, no pathfinding, no doodad or unit-unit collision.

## Orders and state

[`UnitOrder`]:

- `Idle`
- `MoveTo { target: WorldPosition }`

[`issue_unit_order`] sets [`UnitState`] without moving immediately.

[`UnitState`]:

- `Idle`
- `Moving { target: WorldPosition }`

## Movement step

[`step_unit_movement`]:

- Uses [`UnitDefinition::move_speed_mps`]
- Moves straight-line on XZ toward target
- Snaps when within step distance or arrival epsilon
- Grounds Y via [`ground_world_position`] (heightfield only)
- Validates slope via [`estimate_slope_degrees`] against `max_slope_degrees`
- Updates placement through [`relocate_unit`] (cross-chunk safe)
- Sets `Idle` on arrival; preserves rotation, source, metadata on steps

[`step_all_unit_movement`] iterates [`WorldData::sorted_unit_ids`] deterministically.

## Failure without mutation

When terrain, slope sample, or slope limit blocks a step, placement is unchanged
(errors returned before [`relocate_unit`]).

## Runtime unchanged

[`sync_unit_render_entities`] (ADR-028) reads placement each frame. No runtime
changes required for U5.

## Explicit non-goals

- A* / navmesh pathfinding
- Doodad or unit collision
- Selection UI, commands UI, animation, combat, AI
- Automatic per-frame movement (caller drives `step_*`)
- Grounding on [`create_unit`] (ADR-029)

# Consequences

**Benefits:**

- End-to-end proof of authoritative movement → render mirror
- Shared terrain query layer (ADR-029) reused for height and slope
- Cross-chunk relocation via existing WorldData indexes

**Deferred:**

- Obstacle maps, pathfinding, collision, richer orders

# References

- ADR-027 (unit data ownership)
- ADR-028 (runtime sync)
- ADR-029 (terrain grounding)
- ADR-005 (heightfield queries)

[`UnitRecord`]: ../src/world/unit/record.rs
[`WorldData`]: ../src/world/data.rs
[`UnitOrder`]: ../src/world/unit/orders.rs
[`UnitState`]: ../src/world/unit/state.rs
[`issue_unit_order`]: ../src/world/unit/orders.rs
[`step_unit_movement`]: ../src/world/unit/movement.rs
[`relocate_unit`]: ../src/world/data.rs
[`ground_world_position`]: ../src/world/terrain/query.rs
[`sync_unit_render_entities`]: ../src/units/sync.rs
