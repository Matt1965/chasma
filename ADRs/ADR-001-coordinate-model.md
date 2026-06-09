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

---

# Addendum: Coordinate Conventions (Phase 0)

Status: Accepted

The following conventions make the Chunk Coordinate + Local Position model concrete.

- 1 Bevy unit = 1 meter.
- The chunk grid tiles the horizontal XZ plane. Bevy's Y axis is vertical (up).
- Chunk coordinates are 2D integer coordinates (see ADR-006).
- A chunk coordinate addresses a square region of the world. Chunk `(cx, cz)`
  covers world X in `[cx * chunk_size, (cx + 1) * chunk_size)` and world Z in
  `[cz * chunk_size, (cz + 1) * chunk_size)`.
- A chunk's origin is its minimum (lowest X, lowest Z) corner, not its center.
- World origin: chunk `(0, 0)`'s minimum corner sits at global `(0, 0, 0)`.
- Local Position is relative to the chunk's minimum corner:
  - `local.x` in `[0, chunk_size)`
  - `local.z` in `[0, chunk_size)`
  - `local.y` is absolute terrain height (vertical is not chunked; see ADR-006)
- Global render-space conversion:
  - `global = (cx * chunk_size + local.x, local.y, cz * chunk_size + local.z)`
- Conversions must round-trip: `global -> (ChunkCoord, LocalPosition) -> global`.

Chunk identity:

- The chunk coordinate IS the chunk identity. There is no separately generated
  chunk id.
- This is the multiplayer- and persistence-friendly choice: identity is
  deterministic, derived purely from world position, and requires no shared id
  registry to stay consistent across machines or save/load cycles.

Rationale:

Minimum-corner origins keep tiling math simple (floor division by `chunk_size`)
and avoid half-chunk offsets in streaming and persistence. Keeping vertical
unchunked matches the heightfield terrain model (ADR-003).