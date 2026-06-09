# ADR-002: Terrain Chunk Model

# Status

Accepted

# Context

The runtime needs to stream, render, and query a large externally-authored world.

The chunk model must support:

- terrain streaming
- terrain LOD
- authored doodads
- procedural doodads
- exclusion zones
- future occupancy
- future pathfinding
- future persistence

The project previously considered larger chunks such as 512m.

The current preferred chunk size is 256m.

# Decision

Use 256m x 256m terrain chunks as the default terrain chunk size.

Chunks represent geography.

Chunks may own:

- terrain data references
- terrain metadata references
- mask references
- terrain LOD state
- authored doodad references
- procedural doodad cache
- occupancy metadata

Chunks must not own:

- units
- settlements
- factions
- caravans
- economies
- reputation systems

# Rationale

256m chunks provide better streaming and gameplay granularity than 512m chunks.

This is useful for:

- settlements
- authored areas
- base-building
- terrain culling
- smaller mesh rebuilds
- smaller doodad groups

Chunk size should be chosen for terrain streaming, not for every future system.

Future systems may use different spatial groupings.

Examples:

- occupancy cells smaller than chunks
- simulation regions larger than chunks
- far terrain tiles larger than chunks

# Consequences

Benefits:

- Better culling granularity
- Better authored-area granularity
- Better future gameplay compatibility
- Smaller per-chunk rebuilds

Costs:

- More chunks overall
- More bookkeeping
- More streaming decisions
- More LOD seam management

# Alternatives Considered

## 512m terrain chunks

Rejected as the default because it may be too coarse for future settlement, occupancy, and authored gameplay areas.

May still be useful for far terrain aggregation.

## Smaller terrain chunks

Deferred because they increase management overhead and seam complexity.

# Notes

Chunk size may become configurable later.

The architecture should not assume every future system uses terrain chunk size.