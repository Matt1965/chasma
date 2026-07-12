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
- Simulation-facing or rotation mutation
- Combat timing changes
- Overlay layer behavior (ADR-075 future work)

## References

- ADR-074 (animation foundation)
- ADR-075 (layering)
- ADR-077 (A1 audit stabilization — pause timers, blend-in/out)
- ADR-069 (combat design — simulation owns facing until gameplay ADR)
- `src/units/animation/locomotion_polish.rs`
- `docs/animation-authoring.md`
