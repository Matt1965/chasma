# ADR-006: Chunk Grid Dimensionality

# Status

Accepted

# Context

ADR-001 establishes a Chunk Coordinate + Local Position model but does not state
whether the chunk grid is 2D (horizontal only) or 3D (also vertical).

The terrain model is a high-precision heightfield (ADR-003). The world is
effectively 2.5D: a horizontal surface with elevation, not a volumetric space.

The world is also finite. Terrain is externally authored (ADR-003), so the chunk
grid is bounded by the extent of the imported terrain data. "Effectively
infinite view distance" refers to being able to see far across a finite world,
not to an unbounded chunk space.

# Decision

The chunk grid is 2D.

- Chunk coordinates tile the horizontal XZ plane and are 2D integer coordinates.
- The vertical (Y) axis is not chunked.
- The chunk grid is finite and bounded by the imported terrain extent.

Vertical terrain features that a heightfield cannot represent (overhangs, caves,
arches, undercut cliffs) are handled as special mesh assets, not by introducing
vertical chunking. This is consistent with ADR-003's allowance for mesh exports
for landmarks, cliffs, and caves.

# Rationale

A 2D grid matches a heightfield world, keeps coordinate math and streaming
simple, and avoids the large complexity cost of volumetric/3D chunking that no
current or roadmapped feature requires (AGENTS.md Groundwork Rule).

Special mesh assets cover the rare vertical features without forcing every chunk
and every system to carry a third spatial dimension.

A finite, bounded grid simplifies bounds checking, persistence, validation, and
future multiplayer replication.

# Consequences

Benefits:

- Simpler coordinate model and streaming
- Smaller per-chunk bookkeeping
- Matches heightfield terrain authoring
- Finite grid simplifies bounds, persistence, and validation

Costs:

- True volumetric terrain is not supported by chunks
- Overhangs/caves require separate authored mesh assets with their own handling

# Alternatives Considered

## 3D / volumetric chunk grid

Rejected. No current or roadmapped feature needs volumetric terrain, and it would
add significant complexity to every spatial system.

# Notes

If a future feature genuinely requires volumetric grouping, it can introduce its
own spatial structure. ADR-002 already allows future systems to use spatial
groupings other than the terrain chunk grid.
