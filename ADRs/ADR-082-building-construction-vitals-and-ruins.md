# ADR-082: Building Construction, Vitals, and Ruins

## Status

Accepted (B5, July 2026)

## Context

B4 introduced player build mode and `BuildingLifecycleState::Planned` with reserved occupancy.
B5 adds authoritative construction progression, vitals, destruction, ruins, and presentation sync
without worker tasks, resource delivery, or production functionality.

## Decision

### Lifecycle model

`BuildingRecord` owns:

- `lifecycle_state`: `Planned → Foundation → InProgress → Complete`, plus `Destroyed → Ruins`
- `construction.progress_0_1` in `[0.0, 1.0]`
- `vitals: BuildingVitals { current_hp, max_hp }`

`WorldData` is the sole authority. ECS render entities mirror lifecycle for presentation only.

### Temporary construction progression (until B8)

`BuildingConstructionSettings::auto_timed_progress` (default **true** in dev/test) advances
incomplete buildings each fixed simulation tick:

```
delta_progress = SIMULATION_TICK_SECONDS / build_time_seconds
```

Pause and `step_once` follow existing `SimulationControlState` policy — construction runs only
inside `run_simulation_tick`, which is not invoked while paused (except single step).

Worker labor, material delivery, and repair tasks are **not** simulated in B5.

### Simulation integration

`step_all_building_construction` runs after combat AI acquisition and before unit movement
(ADR-065 stage order extension).

### Operational gate

```rust
is_building_operational(record) :=
    lifecycle_state == Complete && current_hp > 0
```

Future production and task systems must use this helper.

### Vitals policy

| State | max_hp | current_hp |
|-------|--------|------------|
| Planned / Foundation / InProgress | definition.max_hp | max(1, max_hp / 10) |
| Complete | definition.max_hp | full |
| Destroyed / Ruins | definition.max_hp | 0 |

Damage clamps at 0; heal clamps at max. HP zero triggers `destroy_building` → immediate `Ruins`.

### Occupancy by lifecycle (ADR-080 extension)

| State | Occupancy | Movement |
|-------|-----------|----------|
| Planned | Reserved | walkable |
| Foundation / InProgress / Complete | Blocked | blocked |
| Destroyed | Blocked (transient) | blocked |
| Ruins | Reserved | walkable |

Stage changes call `update_building_occupancy` atomically before committing record state.

### Presentation

Placeholder cuboids use `lifecycle_building_color` per stage. `BuildingRenderEntity.lifecycle_state`
tracks the last rendered stage; sync updates materials when authority changes.

### Dev / persistence

- Dev inspector shortcuts (D/H/X/R/C/P) call authoritative APIs only.
- Dev scenes v2 serialize `building_records` + `next_building_id`; occupancy rebuilt on load.

### Future seams

- B8 worker tasks replace `auto_timed_progress` labor source.
- Combat targeting may call `damage_building` when building attackability is added.
- Repair gameplay beyond `heal_building` API is deferred.

## Consequences

- Construction is deterministic and testable on the fixed tick.
- No duplicate occupancy or lifecycle systems.
- Build mode (ADR-081) unchanged; ghosts remain client-local until commit creates `Planned` records.

## Related

- ADR-079 (building instances)
- ADR-080 (occupancy)
- ADR-081 (build mode)
- ADR-065 (simulation tick)
