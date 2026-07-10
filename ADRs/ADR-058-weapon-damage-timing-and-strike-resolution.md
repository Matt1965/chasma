# ADR-058: Weapon Damage Timing and Strike Resolution (C5)

# Status

Accepted (C5 — damage timing only)

# Context

ADR-057 (C4) added edge-to-edge range checks, chase, in-range hold, and
attack-move acquisition without applying damage. C5 closes the strike gap while
deferring death/removal (C6), projectiles (C7), armor, and combat AI.

# Decision

## Scope (C5)

| In scope | Out of scope (deferred) |
|----------|-------------------------|
| Attack windup / strike / recovery / cooldown | Death / removal (C6) |
| `attacks_per_second` timing model | Projectile spawn / travel (C7) |
| Damage to `UnitVitals.current_hp` | Animations |
| Strike-time validity re-check | Armor / resistance / crits |
| Melee + `RangedInstant` hit modes | Combat AI |
| Deterministic simulation tick integration | Corpses / loot / XP |

## Timing model

From [`WeaponDefinition`]:

```text
attack_period_seconds = 1.0 / attacks_per_second
cooldown_seconds      = max(0, attack_period - windup - recovery)
```

When `windup + recovery > attack_period`, the cycle may exceed the nominal
period (no panic; optional debug warn). Phases:

1. **Windup** — no damage
2. **Strike** — instant transition; damage applied once here
3. **Recovery** — no damage
4. **Cooldown** — remainder until next windup (may be zero)

State lives on [`UnitRecord.attack_cycle`] (`AttackCycle`: phase, remaining
seconds, struck-this-cycle flag).

## Damage rule

```text
final_damage = WeaponDefinition.damage   // no scaling, armor, or randomness
```

Applied via [`WorldData::damage_unit`] (saturating subtract, HP clamped at 0).
No death or entity removal side effects until C6.

## Strike validity

At the windup→strike transition, re-validate:

- attacker and target exist and are alive
- ownership / weapon target filter (C3)
- edge-to-edge weapon range (C4)
- `attack_cycle.target` matches `CombatState` engagement target (REVIEW-A2)
- attacker is in an attack-capable combat state

On failure: no damage, clear `attack_cycle`, resume chase per C4 (`Chasing` or
`AttackMoving` with target).

### Attack-cycle lifetime (REVIEW-A2)

Strike timing advances only when engagement state is attack-capable and the
combat-state target matches the cycle target. `step_all_combat_strikes` uses
`CombatState` as the authoritative strike target — not `attack_cycle.target`
alone.

Defensive strike gate (`validate_attack_cycle_for_strike`):

- rejects `Peaceful` / non-attack-capable states
- rejects ownership / filter / alive failures
- on mismatch between cycle target and combat target: clear cycle, emit
  `AttackStrikeSkippedStateMismatch`, apply no damage

Engagement invalid-target handling and order cancellation clear cycles **before**
the next strike tick; the strike gate is a safety net, not a substitute.

## Hit modes

| Mode | C5 behavior |
|------|-------------|
| `Melee` | Full timing + damage |
| `RangedInstant` | Full timing + damage at range |
| `Projectile` | **Deferred** — emit `UnsupportedProjectileMode`, no damage |

Projectile weapons do not apply damage until C7.

## Tick integration

Strike timers advance in `step_all_combat_strikes` **after**
`step_all_combat_engagement` within the canonical tick order defined in
[ADR-057](ADR-057-combat-range-and-chase-behavior.md#canonical-simulation-tick-order-review-a4).

Engagement clears attack cycles when targets leave range or become invalid before
strike progression runs. Player simulation passes [`SIMULATION_TICK_SECONDS`]
(30 Hz), not render `Time::delta_secs()`. Pause / `step_once` gate at
[`SimulationControlState::begin_tick`].

## Same-tick death (REVIEW-A4)

Strikes resolve in deterministic [`UnitId`] order. Once a target reaches 0 HP,
later strikes and projectile impacts in the same tick do not apply further damage.
Death is queued once per unit. Kill attribution uses the last recorded lethal hit
before the death pipeline runs (overwritten on each `record_kill_attribution` call).

## Trace events

`CombatStrikeReport` emits:

- `AttackWindupStarted`
- `AttackStrikeApplied` (damage, HP before/after)
- `AttackStrikeMissedInvalidTarget`
- `AttackRecoveryStarted`
- `AttackCooldownStarted`
- `UnsupportedProjectileMode`
- `AttackCycleResetRetarget` (old / new target)
- `AttackCycleClearedInvalidTarget`
- `AttackCycleClearedOrderCancelled`
- `AttackStrikeSkippedStateMismatch` (cycle vs combat target)

Flushed through `PendingSimulationTrace` → `CommandTraceBuffer::record_combat_strike`.

# Consequences

- Units in `Attacking` or in-range `AttackMoving` accumulate weapon cycles and
  apply catalog damage deterministically.
- Leaving range or invalid targets clears cycles; engagement re-establishes chase.
- Retargeting or order cancellation clears partial windup / recovery — no
  stale-target strikes.
- HP may reach zero without removal until C6.
- Projectile weapons are explicitly no-op for damage in C5.

# References

- ADR-055 (C2 vitals)
- ADR-056 (C3 targeting / orders)
- ADR-057 (C4 range / chase)
- `src/world/combat/strike.rs`
- `src/world/combat/cycle_lifecycle.rs`
- `src/world/unit/attack_cycle.rs`
