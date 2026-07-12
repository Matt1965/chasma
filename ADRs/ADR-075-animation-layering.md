# ADR-075: Animation Layering

## Status

Accepted (A4 / D4)

## Context

ADR-074 A1–A3 established locomotion, weapon-driven combat animation, and death/hit
presentation using a single active clip per unit. Tactical combat requires units to
move and attack simultaneously (walk + sword swing, run + bow draw). Simulation
timing and weapon ownership remain unchanged.

ADR-070 is reserved for progression and attributes; layering is documented here.

## Decision

Introduce a **presentation-only layered animation architecture** with three slots:

| Layer | Owner | Active in A4 |
|-------|-------|--------------|
| Lower body | Locomotion (Idle/Walk/Run) | Yes |
| Upper body | Combat/tools (Attack) | Yes |
| Overlay | Hit VFX, buffs, construction sparks, etc. | Framework only |

### Priority (highest first)

1. Death (full-body override)
2. Hit reaction (full-body override until overlay layer is active)
3. Upper body (attack)
4. Lower body (locomotion)

### Blend model

- Animation graphs use an **additive blend root** with **mask groups**:
  - Group 0: lower body bones
  - Group 1: upper body bones
  - Group 2: reserved for overlay
- Locomotion clips mask out upper body; attack clips mask out lower body.
- Death/hit clips are full-body (no mask).
- `configure_unit_animation_layering` assigns bones from scene skeleton using
  `AnimationProfile.upper_body_split_bone`.

### Layering modes

| Mode | When |
|------|------|
| `Masked` | Skeleton split bone found; dual playback |
| `FullBodyExclusive` | Missing split bone or incompatible skeleton; pre-A4 behavior |

### Weapon ownership

Attack clips, playback policy, and strike timing remain on `WeaponDefinition`
(ADR-074 A2). Unit definitions reference animation profiles only for locomotion
and skeleton conventions.

### Skeleton conventions

- `AnimationProfile.upper_body_split_bone`: first upper-body bone name or path
  suffix (e.g. `Spine`).
- Bones at or above the split → upper-body mask group.
- All other animated bones → lower-body mask group.
- Incompatible skeleton → log once, fall back to `FullBodyExclusive`.

### Failure policy

| Failure | Behavior |
|---------|----------|
| Missing upper-body attack clip | Lower body continues (masked mode) |
| Missing lower-body clip | Idle fallback chain (existing profile rules) |
| Skeleton mismatch | Full-body exclusive playback |
| Any layer failure | Simulation unaffected |

## Non-goals (A4)

- Overlay layer behavior (hit reactions remain full-body override from A3)
- Abilities, spellcasting, gathering, building animations
- Root motion
- Gameplay or simulation changes

## A5 follow-up (ADR-076)

Locomotion polish (turns, hysteresis, blend tuning) extends lower-body presentation
without changing layering architecture or simulation ownership.

## A6 follow-up (ADR-077)

Scaling and validation reuse the A4 layering model: LOD throttles intent derivation and
transition work only; layer playback and mask groups are unchanged at **Full** LOD.
Shared `AnimationGraph` handles are keyed by profile + glTF + default weapon.

A1 audit fixes (pause timers, graph mapping, corpse late-install, blend-in/out) are documented in ADR-077.

## References

- ADR-074 (animation foundation A1–A3)
- ADR-076 (locomotion polish A5)
- ADR-077 (scaling, LOD, validation A6)
- ADR-069 (combat design — animation fits simulation)
- `src/units/animation/layers.rs`
- `src/units/animation/layered_playback.rs`
