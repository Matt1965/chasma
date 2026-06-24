# ADR-048: World Inspector Debug System

# Status

Accepted (U-DEV2 — read-only introspection)

# Context

Dev mode (ADR-043/047) provides authoring tools; U-UI3 overlays visualize paths,
steering, and formations. Neither gives a consolidated SC2-style **microscope**
for a single unit's simulation state at runtime.

Inspectors must not mutate [`WorldData`](../src/world/data.rs) or alter movement,
pathfinding, steering, formation, intent, or command resolution.

# Decision

## Philosophy

SC2-style debugging: **observe, don't edit**. The inspector is a read-only lens
on authoritative simulation data, not a second simulation.

## Architecture

```text
WorldData (read-only borrow)
        ↓
capture_unit_inspector_snapshot / capture_interaction_inspector_snapshot
        ↓
WorldInspectorState (cached snapshot + selection)
        ↓
Dev panel Inspector tab + InspectorOverlayFocus → U-UI3 overlays
```

| Component | Ownership | Role |
|-----------|-----------|------|
| [`capture.rs`](../src/dev/inspector/capture.rs) | Dev inspector | Pure snapshot builders |
| [`WorldInspectorState`](../src/dev/inspector/state.rs) | Dev plugin | Selection + cached snapshot |
| [`InspectorOverlayFocus`](../src/debug/inspector_focus.rs) | Debug plugin | Overlay highlight link |
| Dev Inspector tab UI | Dev panel | Text presentation only |

## Snapshot model

Snapshots are **copies** taken at selection or pause edge — not live views.
While simulation runs, the cache is stable (no per-frame recompute). When pause
toggles or selection changes, one refresh captures current truth.

Sections per unit:

- Identity + state (Idle/Moving)
- Path (waypoints, index, segment, length, chunk transitions)
- Formation (slot, offset, target, spacing)
- Steering (separation, cohesion, alignment, final direction, neighbors)
- Block diagnosis (terrain, doodad, slope)
- Chunk residency

Interaction probe (terrain click in dev mode): U6 classification + resolved
U-UI5 order plan — read-only.

## Overlay linking

[`InspectorOverlayFocus`](../src/debug/inspector_focus.rs) carries the inspected
[`UnitId`](../src/world/unit/id.rs) and optional path waypoint index. Selection
and path overlays draw an **orange** highlight when focus differs from gameplay
selection, linking panel ↔ world without changing simulation.

## Input

| Condition | Action |
|-----------|--------|
| Dev mode (F12) + left-click unit | Inspect unit |
| Alt + left-click unit | Inspect (works outside dev mode) |
| Dev mode + left-click terrain | Interaction probe |

Inspector input runs before dev spawn and gameplay intent collection. Does not
modify [`SelectedUnits`](../src/units/input/selection.rs) unless gameplay already
selected separately.

## Performance

- No per-frame capture for running simulation
- Refresh on selection change or pause edge only
- In-memory snapshot; no disk persistence

# Consequences

## Positive

- Deep runtime diagnosis without simulation coupling
- Overlay ↔ inspector visual link
- Testable pure capture functions

## Negative

- Running sim: snapshot may lag live state until pause
- Alt-only inspect has no panel unless dev mode open on Inspector tab

# Non-goals

- Gameplay edits, AI/combat tools, replay, disk persistence

# References

- ADR-039 Debug Overlay (U-UI3)
- ADR-042 Interaction Query (U6)
- ADR-035 Formation (U10)
- ADR-036 Steering (U11)
- ADR-047 Dev Mode Polish
