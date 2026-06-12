# ADR-010: Terrain Runtime Layer Boundaries

# Status

Accepted

# Context

Phase 1 established the World Data Layer (`src/world/`): the authoritative
`WorldData` resource holding per-chunk `ChunkData` (heightfield, metadata, masks),
the coordinate model, and `WorldConfig` (ADR-001, ADR-002, ADR-008).

Phase 2 (ROADMAP "Terrain Runtime") adds terrain mesh generation, chunk loading,
streaming/lifecycle, and LOD. These are runtime, derived, and disposable concerns.

ARCHITECTURE lists a "Terrain Layer" containing "terrain height data, terrain
chunk data, terrain LOD data, terrain mesh generation." Read literally, that would
place authoritative height data in a separate layer from the World Data Layer.
ADR-008 already placed the authoritative heightfield inside `ChunkData` in the
World Data Layer. This ADR resolves that tension rather than reinterpreting it
silently (AGENTS.md: identify conflicts with architecture).

# Decision

## A distinct Terrain Runtime Layer

Introduce a Terrain Runtime Layer at `src/terrain/`, separate from the World Data
Layer at `src/world/`. It is registered by a `TerrainRuntimePlugin` in the
`AppPlugin` composition root, after `WorldFoundationPlugin` (ADR-007).

## Ownership split

- **World Data Layer owns authoritative terrain truth.** The heightfield,
  metadata, and (future) masks stay in `WorldData`/`ChunkData` exactly as ADR-008
  defines. The ARCHITECTURE "Terrain Layer" wording is interpreted as: the
  authoritative *height data* lives in the World Data Layer; the Terrain Runtime
  Layer owns *terrain representation* (meshes, LOD, residency), not the truth.
- **Terrain Runtime Layer owns derived, disposable runtime state only:** mesh
  generation, LOD selection, the streaming/residency process, and the render
  entities for chunks. It holds no authoritative world data.
- **Rendering owns no authoritative state** (ARCHITECTURE Principle 3, ADR-004).

## One-way data flow

```
World Data Layer (authoritative)      Terrain Runtime Layer (derived)
  WorldData.ChunkData.heightfield  ──►  mesh builder ──► Mesh ──► render entity
```

- Mesh generation is a pure function of authoritative data.
- No system answers a query (`height_at`, `chunk_at`, `is_chunk_loaded`) by
  reading meshes or render entities; queries read `WorldData` only (ADR-005).
- The Terrain Runtime Layer depends on the World Data Layer; the World Data Layer
  never depends on the Terrain Runtime Layer or on rendering.

## Build in vertical slices

Phase 2 is implemented as vertical slices that prove this architecture end to end
before adding breadth. Phase 2A implements the smallest complete correct path
(asset → load → `WorldData` insert → pure mesh generation → derived render
entity). Streaming and LOD breadth come in later slices (ADR-012, ADR-013). This
ordering is a deliberate decision: prove ownership and data flow first, not visual
breadth.

# Rationale

Keeping authoritative data in one layer and derived representation in another is
the core architectural guarantee of the project (heightfield = truth, mesh =
visualization). A separate runtime layer makes "rendering is replaceable" literal:
the entire `src/terrain/` layer could be swapped without touching world data.

# Consequences

Benefits:

- Clear, enforceable boundary between truth and visualization.
- Future renderer/streaming changes are contained in `src/terrain/`.
- Queries remain data-backed, not mesh-backed.

Costs:

- Slight indirection: render entities reference chunks by `ChunkId` key and must
  be rebuilt from data when terrain changes.
- The World Data Layer must expose enough (read access to `ChunkData`) for the
  runtime layer to derive meshes.

# Alternatives Considered

## Put height data in the Terrain Layer per the literal ARCHITECTURE wording

Rejected: it would move authoritative data out of `WorldData`, contradicting
ADR-008 and splitting the source of truth across layers.

## Generate meshes inside the World Data Layer

Rejected: it couples authoritative data to rendering concerns and violates
ADR-004 and Principle 3.

# Notes

This ADR defines boundaries only. The asset format (ADR-011), loading/streaming
(ADR-012), and mesh/LOD (ADR-013) decisions sit inside the Terrain Runtime Layer
this ADR establishes.
