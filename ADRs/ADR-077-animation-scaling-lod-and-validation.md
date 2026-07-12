# ADR-077: Animation Scaling, LOD, and Asset Validation (A6 / D6)

## Status

Accepted (A6 / D6 — final animation implementation phase)

## Context

ADR-074–076 delivered locomotion, combat animation, layering, and locomotion polish.
Before production-scale RTS workloads, the animation runtime needed:

- Measured cost awareness
- Shared graph/glTF ownership
- Presentation-only LOD for distant units
- Asset validation with graceful fallbacks
- Dev diagnostics and stress coverage

ADR-071 is reserved for creature AI architecture. Scaling/validation is documented here.

## Decision

### Cost audit findings (pre-optimization baseline)

| Work | Frequency | Notes |
|------|-----------|-------|
| `discover_unit_animation_players` | Spawn / retry only | Descendant walk, not per-frame |
| `build_unit_animation_graphs` | Once per definition (+ shared key) | After glTF load |
| `derive_layered_animation_intent` | Per unit per eval | Skipped at Frozen LOD |
| `resolve_layered_playback_targets` | Per eval | Cheap node lookup |
| `AnimationPlayer` mutation | On transition or speed tweak | Bevy advances clips between evals |
| `prune_state_index` | Per frame | O(units with animation state) |
| Dev `animation_panel` | Dev tab only | Read-only |

**Hot path:** `sync_unit_animation_playback` — O(animated render roots) per frame.
At **Full** LOD: intent derivation every frame. At **Reduced**: throttled. At **Frozen**: pause + skip derivation.

### Shared asset ownership

| Asset | Key | Sharing |
|-------|-----|---------|
| glTF handle | Asset path string | One handle per path |
| `AnimationGraph` | `AnimationGraphShareKey { profile_id, gltf_path, default_weapon_id }` | Identical definitions share one graph handle |
| Per-unit state | `UnitAnimationStateIndex` | Playback, LOD, locomotion polish only |

Definitions with the same profile, glTF path, and default weapon reuse one built graph.
Different weapons or glTF paths produce distinct share keys.

### Animation LOD policy

Presentation-only tiers in `AnimationLodSettings`:

| Tier | Distance (defaults) | Behavior |
|------|---------------------|----------|
| `Full` | ≤ 80 m | Every-frame intent eval + transitions |
| `Reduced` | 80–160 m | Intent eval every 0.25 s; Bevy playback continues |
| `Frozen` | > 160 m (cap 280 m band) | `pause_all`; no intent eval |

**Promotions to Full:** selected units, inspector focus unit, nearby attacking units, LOD tier upgrade (with `AnimationPlaybackPending` restore).

**Hysteresis:** `hysteresis_margin_meters` (12 m default) on tier boundaries.

Simulation and `WorldData` are never read or written by LOD.

### Update throttling

Throttled (presentation only):

- `derive_layered_animation_intent` at Reduced interval
- Hit-reaction detection skipped at Frozen
- Transition graph setup skipped at Frozen

Not throttled:

- Bevy `AnimationPlayer` clip advancement (engine-owned)
- Transform/render sync (separate systems)
- Simulation tick

### State restoration

On promotion (Frozen/Reduced → Full, or `AnimationPlaybackPending`):

1. Fresh intent from current `WorldData` (not stale persisted clip)
2. `force_eval` + pending marker forces safe re-transition
3. Death presentations always use Full LOD path
4. Attack replay uses live `AttackCycle`, not persisted one-shot state alone

If exact visual phase is unknown after Frozen, current locomotion/attack intent wins.

### Asset validation

`validate_definition_animation_assets` at graph build:

| Severity | Examples |
|----------|----------|
| Error | Missing profile, missing Idle clip, no glTF clips |
| Warning | Missing Walk/Run/Turn, attack/death/hit clips, no split bone |
| Info | Static model, model forward axis convention |

`AnimationValidationIndex` logs each issue once per profile/asset key.

### Failure policy

Unchanged from ADR-074: warn once, static model / idle fallback, never panic, simulation unaffected.

## A1 stabilization (post-audit)

Audit-driven corrections without architecture changes:

| Fix | Behavior |
|-----|----------|
| Death/hit graph mapping | Typed `ResolvedClipSet` — distinct death/hit nodes |
| Pause-gated timers | `presentation_advance_seconds` — death/hit timers freeze with sim |
| Step once | Timers advance by `SIMULATION_TICK_SECONDS`, not render delta |
| Late corpse graph install | `resolve_presentation_definition_id` from `UnitRenderMetadata` |
| Off-screen death | No new corpse entities; existing resident roots may finish presentation |
| Attack blend-out | `UpperAttackWeightFade` weight fade when upper attack ends |
| Attack blend-in | Upper attack starts at weight 0, fades in over weapon blend |
| Stale player links | `heal_stale_animation_player_links` → pending rediscovery |
| Weapon import validation | Non-empty, well-formed `Animation Key` required |
| Skeleton target scope | Mask assignment limited to player descendant entities |

Deferred: global skeleton scan replacement beyond descendant filter (low-risk scope applied).

## Non-goals (A6)

- Root motion, IK, motion matching
- GPU/crowd animation
- Animation editor
- Gameplay animation events

## References

- ADR-074, ADR-075, ADR-076
- `src/units/animation/lod.rs`
- `src/units/animation/validation.rs`
- `src/units/animation/assets.rs`
- `docs/animation-authoring.md`
