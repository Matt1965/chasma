# ADR-076: Advanced Locomotion and Animation Polish (A5 / D5)

## Status

Accepted (A5 / D5)

## Context

ADR-074 A1 established Idle/Walk/Run locomotion with playback speed scaling.
ADR-075 A4 added masked lower/upper layering for simultaneous move + attack.

Units still felt robotic: abrupt walk/run toggles, no turn presentation, constant
playback speed while heading misaligned with movement, and coarse cross-fades.

Simulation rotation and movement remain authoritative in `WorldData`. This phase
is **presentation-only** — no gameplay, combat timing, or root motion changes.

ADR-070 is reserved for progression and attributes; locomotion polish is documented here.

## Decision

### Locomotion polish module

`src/units/animation/locomotion_polish.rs` owns per-unit presentation state:

| Field | Purpose |
|-------|---------|
| `last_locomotion_clip` | Walk/run hysteresis |
| `smoothed_speed` | Playback speed smoothing |
| `turn_remaining_seconds` | Turn-in-place hold timer |
| `active_turn_clip` | Current turn clip key |

State is stored in `UnitAnimationPersistedState.locomotion` and survives render
entity recreation (same index as clip persistence).

### Walk / run hysteresis

Separate enter/exit ratios on `UnitAnimationSettings`:

- Enter Run: `speed >= reference * run_enter_ratio` (default 0.75)
- Exit Run: `speed < reference * run_exit_ratio` (default 0.65)

Prevents flicker at the run threshold without changing simulation speed.

### Playback speed

- Base: `move_speed_mps / locomotion_reference_speed_mps * locomotion_speed_scale`
- Heading misalignment slows playback (`foot_slide_max_slowdown`) — mitigates visible
  foot slide without root motion or simulation rotation changes.
- Live `set_speed` when clip unchanged and delta > `speed_update_epsilon`.

### Turning (presentation only)

Compare model forward (`placement.rotation`, **-Z** in local space) to stabilized
movement heading from the active path segment.

| Condition | Behavior |
|-----------|----------|
| Idle + heading delta ≥ `turn_in_place_degrees` | Optional `TurnLeft` / `TurnRight` clip |
| Moving + delta ≥ `turn_adjust_degrees` | Same (higher threshold) |
| Missing turn clips | Skip turn; locomotion continues |

Turn duration from profile `turn_*_duration_seconds` or `turn_default_seconds`.
Timer-driven — not animation events. Simulation does not rotate for turns.

### Transitions

Context-specific blend durations on `UnitAnimationSettings`:

- `accel_blend_ms` — Idle→Walk, Walk→Run
- `decel_blend_ms` — Run→Walk, Walk→Idle
- `stop_blend_ms` — movement stop
- `turn_blend_ms` — turn clips
- `default_blend_ms` — fallback

Lower-body clips use `AnimationTransitions::play` with intent blend duration.

### Model forward axis

**Artist requirement:** glTF/Bevy default forward is **-Z** (`MODEL_FORWARD_AXIS`).
Imported assets must face -Z at bind pose for heading comparison to match visuals.
Documented in `docs/animation-authoring.md`.

### Dev Mode debug (read-only)

Debug tab shows for inspector-selected unit: clip, layers, playback speed, profile,
graph missing clips, heading delta, turn state. No editing in A5.

## Non-goals (A5)

- Root motion
- ~~Simulation-facing or rotation mutation~~ *(superseded 2026-08-18 — see below)*
- Combat timing changes
- Overlay layer behavior (ADR-075 future work)

## Supersession — movement-facing authority (2026-08-18, UNIT-FACING-1)

Normal successful unit movement now updates authoritative `UnitPlacement.rotation`
in `WorldData` from **accepted actual XZ displacement** each simulation step
(`src/world/unit/facing.rs`, applied in `step_unit_movement`).

| Topic | Decision |
|-------|----------|
| Facing source | Final accepted travel vector (previous → new world position), not click target or raw path heading |
| Model forward | Local **-Z** unchanged |
| Blocked / no displacement | Preserve existing rotation |
| Portal / teleport relocation | Preserve rotation; no facing from discontinuous jump |
| Render | `Transform.rotation` composes smoothed visual yaw × asset rotation correction (UNIT-TURN-1 / UNIT-FACING-2) |
| Turn clips | Presentation-only; do **not** mutate simulation rotation |
| Visual turn rate | Per-unit `Turn Speed Deg/s` — presentation yaw only; movement unconstrained (UNIT-TURN-1) |
| Combat-facing (COMBAT-FACING-1) | While **Attacking** and not **Moving**, authoritative yaw updates toward the current combat target each engagement tick; while **Chasing**/**Moving**, accepted travel displacement remains facing authority; presentation still catches up via Turn Speed Deg/s |

Locomotion polish heading misalignment during **Moving** is intentionally skipped
(`movement_heading_delta` returns `None`) because travel-facing owns rotation while
moving. TurnLeft/TurnRight clips remain available for future stationary or
explicit turn-in-place behavior.

### Visual turn speed (UNIT-TURN-1, 2026-08-18)

| Topic | Decision |
|-------|----------|
| Authoritative facing | `UnitPlacement.rotation` updates immediately from accepted XZ travel (unchanged) |
| Visual facing | `UnitVisualFacing` on render root; yaw-only interpolation toward placement |
| Turn Speed Deg/s | Per-unit authored catalog data; default **540** when column absent/blank |
| Movement | **Not** constrained by visual turn rate — no turning radius, no move slowdown |
| Composition | `visual_world_yaw × asset_sizing.rotation_correction` (correction after smooth yaw) |
| Idle | Visual yaw continues catching up after movement stops |
| Dead | Presentation yaw frozen at death |
| Pause | Uses `presentation_advance_seconds` — zero delta while paused |

## References

- ADR-074 (animation foundation)
- ADR-075 (layering)
- ADR-077 (A1 audit stabilization — pause timers, blend-in/out)
- ADR-069 (combat design — simulation owns facing until gameplay ADR)
- `src/units/animation/locomotion_polish.rs`
- `docs/animation-authoring.md`
