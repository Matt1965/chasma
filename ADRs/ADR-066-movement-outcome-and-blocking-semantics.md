# ADR-066: Movement Outcome and Blocking Semantics

## Status

Accepted (REVIEW-B2)

## Context

Prior to this ADR, [`handle_blocked_step`] in [`movement.rs`](../src/world/unit/movement.rs)
swallowed slope, doodad, and terrain blocks: it forced [`UnitState::Idle`], returned
`Ok` with `moved: false`, and never surfaced [`UnitMovementError`] block variants.
[`BatchUnitMovementReport`] counters for blocked outcomes were wired but unreachable.

Separately, `simulation_speed_multiplier` was stored but unused. ADR-064 removed it;
REVIEW-B2 confirms no dev/player UI depends on speed control.

## Decision

### Movement step outcomes (ADR-066)

[`step_unit_movement`] returns [`UnitMovementStepOutcome`]:

| Outcome | Meaning |
|---------|---------|
| `Idle` | Unit not moving this tick (not in `Moving`, or dead/queued) |
| `Moved` | Position advanced |
| `Arrived` | Terminal reach of path / partial arrival policy |
| `Blocked(reason)` | Expected world condition; unit remains valid |
| `Failed(error)` | Missing definition, missing unit, broken invariant |

[`BlockedMovementReason`] distinguishes terrain, slope, doodad, and path categories.
[`classify_slope_walkability`] separates `SlopeUnavailable` from `SlopeTooSteep`.

### Blocked vs failed

**Blocked:** world geometry / residency — unit stays alive, order/path retained when
retry may succeed.

**Failed:** catalog or state invariant — counted in `failed_*` batch fields.

### Blocked-state retention

For temporary blocks (terrain, slope, doodad):

- Position, chunk, rotation unchanged
- [`UnitState::Moving`] retained with same target/path/waypoint index
- Combat state untouched

**Terminal transitions to `Idle` only when:**

- Path waypoints exhausted
- Partial arrival within `PARTIAL_ARRIVAL_DISTANCE_METERS` of final target while blocked
- Normal arrival at final waypoint

Waypoint skip within `WAYPOINT_SKIP_DISTANCE_METERS` of a blocked waypoint may advance
`waypoint_index` without position change (existing ADR-030 behavior).

No repathing in this phase.

### Batch reporting

[`BatchUnitMovementReport`] counts one terminal category per unit per tick:
`moved`, `arrived`, `idle`, `blocked_*`, `failed_*`. Traces emit on block **reason
change** only (deduplicated per unit per batch).

### Observability

- [`MovementBlockObservability`](../src/debug/movement_observability.rs) — last block
  per unit (not [`WorldData`] truth)
- [`CommandTraceBuffer::record_unit_movement`] — `UnitMovementBlocked` entries
- Inspector prefers observability trace, falls back to static diagnosis

### Simulation speed

**Option B — removed.** [`SimulationControlState`] has pause, step-once, and tick count
only. Fixed tick rate remains [`SIMULATION_TICK_SECONDS`] (30 Hz). Future speed control
requires a new ADR and validated UI.

## Consequences

- `handle_blocked_step` removed; replaced by `apply_blocked_movement`
- [`UnitMovementError`] shrinks to `UnitNotFound` / `DefinitionNotFound`
- [`SimulationTickReport::movement_blocked_total`] aggregates block counters
- ADR-037 movement ADRs reference this outcome model for blocking semantics

## References

- [ADR-030](ADR-030-unit-orders-and-straight-line-movement.md)
- [ADR-037](ADR-037-unit-movement-feel-and-stabilization-layer.md)
- [ADR-064](ADR-064-fixed-timestep-simulation-clock.md) — speed multiplier removal
- [ADR-065](ADR-065-authoritative-simulation-tick-orchestrator.md) — movement stage

[`handle_blocked_step`]: ../src/world/unit/movement.rs
[`step_unit_movement`]: ../src/world/unit/movement.rs
[`UnitMovementStepOutcome`]: ../src/world/unit/movement.rs
[`BlockedMovementReason`]: ../src/world/unit/movement.rs
[`classify_slope_walkability`]: ../src/world/terrain/query.rs
[`BatchUnitMovementReport`]: ../src/world/unit/movement.rs
[`MovementBlockObservability`]: ../src/debug/movement_observability.rs
[`CommandTraceBuffer::record_unit_movement`]: ../src/debug/trace.rs
[`SimulationControlState`]: ../src/simulation/control.rs
[`SIMULATION_TICK_SECONDS`]: ../src/simulation/control.rs
