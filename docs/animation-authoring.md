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
| Gameplay forward axis | **-Z** (Bevy / glTF default) |
| Up axis | **+Y** |

Units must face **-Z** at bind pose when no correction is needed. During movement,
authoritative [`UnitPlacement`](../../src/world/unit/placement.rs) yaw is updated so
local **-Z** points along the unit's actual accepted horizontal travel direction
(UNIT-FACING-1 / ADR-076).

### Visual vs gameplay facing

Source GLBs may be authored with a different forward axis. Do **not** change movement
facing code per asset. Instead set the Units sheet columns:

| Column | Purpose |
|--------|---------|
| Rotation Correction X Deg | Pitch — rotation **about X** |
| Rotation Correction Y Deg | Yaw — rotation **about Y** (most common; use for sideways/backward GLB forward) |
| Rotation Correction Z Deg | Roll — rotation **about Z** |

Workbook columns name **physical axes**. The importer maps them to semantic orientation as
`yaw = Y`, `pitch = X`, `roll = Z` (see `rotation_correction_from_workbook_xyz_degrees`).
For a GLB that faces the wrong horizontal direction, set **Y only** — do not use X/Z as a
workaround for yaw.

> **Integration note (UNIT-FACING-2A):** An earlier importer bug passed `(X, Y, Z)` directly
> into `QuantizedOrientation::from_degrees(yaw, pitch, roll)`, so a Y-column yaw was applied
> as pitch and tipped models on their side. Verify visually after setting corrections.

Runtime presentation composes:

```text
visual Transform.rotation = placement.rotation × sizing_rotation_correction
```

Gameplay, animation heading, and locomotion polish continue to use **`placement.rotation`
only** — not the visual correction.

Verify movement visually after import. The runtime validation info line documents
Chasma's **-Z convention**; it does **not** prove the raw GLB faces -Z.

### Visual turn speed (`Turn Speed Deg/s`)

Maximum **presentation** body yaw speed in degrees per second. This limits how quickly
the rendered model rotates toward authoritative travel facing — it does **not** limit
actual movement, steering, pathing, or `UnitPlacement.rotation` updates.

| Unit (initial tuning) | Turn Speed Deg/s | ~180° reversal |
|-----------------------|------------------|----------------|
| Robot | 540 | ~0.33 s |
| Fox | 720 | ~0.25 s |
| Cavecrawler | 360 | ~0.50 s |

Blank/missing column defaults to **540**. Values must be finite and > 0.

Runtime composes smoothed visual yaw first, then model rotation correction:

```text
Transform.rotation = visual_world_facing × sizing_rotation_correction
```

Incorrect export forward causes wrong visuals until rotation correction is authored.

## Required clips (locomotion)

These map to **`Animation Profiles`** worksheet columns (case-sensitive glTF clip names):

| Worksheet column | Runtime field | Notes |
|------------------|---------------|-------|
| Idle Animation | `idle_clip` | **Required** when profile enabled |
| Walk Animation | `walk_clip` | Fallback chain: Run→Walk→Idle |
| Run Animation | `run_clip` | Optional; falls back to Walk |
| Locomotion Reference Speed | `locomotion_reference_speed_mps` | m/s the walk/run cycles were authored for (default 4.0 when column absent) |

## Optional clips (presentation / turns)

Also authored on **`Animation Profiles`** (all optional; blank cell → unset):

| Worksheet column | Runtime field | Notes |
|------------------|---------------|-------|
| Death Animation | `death_clip` | Full-body override |
| Hit Reaction Animation | `hit_reaction_clip` | Full-body override |
| Upper Body Split Bone | `upper_body_split_bone` | First upper-body bone for masked layering; leave blank for full-body creatures |
| Turn Left Animation | `turn_left_clip` | Turn-in-place / heading adjust |
| Turn Right Animation | `turn_right_clip` | Mirror of left |
| Turn Left Duration | `turn_left_duration_seconds` | Seconds; blank → runtime default |
| Turn Right Duration | `turn_right_duration_seconds` | Seconds; blank → runtime default |

Turn clips should be **in-place** (no root translation). Duration can be authored
in profile or defaults apply at runtime.

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

Owned by **weapon** definitions (`Animation Key` on the **`Weapons`** worksheet), not animation profiles.
The key must exactly match a named clip in the unit's GLB.

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
