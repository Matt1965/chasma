# ADR-059: Unit Death and Removal Pipeline (C6)

# Status

Accepted (C6 — death and cleanup only)

# Context

ADR-058 (C5) applied weapon damage to [`UnitVitals`] without death state or
removal. Units could reach 0 HP and remain in [`WorldData`]. C6 closes the
lifecycle gap while deferring corpses, loot, XP, animations, and projectiles.

# Decision

## Scope (C6)

| In scope | Out of scope (deferred) |
|----------|-------------------------|
| `UnitState::Dead` | Corpse entities |
| Deferred [`UnitRemovalQueue`] | Death animations |
| [`WorldData::remove_unit_by_id`] | Loot / XP |
| Target + selection cleanup | Respawn |
| Kill attribution traces | AI morale reactions |
| Runtime despawn via existing sync | Projectile changes |

## Death model

When [`UnitVitals::current_hp`] reaches 0:

1. Mark [`UnitState::Dead`] (same tick, after damage)
2. Enqueue [`UnitRemovalEntry`] with `RemovalReason::Killed`
3. Clear combat/movement side effects on the dead unit
4. Clear other units' combat targets pointing at the dead unit
5. Remove records from [`WorldData`] in deterministic [`UnitId`] order

Damage application (C5) does **not** remove units inline. Removal is deferred to
the death pipeline step that runs immediately after combat strikes each tick.

## Removal queue

[`UnitRemovalQueue`] stores pending [`UnitRemovalEntry`] rows:

- `unit_id`
- `reason` (`Killed`, `DevDeleted`, `Cleanup`, `Unknown`)
- `killer: Option<UnitId>`
- `tick`

Duplicate queue entries for the same id are rejected.

Public API: [`queue_unit_removal`] for future dev-delete reuse.

## Kill attribution

Combat strikes record [`KillAttribution`] (killer + HP before) on [`WorldData`]
when damage is applied. Death detection consumes this when emitting
`UnitDied` / queue rows.

## Simulation tick order

Within [`step_all_unit_movement`]:

1. `step_all_combat_strikes` (C5 — unchanged timing rules)
2. `step_unit_death_pipeline` (detect → queue → target cleanup → remove)
3. `step_all_combat_engagement` (C4)
4. `resolve_pending_unit_orders`
5. `step_unit_movement` (skips dead / 0 HP units)

Pause / `step_once` gating remains at [`SimulationControlState::begin_tick`].

## Target and selection cleanup

When a unit dies or is removed:

- Attack / chase / attack-move acquired targets pointing at it are cleared
- Dead unit command buffer, smoothing, and attack cycle are cleared
- [`SelectedUnits::prune_dead`] runs client-side after simulation tick
- [`PlayerHudState`] primary selection resyncs

## Runtime presentation

No new render entities. [`sync_unit_render_entities`] already despawn ECS
entities when [`UnitRecord`] disappears from [`WorldData`] — derived
presentation only.

## Trace events

[`UnitDeathReport`] emits:

- `UnitDied`
- `UnitRemovalQueued`
- `UnitRemoved`
- `TargetClearedDueToDeath`

Flushed through [`PendingSimulationTrace`] → [`CommandTraceBuffer::record_unit_death`].

# Consequences

- 0 HP units exist briefly as `Dead` before removal in the same tick
- Combat cannot target dead units ([`is_unit_alive`] includes `Dead` state)
- Orders to dead/removed units fail with `UnitNotFound`
- Deterministic removal ordering supports future replay/multiplayer seams

# References

- ADR-055 (C2 vitals)
- ADR-058 (C5 strike timing)
- `src/world/unit/death.rs`
- `src/units/sync.rs`
