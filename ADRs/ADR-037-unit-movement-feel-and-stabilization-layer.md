# ADR-037: Unit Movement Feel & Stabilization Layer (U12)

# Status

Accepted (U12 — responsiveness and first-step stabilization)

# Context

After U7–U11, units could still appear to move in the wrong direction for the
first tick before correcting toward their path. Grid A* paths often start with a
waypoint near or behind the unit; U11 steering could dominate when path direction
was zero, pulling toward formation targets (U10) instead of the path.

## Root cause (wrong first direction)

Not primarily path-assignment timing — paths were already computed synchronously.
The bug was **zero path direction + active steering**: when the unit sat on/near the
first grid waypoint, path direction was zero while U11 cohesion still pulled toward
the formation target. Smoothing could further reduce net progress toward later
waypoints if left unbounded near arrival.

# Decision

## Pipeline

```text
Right-click → command buffer (Idle) → resolve path → Moving → stabilize direction
→ optional smoothing → U11 steering (gated) → relocate
```

## Command buffer (`src/world/movement/feel/command_buffer.rs`)

`MoveTo` orders enqueue on [`WorldData`] and resolve at the start of
[`step_all_unit_movement`] before any unit steps. Units remain `Idle` until path
generation succeeds. One-tick deferral eliminates same-frame order/move races.

## Stabilization (`stabilization.rs`)

- **Rule 1:** Movement direction comes from the active path waypoint only — never
  the raw click target when a path exists.
- **Rule 2:** Skip consumed waypoints within epsilon; no fallback vector when path
  is empty or direction is zero.
- **Rule 3:** First movement step uses stabilized waypoint direction; smoothing
  bypasses the first tick.

## Steering gate (U11 extension)

[`apply_steering`] early-outs when path direction is zero or steering is disallowed.
Prevents cohesion/separation from replacing path intent on the first frame.

## Smoothing (`smoothing.rs`)

Optional exponential direction blend with max turn clamp — feel only; does not alter
waypoints, path, or targets. Smoothing is **bypassed** on the final path segment and
when within two step lengths of the active waypoint so net progress toward waypoints
is never delayed.

## Presentation (`src/player/move_feedback.rs`, `indicator.rs`)

- Move destination ground marker (fade in/out)
- Path debug polylines when `debug_unit_interaction` is enabled
- Selection ring fade-in (child of unit render entity — no chunk jitter)

# Consequences

**Benefits:**

- Eliminates first-frame wrong-direction artifacts
- SC2-like command feedback without changing simulation rules
- Deterministic buffer resolution order (sorted [`UnitId`])

**Costs:**

- Move orders take one simulation tick to begin after issue
- Additional state on [`WorldData`] (buffer + smoothing cache)

# References

- ADR-036 (U11 steering)
- ADR-035 (U10 formation)
- ADR-032 (U7 pathfinding)
- ADR-033 (U8 player control)

[`WorldData`]: ../src/world/data.rs
[`step_all_unit_movement`]: ../src/world/unit/movement.rs
[`apply_steering`]: ../src/world/movement/steering/avoidance.rs
[`UnitId`]: ../src/world/unit/id.rs
