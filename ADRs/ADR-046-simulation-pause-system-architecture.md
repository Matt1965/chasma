# ADR-046: Simulation Pause System Architecture

# Status

Accepted (core simulation execution control)

# Context

The runtime separates client presentation (rendering, UI, debug overlays) from
authoritative simulation ([`WorldData`](../src/world/data.rs) mutations via
movement, pathfinding, steering, and formation). Dev mode (ADR-043) and future
combat debugging (U13+) require **deterministic step-through** without altering
simulation rules or coupling pause to UI.

Prior behavior: [`tick_unit_movement`](../src/player/simulation.rs) advanced every
render frame with no global gate.

# Decision

## Real time vs simulation time

```text
Real time (render / UI / debug / input collection)
        ↓
SimulationClock accumulator (ADR-064)
        ↓
SimulationControlState gate (pause / step_once)
        ↓
Simulation tick(s) — run_simulation_tick × N
        ↓
Movement · combat · projectiles · death · AI (unchanged rules)
```

Pause affects **simulation tick progression only**. Rendering, UI, debug overlays,
selection, and intent collection continue on real time.

## Ownership

| Component | Layer | Role |
|-----------|-------|------|
| [`SimulationControlState`](../src/simulation/control.rs) | Core (`src/simulation/`) | Authoritative pause/tick counter |
| [`SimulationControlRequests`](../src/simulation/control.rs) | Core | External request queue (dev UI, tooling) |
| [`SimulationPlugin`](../src/simulation/plugin.rs) | App composition | Registers resources + control input |
| [`SimulationSystems`](../src/simulation/plugin.rs) | Schedule set | Gated simulation mutation systems |
| Dev panel buttons | Dev (`src/dev/`) | Issue requests; do **not** own state |

## SimulationControlState

| Field | Purpose |
|-------|---------|
| `paused` | When true, skip simulation ticks |
| `step_once` | Run exactly one tick, then re-pause |
| `current_tick` | Monotonic completed simulation tick counter |

Fixed-timestep scheduling lives on [`SimulationClock`](../src/simulation/control.rs)
(ADR-064), not on this resource.

## Input bindings

| Key | Action |
|-----|--------|
| Space | Toggle pause / resume |
| Shift+Space | Step one simulation tick |

F12 remains dev mode toggle (ADR-043). Space is **not** used for dev catalog search.

## Gating point

[`tick_unit_movement`](../src/player/simulation.rs) plans ticks via
[`SimulationClock::plan_frame`](../src/simulation/control.rs), then for each planned tick
calls [`SimulationControlState::begin_tick`](../src/simulation/control.rs) before
[`run_simulation_tick`](../src/simulation/tick.rs) (ADR-065) and
[`complete_tick`](../src/simulation/control.rs) after.

One orchestrator call advances all canonical simulation stages (ADR-057); locomotion is
[`step_all_unit_movement`](../src/world/unit/movement.rs) inside that pipeline.

**Not gated** (by design):

- Intent collection ([`collect_unit_input_intents`](../src/client/pipeline.rs))
- Intent dispatch (order queue / selection)
- Render sync, terrain streaming, debug overlays
- Dev spawning (WorldData edits while paused — authoring, not simulation advance)

## Step-through model

1. User sets `step_once` (Shift+Space or dev button).
2. Next frame: `begin_tick` returns true even if `paused`.
3. One full `run_simulation_tick` runs (ADR-065).
4. `complete_tick` increments `current_tick`, clears `step_once`, sets `paused = true`.

No partial ticks; no duplicate execution within a frame.

## Dev mode integration

When dev mode is enabled, the panel displays pause state and tick count and exposes
Pause/Resume and Step buttons via [`SimulationControlRequests`](../src/simulation/control.rs).

Dev mode reads [`SimulationControlState`](../src/simulation/control.rs); it never
mutates it directly.

## Future compatibility

- SC2-style replay debugging: `current_tick` is the hook for recorded snapshots.
- Multiplayer: control state can move to server authority; gate pattern unchanged.
- Time dilation: deferred until product-facing controls exist (see ADR-064).

# Consequences

- Safe dev spawning and scene capture while simulation is frozen.
- Deterministic debugging foundation for U13+ combat systems.
- Simulation rules remain untouched; only execution scheduling changes.

# Non-goals

- Gameplay time dilation mechanics
- Save/load integration
- UI-owned pause state
- Gating rendering or ECS presentation loops
