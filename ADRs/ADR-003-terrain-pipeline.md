# ADR-003: External Terrain Pipeline

# Status

Accepted

# Context

The project is not a terrain sculpting tool.

Terrain creation should occur outside the runtime using tools such as:

- Gaea
- Houdini
- World Machine
- GIS/DEM pipelines
- custom generation tools

The runtime should import externally-authored data and convert it into streamed runtime representations.

The previous project experienced visible stair-stepping when using lower-precision heightmap data.

# Decision

Use high-precision external terrain data as the authoritative source.

The preferred terrain input is:

- OpenEXR (.exr) floating-point heightfield
- exported terrain masks
- exported terrain color/material maps where useful

The current expected workflow is:

```text
Gaea
  ↓
EXR Heightfield
  ↓
Terrain Masks / Color Maps
  ↓
Runtime Import
  ↓
Chunk Data
  ↓
Generated Terrain Meshes
  ↓
Rendering

Terrain meshes are derived data.

Heightfield and terrain metadata are authoritative data.

Rationale

EXR/floating-point heightfields avoid terrain quantization artifacts common with low-precision heightmaps.

Keeping heightfield data authoritative makes future queries easier:

height lookup
slope lookup
terrain normal lookup
placement validation
future pathfinding

Mesh-first terrain would make gameplay queries depend on rendered geometry or mesh spatial search.

Consequences

Benefits:

High terrain precision
Better terrain queries
Better runtime LOD generation
Better future pathfinding compatibility
Better separation between data and rendering

Costs:

Runtime importer required
Mesh generation required
Larger source data
More terrain pipeline work
Alternatives Considered
Mesh-first terrain source

Rejected as authoritative terrain source.

Mesh exports may still be used for:

landmarks
cliffs
caves
special terrain assets
far terrain overview meshes
preview geometry

They should not replace the authoritative heightfield.

Low-precision image heightmaps

Rejected because they can introduce visible quantization and stair-stepping.

Notes

The exact terrain sample scale remains undecided:

1m per sample
2m per sample

Higher source detail is preferred when storage and build time allow it.