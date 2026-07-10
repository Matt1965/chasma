# ADR-065: Authoritative Simulation Tick Orchestrator

## Status

Accepted (REVIEW-B1)

## Context

Through C9, authoritative simulation stages (command resolve, combat engagement,
strikes, projectiles, death, combat AI, movement) were coordinated inside
[`step_all_unit_movement`](../src/world/unit/movement.rs). That function name and
location obscured ownership: movement code owned combat orchestration, tick order was
hard to discover, and tests had to invoke the full god-tick to exercise any stage.

REVIEW-A4 established canonical stage ordering in ADR-057. REVIEW-A1/A4/A6 and ADR-064
fixed pause, step-once, and render-independent fixed timestep at
[`tick_unit_movement`](../src/player/simulation.rs). The orchestration seam was still
missing.

## Decision

### Dedicated orchestrator

Authoritative tick sequencing lives in [`run_simulation_tick`](../src/simulation/tick.rs).
[`step_all_unit_movement`](../src/world/unit/movement.rs) advances locomotion only.

```text
Render / UI / debug (outside orchestrator)
        ↓
SimulationClock + SimulationControlState (pause / step_once gate)
        ↓
run_simulation_tick × N   (one call = one authoritative tick)
        ↓
SimulationTickReport
```

### Canonical stage order (unchanged from ADR-057 REVIEW-A4)

```text
1. resolve_pending_unit_orders
2. step_all_combat_engagement
3. step_all_combat_strikes        (may spawn projectiles)
4. step_all_projectiles           (same-tick spawns skipped)
5. step_unit_death_pipeline
6. step_combat_ai_acquisition
7. step_all_unit_movement
```

Do not reorder stages without a new ADR. Engagement before strikes; death after
projectiles; combat AI after death cleanup; movement last.

### Subsystem ownership

| Stage | Owner module | API |
|-------|--------------|-----|
| Command resolve | `world::unit::orders` | `resolve_pending_unit_orders` |
| Engagement | `world::combat::engagement` | `step_all_combat_engagement` |
| Strikes | `world::combat::strike` | `step_all_combat_strikes` |
| Projectiles | `world::projectile` | `step_all_projectiles` |
| Death | `world::unit::death` | `step_unit_death_pipeline` |
| Combat AI | `world::combat::ai` | `step_combat_ai_acquisition` |
| Movement | `world::unit::movement` | `step_all_unit_movement` |

The orchestrator contains no combat or movement algorithms — only ordered calls and
report aggregation.

### WorldData authority

[`WorldData`] remains the sole authoritative mutable simulation state. The orchestrator
does not clone world data, introduce ECS combat truth, or use global mutable statics.
Borrow conflicts continue to be resolved with sorted ID snapshots and focused APIs
inside subsystems.

### Reporting

[`SimulationTickReport`](../src/simulation/report.rs) composes existing subsystem
reports (`CommandBufferResolveReport`, `CombatEngagementReport`, `CombatStrikeReport`,
`ProjectileReport`, `UnitDeathReport`, `CombatAiReport`, `BatchUnitMovementReport`).
[`tick_unit_movement`](../src/player/simulation.rs) merges per-tick reports into
[`PendingSimulationTrace`] for debug flush.

Non-fatal per-unit failures stay inside subsystem reports; one unit/path/target failure
must not abort the tick for unrelated units.

### Fixed-timestep integration

- One `run_simulation_tick` call advances exactly one authoritative tick.
- Paused simulation does not call the orchestrator (`SimulationControlState::begin_tick`).
- `step_once` runs exactly one orchestrator call then re-pauses.
- Render sync, UI, camera, and environment systems remain outside the orchestrator.

**REVIEW-B5 client frame ordering** (Update schedule, separate from fixed tick):

```text
RuntimeSyncSystems → PlayerControlSystems (sim tick + intents + presentation)
```

[`RuntimeSyncSystems`](../src/player/plugin.rs) mirrors authoritative [`WorldData`] into
render entities before [`PlayerControlSystems`](../src/player/plugin.rs) runs client input
and presentation. Runtime sync must never become authoritative — see ADR-068.

Bevy registers a single thin system (`tick_unit_movement` in `SimulationSystems`) that
calls the pure orchestrator API. Deterministic ordering is preserved by sequential stage
calls inside one function rather than parallel ECS systems.

### Adding future stages

1. Implement behavior in the owning subsystem module with a focused API.
2. Document the stage position relative to ADR-057 / this ADR.
3. Add one call in `run_simulation_tick` — do not embed algorithms in the orchestrator.
4. Extend `SimulationTickReport` only if the subsystem already produces counts/traces.

## Consequences

- Tick order is obvious from `run_simulation_tick` and this ADR.
- Movement tests can call `step_all_unit_movement` without advancing combat.
- Integration tests call `run_simulation_tick` for full-tick scenarios.
- ADR-057, ADR-058, ADR-059, ADR-060, ADR-062 reference this ADR for pipeline entry.

## References

- [ADR-046](ADR-046-simulation-pause-system-architecture.md) — pause / step-once
- [ADR-057](ADR-057-combat-range-and-chase-behavior.md) — canonical stage order
- [ADR-064](ADR-064-fixed-timestep-simulation-clock.md) — fixed timestep clock
