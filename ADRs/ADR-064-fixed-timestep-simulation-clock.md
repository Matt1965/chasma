# ADR-064: Fixed-Timestep Simulation Clock

## Status

Accepted

## Context

Prior to this ADR, [`tick_unit_movement`](../src/player/simulation.rs) ran once per render
frame and passed a fixed [`SIMULATION_TICK_SECONDS`](../src/simulation/control.rs) into
[`run_simulation_tick`](../src/simulation/tick.rs) (ADR-065).
[`SimulationControlState`]
(ADR-046) gated pause and step-once, but did not accumulate real elapsed time. Higher FPS
therefore advanced the authoritative world faster than lower FPS.

Combat timers, projectile motion, AI scan intervals, and unit movement all depend on
simulation tick count, not render frame count. Frame-rate-dependent advancement is a
correctness bug and blocks deterministic replay and future multiplayer lockstep.

## Decision

### Render time vs simulation time

```text
Render frame (variable real delta from Time::delta_secs)
        ↓
SimulationClock accumulator (real seconds)
        ↓
FrameTickPlan (0..N fixed ticks, capped per frame)
        ↓
SimulationControlState gate (pause / step_once)
        ↓
run_simulation_tick × N (each with SIMULATION_TICK_SECONDS)
```

Presentation (render sync, UI, input, debug overlays, camera, environment) stays on
`Update` real time. Only [`SimulationSystems`](../src/simulation/plugin.rs) authoritative
mutation runs on fixed ticks.

### Schedule approach

Use an explicit [`SimulationClock`](../src/simulation/control.rs) resource processed from
`Update`, not Bevy `FixedUpdate`. This preserves ADR-046 pause/step semantics without
fighting the engine fixed schedule:

- **Paused:** no accumulator growth, zero ticks.
- **Step once:** exactly one tick on the next frame; accumulator unchanged.
- **Running:** accumulate real delta, emit `floor(accumulator / tick_seconds)` ticks.

### Fixed rate

Simulation rate remains **30 Hz** via `SIMULATION_TICK_SECONDS = 1.0 / 30.0`. Gameplay
timing values are unchanged; only scheduling is fixed.

### Catch-up policy

[`MAX_SIMULATION_TICKS_PER_FRAME`](../src/simulation/control.rs) (8) caps ticks per render
frame. Remaining accumulated time carries forward on subsequent frames. Time is not
discarded when capped; slow frames catch up over multiple frames.

### Speed multiplier

`simulation_speed_multiplier` was removed from [`SimulationControlState`]. It was stored
but unused and not exposed in dev UI. Time scaling is deferred until a product-facing
control exists (ADR-046 future compatibility).

### Systems under fixed ticks

All authoritative work orchestrated by `run_simulation_tick` (ADR-065) runs per fixed tick:

- Combat strikes and attack timers
- Projectile simulation
- Death pipeline
- Combat AI acquisition scans
- Combat engagement
- Pending order resolution
- Unit movement, steering, formation

No additional systems were moved; the single tick entry point already owned simulation
mutation.

### Trace merging

When multiple ticks run in one render frame, per-tick reports merge into
[`PendingSimulationTrace`](../src/debug/pending.rs) before debug flush.

## Pause / step behavior

Unchanged from ADR-046:

| State | Clock behavior |
|-------|----------------|
| Running | Accumulate delta, run planned ticks |
| Paused | No accumulation, zero ticks |
| Step once | One tick, then re-pause |

Rendering, UI, and debug continue while paused.

## Future compatibility

- **Replay:** `current_tick` plus fixed `SIMULATION_TICK_SECONDS` gives a stable
  simulation timeline independent of capture FPS.
- **Multiplayer:** Server can own `SimulationClock` accumulation; clients keep the same
  gate pattern. This ADR does not implement lockstep or networking.
- **Time dilation:** Reintroduce as an explicit multiplier on accumulator input when
  product requirements and UI exist.

## Consequences

- 30, 60, and 120 FPS produce identical simulation state after equal real elapsed time.
- Lag spikes may spread catch-up across multiple frames (bounded by per-frame cap).
- `simulation_speed_multiplier` field removed; any saved inspector snapshots referencing
  it must be regenerated.

## Non-goals

- Network lockstep
- Replay recording/playback
- Render interpolation redesign
- Gameplay balance changes
- Bevy `FixedUpdate` migration
