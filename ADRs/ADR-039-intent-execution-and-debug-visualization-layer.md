# ADR-039: Intent Execution + Debug Visualization Layer (U-UI3)

# Status

Accepted (U-UI3 — intent integration and debug observability)

# Context

ADR-038 (U-UI2) introduced the client intent pipeline:

```text
Input → ClientIntentQueue → dispatch → command APIs → WorldData
```

U-UI2 established the architecture but left observability mostly console-based
(`debug_intents`, `debug_unit_interaction`). Dispatch reports were stored in
`Local` state and path gizmos lived in the player presentation module without
a unified debug layer.

U-UI3 requires the pipeline to be **real, visible, and debuggable** without
changing simulation rules, movement, pathfinding, steering, or formation logic.

# Decision

## Full pipeline (unchanged contract from ADR-038)

```text
Input → Intent Queue → Dispatcher → Command APIs → WorldData → Presentation
                              ↓              ↓
                     IntentDispatchHistory   CommandTraceBuffer
                              ↓
                     Debug overlay (read-only gizmos)
```

## Command trace architecture

| Component | Responsibility |
|-----------|----------------|
| [`CommandTraceBuffer`](../src/debug/trace.rs) | Ring buffer (256 entries) of intent/command events |
| [`PendingDispatchTrace`](../src/debug/dispatch_pending.rs) | Batch written by dispatcher, flushed by trace system |
| [`PendingSimulationTrace`](../src/debug/pending.rs) | Command-buffer resolve report from movement tick |
| [`IntentDispatchHistory`](../src/debug/trace.rs) | Last frame's intent dispatch report for overlays |
| [`ClientFrameIndex`](../src/debug/trace.rs) | Monotonic client frame counter |

Trace entries record:

- tick index and sequence
- intent kind
- affected unit IDs
- resulting [`UnitOrder`] when applicable
- outcome (applied, ignored, queued, resolved, failed)
- optional path waypoint count after resolve

**Rules:**

- Simulation/command paths emit trace data; overlays **read only**
- Trace buffer rotates safely; max 64 entries per tick
- Duplicate entries (same tick, intent, units, order, outcome) are suppressed

## Debug overlay system (`src/debug/overlay/`)

| Module | Visualization |
|--------|---------------|
| `intent_overlay.rs` | Move-command target markers from last dispatch |
| `path_overlay.rs` | Waypoint polylines + active segment highlight |
| `formation_overlay.rs` | Unit → formation target lines |
| `steering_overlay.rs` | Sampled separation + cohesion vectors |
| `selection_overlay.rs` | Extra gizmo rings (complements mesh indicators) |

[`DebugOverlaySettings`](../src/debug/settings.rs) provides per-category toggles,
a master switch, and `max_draw_units` (default 64) to cap draw cost.

## Production gating (REVIEW-A6)

| Build | Debug overlay draw systems | Default config |
|-------|---------------------------|----------------|
| Default / production | Not registered (`feature = "dev"`) | [`DebugOverlayConfig::production()`](../src/debug/settings.rs) — all categories off |
| Dev | Registered in [`DebugOverlayPlugin`](../src/debug/plugin.rs) with per-category `run_if` | Categories off until Dev Mode toggles |

**Gameplay presentation** (selection mesh rings, box-select marquee, move-command
marker/ping, cursor feedback) remains registered in [`PlayerPlugin`](../src/player/plugin.rs)
and is **not** gated by debug overlay settings.

**Command trace emission** (`CommandTraceBuffer`, flush systems) remains active in all
builds; only **visualization** (intent/combat overlays, inspector panels) is dev-gated.

## Read-only overlay rule (REVIEW-A6)

[`InteractionDebugSnapshot`](../src/debug/interaction_snapshot.rs) is a **client-local
debug resource** (not [`WorldData`](../src/world/data.rs)). [`capture_interaction_debug_snapshot`](../src/debug/interaction_capture.rs) populates it from dispatch history;
[`draw_interaction_debug_overlay`](../src/debug/overlay/interaction_overlay.rs) reads only.

## Schedule (player control)

```text
advance_client_frame_index
  → tick_unit_movement
  → flush_simulation_command_trace
  → collect_unit_input_intents
  → dispatch_client_intents
  → flush_intent_dispatch_trace
  → [dev] capture + debug overlays (chain, run_if per category)
  → presentation sync (selection rings, move marker, box select)
```

## Separation of concerns

| Layer | May mutate | Must not |
|-------|------------|----------|
| Input collection | `ClientIntentQueue`, `BoxSelectDrag` | `WorldData`, `SelectedUnits` |
| Intent dispatch | `SelectedUnits`, command APIs | Direct rendering |
| Simulation | `WorldData` | Input devices, overlays |
| Debug overlays | Gizmos + client-local debug resources only | `WorldData`, command issuance |

[`ClientBoundaryGuard`](../src/debug/boundaries.rs) debug-asserts that input
collection and intent dispatch do not overlap.

## SC2-style observability model

Trace-first debugging: every player action produces inspectable records before
visual feedback. This enables future replay (record intents), AI control (emit
intents), multiplayer sync, and deterministic audit tools without changing the
U-UI2 intent contract.

# Consequences

**Benefits:**

- End-to-end intent → command → simulation flow is observable at runtime
- Debug visuals are toggleable and capped; simulation remains authoritative
- Dispatcher stays within Bevy system param limits via pending-trace flush

**Costs:**

- Additional resources and two flush systems per frame
- Steering/formation overlays re-sample data (read-only, capped)

# References

- ADR-038 (intent pipeline)
- ADR-033 / ADR-034 (player control and selection)
- ADR-035 / ADR-036 / ADR-037 (formation, steering, movement feel)

[`UnitOrder`]: ../src/world/unit/orders.rs
[`CommandTraceBuffer`]: ../src/debug/trace.rs
[`DebugOverlaySettings`]: ../src/debug/settings.rs
