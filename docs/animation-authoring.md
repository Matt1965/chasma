# Animation Authoring Guide

Presentation-only animation for Chasma units. Simulation (`WorldData`) never reads
animation state.

## Pipeline overview

1. Author clips in DCC tool (Blender, etc.)
2. Export glTF with **named animations** matching the unit's `AnimationProfile`
3. Assign `Animation Profile` on the unit definition (Excel / dev catalog)
4. Runtime builds a shared `AnimationGraph` per `UnitDefinitionId`

## Model orientation

| Convention | Value |
|------------|--------|
| Forward axis | **-Z** (Bevy / glTF default) |
| Up axis | **+Y** |

Units must face **-Z** at bind pose. Locomotion turn presentation compares model
forward from `placement.rotation` to movement heading — incorrect export forward
causes wrong turn direction or foot-slide slowdown.

## Required clips (locomotion)

| Profile key | glTF clip name | Notes |
|-------------|----------------|-------|
| Idle | `idle_clip` column | Required when profile enabled |
| Walk | `walk_clip` | Fallback chain: Run→Walk→Idle |
| Run | `run_clip` | Optional; falls back to Walk |

## Optional clips (A5 polish)

| Profile key | glTF clip name | Notes |
|-------------|----------------|-------|
| TurnLeft | `turn_left_clip` | Turn-in-place / heading adjust |
| TurnRight | `turn_right_clip` | Mirror of left |
| Death | `death_clip` | Full-body override |
| Hit | `hit_reaction_clip` | Full-body override |

Turn clips should be **in-place** (no root translation). Duration can be authored
in profile (`turn_left_duration_seconds`, etc.) or defaults apply.

## Layering (masked playback)

Set `upper_body_split_bone` to the first upper-body bone (e.g. `Spine`).

- Locomotion clips play on **lower body** mask
- Attack clips play on **upper body** mask
- Missing split bone → full-body exclusive mode

## Playback speed

Walk/run playback speed scales from:

```
move_speed_mps / locomotion_reference_speed_mps * locomotion_speed_scale
```

Set `locomotion_reference_speed_mps` to the speed the walk/run cycles were authored for.

## Walk vs run threshold

Default enter run at **75%** of reference speed; exit run at **65%** (hysteresis).
Tunable via `UnitAnimationSettings` — does not change unit `move_speed_mps`.

## Attack clips

Owned by **weapon** definitions (`animation_key`), not animation profiles.

## Validation checklist

- [ ] Model faces -Z at bind pose
- [ ] Clip names match profile exactly (case-sensitive)
- [ ] Walk/run cycles authored at `locomotion_reference_speed_mps`
- [ ] Turn clips have no root motion
- [ ] Split bone exists in skeleton for masked units
- [ ] Death/hit clips are one-shot, not looping

## Dev Mode

**Debug tab** (with unit selected via Inspector): current clip, layers, playback
speed, LOD tier, distance, graph share identity, validation counts, aggregate
Full/Reduced/Frozen counts, missing clips.

See ADR-074, ADR-075, ADR-076, ADR-077.

## Runtime scale (A6)

- Identical profile + glTF + weapon → shared `AnimationGraph`
- Distant units use presentation LOD (`AnimationLodSettings`) — simulation unchanged
- Missing optional clips log once as warnings; required Idle missing is an error

## Runtime stabilization (A1 audit fixes)

- Death/hit graph nodes are distinct when both clips exist in profile + glTF
- Simulation **pause** freezes death/hit timers and `AnimationPlayer` playback
- **Step once** advances presentation timers by one simulation tick (30 Hz)
- Corpses may **late-install** graphs via `UnitRenderMetadata` after world removal
- **Off-screen deaths** do not spawn new corpse entities (no presentation queue)
- Weapon `Animation Key` must be non-empty and glTF-safe at Excel import
- Stale `AnimationPlayer` links self-heal via pending rediscovery
- Attack **blend-in/out** from weapon metadata applies to upper-body presentation
