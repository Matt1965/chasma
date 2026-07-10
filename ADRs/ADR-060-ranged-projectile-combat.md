# ADR-060: Ranged Projectile Combat

## Status

Accepted

## Context

ADR-058 (C5) implemented weapon windup, strike, recovery, and cooldown for melee and
ranged-instant weapons. Projectile hit mode was intentionally deferred: strikes emitted
`UnsupportedProjectileMode` and applied no damage.

C7 requires authoritative projectile simulation with disposable ECS visuals. Damage must
apply at impact, not at strike time. Projectile payload must remain fixed after launch.

REVIEW-A3 identified that impact previously revalidated only target existence/alive state.
That is insufficient for combat correctness when ownership or weapon target filters change
during flight.

## Decision

### Authoritative simulation

Projectile state lives on [`WorldData`] as [`ProjectileRecord`] entries keyed by
[`ProjectileId`]. Simulation owns position, target snapshot, speed, status, and stored
damage payload. ECS entities in `src/projectiles/` are render mirrors only.

### Launch-time payload freezing

When a projectile weapon completes windup and passes strike validation, the simulation
creates an `InFlight` record with a fixed payload:

- `source_unit_id`, `target_unit_id`, `weapon_id`
- `damage`, `damage_type`
- `speed_mps`, spawn position, initial `target_position_snapshot`
- [`ProjectileLaunchSnapshot`]: frozen attacker ownership (`source_owner_id`,
  `source_team_id`, `source_affiliation`), weapon `target_filters`, and
  `dev_allow_all_targets` policy flag

Strike-time range and full [`validate_attack_target`] rules apply at spawn only. Weapon or
unit stat changes after launch do not alter an in-flight projectile's damage payload.

### Projectile record lifecycle

1. **Spawn (strike phase):** Create an `InFlight` record at the attacker's authoritative
   [`WorldPosition`]. Do not apply damage.
2. **Move (simulation tick):** Each tick, step in-flight projectiles toward the target's
   current position and refresh `target_position_snapshot`. If the target is missing or
   dead before impact, mark `Expired` and remove without applying damage.
3. **Hit (valid impact):** Revalidate target legality (see below). Apply stored damage via
   `WorldData::damage_unit`, record kill attribution, emit `Hit` / `DamageApplied`, mark
   `Hit`, remove the record.
4. **Invalid impact:** Revalidation fails. Apply no damage, mark `Invalidated`, emit
   `ImpactRejected { reason }`, remove the record.

### Impact-time legality revalidation (REVIEW-A3)

At impact, [`validate_projectile_impact_target`] reuses the same ownership and weapon-filter
rules as launch, evaluated against the frozen [`ProjectileLaunchSnapshot`] and the target's
current [`UnitRecord`]:

- Target still exists (`TargetMissing`)
- Target is alive (`TargetDead`)
- Source/target are not the same unit (`TargetNowFriendly`)
- Launch-time ownership hostility still permits the attack (`TargetNowFriendly`)
- Launch-time weapon target filters still permit the target (`TargetFilterRejected`)
- Snapshot ownership context is usable (`OwnershipUnavailable` when affiliation is `Unknown`
  and dev override is off)

Impact revalidation does **not** require the source unit to exist or be alive. It does
**not** recheck weapon range.

Typed rejection reasons live in [`ProjectileImpactRejection`]. Simulation must not duplicate
ownership/filter logic outside `src/world/combat/targeting.rs`.

### Source and target death behavior

- **Target dies or is removed before impact:** Projectile expires during travel (`Expired`);
  no damage.
- **Source dies or is removed after launch:** Projectile continues. Impact uses the frozen
  launch snapshot for ownership/filter checks and may still apply damage to a legally valid
  target.

### Ownership changes during flight

If a target's team or affiliation changes mid-flight such that the launch-time attacker
relationship would no longer permit the attack, impact is rejected with `TargetNowFriendly`
(or `TargetFilterRejected` when filters no longer match). No friendly-fire damage is
applied solely because ownership changed after launch.

### Range at impact

Weapon range is validated at strike time only. In-flight projectiles may follow a moving
target beyond the original weapon range. Homing-to-current-position behavior is intentional
for C7; only target legality is revalidated at impact.

### Weapon data

[`WeaponDefinition`] gains `projectile_speed_mps`. Excel import accepts optional
`Projectile Speed`; validation requires `> 0` when `hit_mode == Projectile`. Melee and
ranged-instant weapons ignore the column.

`projectile_key` maps to `assets/projectiles/{projectile_key}.glb` for visuals. Missing
keys log once and skip the visual; simulation continues.

### Tick order

Projectile movement runs after strikes in the canonical simulation order
([ADR-057](ADR-057-combat-range-and-chase-behavior.md#canonical-simulation-tick-order-review-a4)).
Projectiles spawned during strike resolution are **not** stepped in the same tick;
movement begins on the next tick. Impact deaths enter the same
`step_unit_death_pipeline` as direct strikes.

### Runtime rendering

`ProjectilesRuntimePlugin` registers after `UnitsRuntimePlugin`. Sync spawns/updates/
despawns render entities from authoritative records. Render transforms apply terrain
vertical scale to Y only; world records are never modified by the runtime layer.

### Trace / debug

Simulation emits `ProjectileSpawned`, `ProjectileHit`, `ProjectileExpired`,
`ProjectileImpactRejected`, and `ProjectileDamageApplied` through [`ProjectileReport`] →
[`CommandTraceBuffer`].

## Consequences

- Projectile weapons reuse C5 attack-cycle timing; only damage timing differs.
- No line-of-sight, terrain collision, splash, homing arcs, or ballistics in C7.
- Future systems can extend `ProjectileRecord` or add behavior modules without making ECS
  authoritative.

## Future hooks

Reserved for later phases:

- Splash / area damage at impact
- Homing and lead targeting
- Ballistic arcs and gravity
- Line-of-sight and terrain/doodad collision
- Projectile interception
- VFX and sound keyed off trace events

## Non-goals (C7)

No combat AI, animation system, armor/resistance, or pathfinding changes.
