# ADR-040: Gameplay UI Layer Architecture (U-UI4)

# Status

Accepted (U-UI4 — SC2-style gameplay UI foundation)

# Context

U-UI2 established the intent pipeline; U-UI3 added debug trace and developer
overlays. Player-facing presentation (selection rings, move marker) lived in the
player layer without a dedicated gameplay UI boundary.

U-UI4 introduces the first **player experience** UI layer: minimal HUD, command
feedback, cursor context, and selection readability — without exposing debug
complexity or touching simulation rules.

# Decision

## Three-layer split

```text
Simulation (WorldData)     — authoritative truth
Debug layer (U-UI3)        — dev observability (trace, gizmos)
Gameplay UI (U-UI4)        — player feedback (HUD, markers, cursor mode)
```

Gameplay UI **never** writes `WorldData`, **never** mutates selection, and
**never** reads debug overlay state except the dev-only `DebugOverlaySettings.enabled`
flag for a small HUD badge.

## Module layout (`src/ui/gameplay/`)

| Module | Responsibility |
|--------|----------------|
| [`state.rs`](../src/ui/gameplay/state.rs) | `GameplayUiState`, snapshot derivation |
| [`selection_ui.rs`](../src/ui/gameplay/selection_ui.rs) | Sync from client sources |
| [`command_feedback.rs`](../src/ui/gameplay/command_feedback.rs) | Move marker + command ping |
| [`cursor_feedback.rs`](../src/ui/gameplay/cursor_feedback.rs) | Logical cursor mode from hover |
| [`hud.rs`](../src/ui/gameplay/hud.rs) | Screen-space HUD widgets |

## State consumption model

Gameplay UI reads **only**:

- [`SelectedUnits`](../src/units/input/selection.rs) — selection count, leader id
- [`IntentDispatchHistory`](../src/debug/trace.rs) — last-frame dispatch results
- [`CommandTraceBuffer`](../src/debug/trace.rs) — filtered gameplay-visible entries
- Hover context from intent-layer picking helpers (read-only `WorldData` for ray pick)

Command state (`Idle` / `Moving`) derives from applied `MoveCommand` intents and
filtered trace entries — not from ad-hoc simulation polling.

## Schedule placement

```text
… → flush_intent_dispatch_trace → sync_gameplay_ui_state → HUD/feedback sync
collect_unit_input_intents → sample_gameplay_cursor_context
```

Gameplay feedback runs **after** intent trace flush so HUD reflects the same frame's
commands. Cursor context samples **after** intent collection.

## SC2-style philosophy

- **Minimal HUD** — count, command label, portrait placeholders, optional DBG badge
- **Strong feedback loops** — grounded move marker, command ping pulse, selection rings (player layer)
- **Intent-driven updates** — UI mirrors client dispatch, not raw input devices

## Command feedback

[`MoveCommandFeedback`](../src/ui/gameplay/command_feedback.rs) owns the destination
marker and a short ping pulse. The intent dispatcher sets the marker target when
`MoveCommand` is applied (presentation hook only; order issuance unchanged).

## Separation from debug

Debug overlays (paths, steering vectors, resolve traces) remain in `src/debug/`.
Gameplay UI does not consume debug gizmo data or show internal trace complexity.

# Consequences

**Benefits:**

- Clear player vs developer presentation boundary
- Replay-ready UI sync from intent history + trace
- HUD updates only on state change (`hud_dirty` flag)

**Costs:**

- Some presentation moved from `player/` to `ui/gameplay/`
- OS cursor icon sync deferred (logical `GameplayCursorMode` only for now)

# References

- ADR-038 (intent pipeline)
- ADR-039 (debug visualization)
- ADR-033 / ADR-034 (player control and selection)

[`GameplayUiState`]: ../src/ui/gameplay/state.rs
[`MoveCommandFeedback`]: ../src/ui/gameplay/command_feedback.rs
