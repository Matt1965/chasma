# ADR-074: Runtime Unit Animation Foundation (A1)

## Status

Accepted (A1 locomotion; A2 weapon-driven combat animation)

## Context

ADR-028 established derived unit render entities from authoritative `WorldData`.
Combat timing (ADR-058), fixed simulation tick (ADR-064/065), and animation design
review A0 defined presentation-only animation driven by simulation state — not the
reverse.

A1 implements the runtime framework for Idle / Walk / Run locomotion clips.

## Decision

### Presentation authority

- Animation is **derived presentation** in `src/units/animation/`.
- `WorldData`, `UnitRecord`, `CombatState`, and `AttackCycle` remain animation-blind.
- Simulation must never read `AnimationPlayer`, clip timing, or animation events.
- No animation event applies damage or mutates simulation.

### Runtime ownership

| Concern | Owner |
|---------|--------|
| Locomotion clip mapping | `AnimationProfile` catalog (`AnimationProfileCatalog`) |
| Unit → profile reference | `UnitDefinition.animation_profile_id` |
| glTF clip names | `Animation Profiles` Excel sheet (dev import) |
| `AnimationGraph` per definition | `UnitAnimationAssets` (shared per `UnitDefinitionId`) |
| Player discovery link | `UnitAnimationPlayerLink` on render root |
| Last applied clip (survives respawn) | `UnitAnimationStateIndex` keyed by `UnitId` |
| Playback | `AnimationTransitions` on player entity |

### AnimationPlayer discovery

After `SceneRoot` spawn:

1. Descend `Children` from `UnitRenderEntity` root (not per-frame global scan).
2. Cache first `AnimationPlayer` in `UnitAnimationPlayerLink`.
3. Duplicate players: deterministic lowest `Entity` id + warn once.
4. Missing player: `PendingAnimationLink` + retry; static model; warn once.
5. Scene recreation: `AnimationPlaybackPending` forces initial play even if persisted clip matches.

### Profile system

- `AnimationProfileId` + `AnimationProfile` catalog (parallel to weapon/unit catalogs).
- Units reference profiles via optional `Animation Profile` column.
- Missing/blank profile → static model (no error).
- Disabled/missing profile at runtime → warn once, static model.

### Clip lookup

- Stable **glTF clip names** from profile data — not numeric indices.
- Fallback chain: Run → Walk → Idle; missing clips warn once.

### Idle / Walk / Run mapping

Pure function `derive_unit_animation_intent`:

| Authoritative state | Intent |
|-------------------|--------|
| `UnitState::Moving` + speed ≥ `reference * 0.75` | Run (with fallback) |
| `UnitState::Moving` otherwise | Walk (with fallback) |
| `UnitState::Idle` | Idle |
| `UnitState::Dead` | None (no animation) |

Run threshold constant: `DOCUMENTED_RUN_SPEED_RATIO = 0.75` (`UnitAnimationSettings.run_speed_ratio`).

Combat state is ignored in A1.

### Playback

- `AnimationTransitions::play` only when intent clip changes or `AnimationPlaybackPending`.
- Looping locomotion clips; cross-fade via `default_blend_ms`.
- Playback speed scales from `move_speed_mps / locomotion_reference_speed_mps` × `locomotion_speed_scale` (hook for future sim speed).
- Simulation pause freezes unit animation (`pause_all`); UI/environment clocks independent.

### Root motion / rotation

- Authoritative `Transform` on render root comes from `sync_unit_render_entities` only.
- Bone animation on descendant `AnimationPlayer` must not translate or rotate the unit root.

### System ordering

```text
RuntimeSync (sync_unit_render_entities)
  → UnitAnimationSystems (graph build, discovery, install, playback)
  → after SimulationSystems (intent reflects current tick)
  → GameplayPresentation (health bars, HUD)
```

Configured in `AppPlugin` and `UnitAnimationPlugin`.

### Failure policy

Warn once; never panic; never block simulation:

- missing profile, clip, glTF, or `AnimationPlayer`
- duplicate players
- scene not yet loaded
- missing weapon attack clip → Idle fallback

## A2: Weapon-driven combat animation

### Weapon animation ownership

| Field | Owner |
|-------|--------|
| Attack clip name | `WeaponDefinition.animation_key` |
| Playback policy | `WeaponDefinition.attack_animation.playback_policy` |
| Normalized strike time | `WeaponDefinition.attack_animation.normalized_strike_time` |
| Blend in / out | `WeaponDefinition.attack_animation.blend_in_ms` / `blend_out_ms` |
| Attack variant (future) | `WeaponDefinition.attack_animation.variant` |

Locomotion remains on `AnimationProfile`. Natural weapons (bite, claws) use the same weapon path.

### Attack intent

`derive_unit_animation_intent` returns `UnitAnimationIntent::Attack { weapon_id, phase, .. }`
when `AttackCycle` phase is Windup, Strike, or Recovery. Cooldown returns to locomotion.

Combat simulation timing (windup / strike / recovery / damage / projectiles) is unchanged.

### Playback synchronization

- Attack clips are one-shot (no loop).
- Speed = `clip_duration / (windup + recovery)` under `ScaleToCycle` policy.
- Simulation strike remains authoritative; animation may `set_seek_time` at Strike when drift exceeds tolerance.
- Transitions: locomotion ↔ attack use weapon blend in/out ms.
- Same attack cycle does not restart clip across Windup → Strike → Recovery.

### A2 failure policy

Missing attack clip or weapon animation → warn once, fall back to Idle locomotion node.

## Non-goals (A1–A4)

- Overlay layer behavior (framework only in A4)
- Animation LOD, debug UI
- Root motion
- Animation-driven gameplay

## A4: Animation layering

See [ADR-075](ADR-075-animation-layering.md). Lower and upper body play independently
when skeleton supports masked graphs; otherwise full-body exclusive fallback preserves
pre-A4 behavior.

## Future (A5+)

- Downed state and corpse persistence in world data (ADR-069)
- Overlay layer activation (hit reactions, buffs, VFX)
- See animation design review A0 phase map

## A3: Death presentation and hit reactions

### Death lifecycle

When a unit is removed from [`WorldData`], runtime sync no longer despawns immediately.
`begin_death_presentations` transitions the render entity into [`DeathPresentation`]:
plays a one-shot death clip (or freezes pose), then `tick_death_presentations` despawns
after the presentation timer expires. Simulation never waits.

### Hit reactions

`detect_unit_hit_reactions` compares cached HP to authoritative vitals each frame.
Short one-shot hit clips play during locomotion only — never during attack phases.
Death intent always overrides hit reactions.

### Profile clips

| Clip | Source |
|------|--------|
| Death | `AnimationProfile.death_clip` |
| Hit | `AnimationProfile.hit_reaction_clip` |

### A3 failure policy

Missing death clip → freeze final pose for `death_freeze_hold_seconds`.
Missing hit clip → idle fallback node; no simulation impact.

## A5 locomotion polish (ADR-076)

D5 extends presentation without simulation changes:

- Walk/run hysteresis (`run_enter_ratio` / `run_exit_ratio`)
- Turn-in-place clips from heading vs model forward (-Z)
- Context blend durations (accel/decel/stop/turn)
- Live playback speed updates and heading-based foot-slide slowdown
- `LocomotionPresentationState` persisted per `UnitId`

## A6 scaling (ADR-077)

Final animation phase: shared graphs, presentation LOD, asset validation, aggregate dev metrics.
Simulation remains animation-blind.

## A1 stabilization (ADR-077)

Post-audit corrections: pause-gated death/hit timers, typed death/hit graph mapping, late corpse graph install, off-screen death policy, attack blend-in/out, stale link recovery, weapon animation-key import validation.

**Excel note:** locomotion profile columns are importable today; death/hit/turn/layering profile fields remain code/starter until the workbook schema extends.

## References

- ADR-028 (unit runtime layer)
- ADR-058 (combat timing — authoritative)
- ADR-059 (death removal — simulation authority)
- ADR-064/065 (simulation clock)
- ADR-069 (combat design — animation fits sim, not vice versa)
- ADR-076 (advanced locomotion polish)
- ADR-077 (scaling, LOD, validation)
- `DESIGN.md` animation section
- `docs/animation-authoring.md`
- `src/units/animation/`
