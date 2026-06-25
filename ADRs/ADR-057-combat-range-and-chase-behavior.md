# ADR-057: Combat Range and Chase Behavior (C4)

# Status

Accepted (C4 — range and positioning only)

# Context

ADR-056 (C3) added attack orders, combat state, and target validation without
range checks, chase, or attack-move acquisition. C4 closes the positioning gap
while deferring damage, timing, death, projectiles, and combat AI to C5+.

# Decision

## Scope (C4)

| In scope | Out of scope (deferred) |
|----------|-------------------------|
| Edge-to-edge weapon range | Damage application |
| `CombatState::Chasing` | Windup / cooldown strike resolution |
| Standoff destination + pathfind | Death / removal |
| In-range hold (stop movement) | Projectiles |
| Target revalidation each tick | Animations |
| Attack-move hostile scan | Combat AI / surround tactics |
| Combat engagement trace | A* / steering / formation changes |

## Range rule (edge-to-edge)

Effective reach uses authoritative [`WorldPosition`] center distance minus both
collision radii from [`UnitDefinition`]:

```text
edge_distance = center_distance - attacker_radius - target_radius
in_range      <= weapon.range_meters
leave_range   >  weapon.range_meters + RANGE_HYSTERESIS_METERS (0.5 m)
```

[`WeaponDefinition.range_meters`] is the edge-to-edge truth. Helpers live in
`src/world/combat/range.rs` (`is_in_weapon_range`, `measure_weapon_range`).

## Combat state

| State | Meaning |
|-------|---------|
| `Attacking { target }` | Valid target, in weapon range, locomotion idle |
| `Chasing { target }` | Valid target, out of range, pathing to standoff |
| `AttackMoving { destination, target }` | Move toward destination; optional acquired target |

Direct `Attack` orders set `Attacking` or `Chasing` immediately based on range.
`AttackMoving` retains `destination` when a scanned target is lost.

## Standoff destination

Computed along target→attacker XZ direction at:

```text
center_distance = weapon.range + attacker_radius + target_radius
```

Grounded via [`ground_world_position`]. Failures report `TerrainUnavailable` and
do not teleport or mutate placement.

## Movement reuse

Chase uses shared [`start_unit_move_to`] (same pathfinding as deferred MoveTo).
Combat runs **before** command-buffer resolve and movement each tick:

1. `step_all_combat_engagement`
2. `resolve_pending_unit_orders`
3. `step_unit_movement`

No changes to A*, steering, or formation systems.

## Attack-move scan

While `AttackMoving` without a valid target:

- Scan sorted units within `ATTACK_MOVE_SCAN_RADIUS_METERS` (16 m)
- Filter with C3 ownership + weapon validation
- Pick closest valid hostile; tie-break lowest [`UnitId`]
- Transition to `AttackMoving { destination, target: Some(id) }` and chase

When acquired target becomes invalid, resume `AttackMoving { destination, target: None }`.

## Reports / trace

Typed [`CombatEngagementStatus`] values:

- `TargetInvalid`, `MissingWeapon`, `OutOfRangeChasing`, `InRangeReady`
- `TerrainUnavailable`, `PathUnavailable`, `AttackMoveAcquired`, `AttackMoveMoving`

Command trace records range metadata (center/edge distance, weapon range, status)
via [`CommandTraceBuffer::record_combat_engagement`].

## Why damage is deferred (C5)

C4 proves positioning, range math, and movement integration are stable before
adding strike timing and HP mutation. Separating concerns keeps C4 tests
deterministic and avoids coupling pathing bugs to damage resolution.

# Consequences

- `step_all_unit_movement` signature adds `WeaponCatalog` and
  `AttackTargetingPolicy`
- `CombatState` gains `Chasing` variant
- Simulation tick flushes combat traces alongside command resolve traces
- No damage occurs in C4; HP remains unchanged during chase/engage tests
