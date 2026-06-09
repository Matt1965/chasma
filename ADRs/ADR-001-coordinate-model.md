# ADR-001: Authoritative World Coordinate Model

# Status

Accepted

# Context

The project targets a large world that may eventually support:

- large terrain datasets
- chunk streaming
- high-altitude viewing
- persistent simulation
- pathfinding
- multiplayer compatibility

Large global floating-point coordinates can lose precision as distance from origin increases.

This can cause future problems with:

- camera jitter
- movement precision
- projectile precision
- placement accuracy
- simulation consistency
- persistence
- multiplayer synchronization

# Decision

The authoritative world position model is:

- Chunk Coordinate
- Local Position

Chunk coordinates are integer coordinates.

Local positions are floating-point positions relative to the chunk.

Rendering may use local floating-point transforms as needed.

World data, simulation data, persistence, and queries should use chunk-relative positions as the source of truth.

# Rationale

This model supports large worlds without requiring all systems to rely on giant global floating-point coordinates.

It also aligns naturally with chunk streaming and persistence.

# Consequences

Benefits:

- Better large-world precision
- Cleaner chunk ownership
- Better persistence model
- Better future multiplayer compatibility
- Easier chunk-local queries

Costs:

- More coordinate conversion code
- Systems must be careful about global vs chunk-local positions
- Debugging positions may be slightly more complex

# Alternatives Considered

## Global Vec3 world coordinates

Rejected as authoritative model because it risks precision problems at large world sizes.

## Floating origin only

Rejected as the sole solution because it primarily solves rendering/camera precision, not persistence or simulation ownership.

# Notes

Floating origin may still be used later for rendering.

It should not replace chunk-relative authoritative positioning.