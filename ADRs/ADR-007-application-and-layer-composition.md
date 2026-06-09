# ADR-007: Application and Layer Composition

# Status

Accepted

# Context

Phase 0's primary deliverable is application structure, plugin structure, and
plugin boundaries (ROADMAP Phase 0). ARCHITECTURE defines world layers (World
Data, Terrain, Doodad, Occupancy, Rendering, Gameplay, Future Simulation) but
does not define how those layers map to Bevy plugins or how the application is
composed.

The AGENTS.md Groundwork Rule requires building seams, not fake future systems.

# Decision

The application is composed as a tree of plugins with a single composition root.

- `main.rs` is a thin binary entry point. It builds the Bevy `App`, adds
  `DefaultPlugins` (a runnable shell is acceptable in Phase 0), adds `AppPlugin`,
  and runs. No system or composition logic lives in `main`.
- `AppPlugin` (`src/app`) is the composition root. It is the only place that
  registers layer plugins, in dependency order.
- Each architectural layer becomes its own plugin, added to `AppPlugin`, only
  when that layer has real content. Empty placeholder layer plugins are not
  created in advance.

Phase 0 plugin tree:

```text
AppPlugin
  └─ WorldFoundationPlugin   (src/world)
```

`WorldFoundationPlugin` owns the World Data Layer foundation: the coordinate
model (ADR-001), chunk identity (ADR-002), and `WorldConfig`. It registers the
foundational data types for Bevy reflection.

Reflection vs serialization:

- Foundational data types derive and register Bevy `Reflect` in Phase 0. This is
  cheap and is a legitimate seam for future inspection, persistence, and
  multiplayer.
- `serde` serialization is deferred until persistence needs are real (ROADMAP
  Phase 7). It is not added in Phase 0.

# Rationale

A single composition root keeps layer ordering and wiring in one place, matches
the layered architecture, and keeps `main` trivial. Adding layer plugins only
when they have content honors the Groundwork Rule and the roadmap phase gates,
while `AppPlugin`'s registration list provides the seam for future layers without
empty stubs.

# Consequences

Benefits:

- Clear, single place for layer wiring and ordering
- `main` stays trivial and replaceable (future headless/server builds)
- No speculative empty plugins
- Reflection seam in place without `serde` cost

Costs:

- Adding a new layer requires touching `AppPlugin` (intended, low cost)

# Alternatives Considered

## Pre-stub all layer plugins in Phase 0

Rejected. The layers have no content yet; empty plugins are fake future systems
(Groundwork Rule). The seam already exists in `AppPlugin`.

## Compose plugins directly in main.rs

Rejected. Keeps binary-specific code entangled with composition and harms reuse
for future non-windowed builds.

# Notes

This ADR governs structure only. It introduces no gameplay, terrain, rendering,
or simulation systems, consistent with Phase 0 notes in ROADMAP.
