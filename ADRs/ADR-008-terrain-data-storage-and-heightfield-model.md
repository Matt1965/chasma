# ADR-008: Terrain Data Storage and Heightfield Model

# Status

Accepted

# Context

Phase 1 (ROADMAP, World Data Layer) makes terrain data authoritative and
loadable. ADR-003 establishes that a high-precision floating-point heightfield
is the authoritative terrain source and that meshes are derived, disposable
data. ADR-002 establishes 256 m chunks that own terrain data references. ADR-006
establishes a finite, 2D (XZ) chunk grid. ADR-001 (addendum) fixes 1 unit = 1 m,
minimum-corner chunk origins, and the coordinate model.

What remains undecided, and what this ADR settles, is the concrete shape of the
authoritative terrain data:

- how the single externally-authored source heightfield maps to per-chunk data
- the in-memory layout of a chunk's heightfield
- how many samples a chunk has, and how chunk edges are shared
- how height is sampled between samples
- what terrain metadata and mask data the World Data Layer owns at this phase

These choices are load-bearing for Phase 2 (mesh generation, LOD, streaming) and
are difficult to change later, so they are decided before implementation.

# Decision

## Per-chunk heightfield tiles

The authoritative runtime terrain representation is a set of **per-chunk
heightfield tiles**, not a single world-spanning heightfield held in memory.

- The source heightfield (ADR-003) is partitioned at import into one
  `Heightfield` per chunk, keyed by `ChunkId`.
- Per-chunk tiles align with the chunk model (ADR-002), enable chunk-local
  operations and future streaming (Phase 2), and avoid whole-world scans
  (ARCHITECTURE Scalability Rule).

## Sample resolution

- A chunk's sample spacing is `WorldConfig::meters_per_sample` (ADR-003 addendum:
  provisionally 1 m).
- A chunk covers `chunk_size_meters` per edge (ADR-002: 256 m).
- Samples per chunk edge is therefore `chunk_size_meters / meters_per_sample + 1`
  (see edge sharing below). With the provisional defaults this is `257`.
- `chunk_size_meters / meters_per_sample` must be a positive integer. Import
  rejects configurations where it is not.

## Edge sharing (seam model)

- Adjacent chunks **share their boundary samples**: each chunk stores `N + 1`
  samples per edge, where `N = chunk_size_meters / meters_per_sample`.
- The sample at local coordinate `0` of a chunk equals the sample at local
  coordinate `chunk_size` of its lower neighbor (same world position, same
  height value).
- Rationale: shared, identical edge samples guarantee that Phase 2 meshes from
  neighboring chunks meet exactly, preventing terrain cracks/seams. Duplicated
  (independent) edges were rejected because they permit divergent edge heights.

## In-memory layout

- A `Heightfield` owns a row-major `Vec<f32>` of length `(N + 1) * (N + 1)`,
  plus `samples_per_edge` and `spacing_meters`.
- Indexing is `row * (N + 1) + col`, where `col` advances along +X and `row`
  advances along +Z, consistent with the XZ grid and minimum-corner origin
  (ADR-001 addendum).
- Heights are `f32` and authoritative (ADR-003). They are never quantized to a
  lower-precision type for storage.

## Sampling semantics

- `Heightfield::sample(local_x, local_z)` returns height via **bilinear
  interpolation** between the four surrounding samples.
- Inputs are clamped to the chunk's `[0, chunk_size]` domain. Sampling outside a
  chunk is the caller's responsibility (resolve the correct chunk first via the
  coordinate model).
- Bilinear is the baseline. Higher-order interpolation is an optimization to be
  introduced only with evidence (ARCHITECTURE Performance Philosophy).

## Terrain metadata

- The World Data Layer owns a small `TerrainMetadata` per chunk, computed at
  import: `height_min` and `height_max`.
- Justified consumers: Phase 2 chunk bounds / AABB and culling, and import-time
  validation. No biome, material, or slope caches are stored (ADR-005 marks such
  queries as internal/deleted; no current consumer).

## Terrain masks

- The World Data Layer defines a `TerrainMask` data container: per-sample values
  for one named mask layer, referenced by chunk data.
- Masks are **data only** in Phase 1. They are imported if present and otherwise
  absent. No mask-processing system exists yet (consumers are Phase 2 material
  blending and Phase 3 doodad placement). This is a data seam, not a system
  (AGENTS.md Groundwork Rule).

## Ownership

- `WorldData` (a resource) owns `ChunkId -> ChunkData` and the finite world
  extent (min/max chunk coordinates), discovered at import (ADR-006).
- `ChunkData` (the chunk definition, ADR-002) owns its `Heightfield`,
  `TerrainMetadata`, and `TerrainMask`s **inline** in Phase 1 (no streaming yet).
- `ChunkData` must not own doodads, occupancy, LOD state, or mesh handles. Those
  are later phases; ADR-002's recorded allowance is the seam, not a field.

# Rationale

Per-chunk tiles with shared edges match the chunk model, keep operations
chunk-local, and make Phase 2 meshing seam-free without rework. Keeping height as
authoritative `f32` honors ADR-003 and avoids the stair-stepping that motivated
it. Restricting metadata and masks to data with near-term consumers honors the
"build seams, not fake future systems" rule while still delivering the ROADMAP
Phase 1 deliverables.

# Consequences

Benefits:

- Seam-free terrain meshes in Phase 2 (shared edges)
- Chunk-local data, ready for Phase 2 streaming
- Clean authoritative-vs-derived separation (heightfield is truth)
- Minimal, consumer-justified metadata and mask surface

Costs:

- Edge samples are stored in both neighboring chunks (small memory overhead)
- Import must validate that chunk size is an integer multiple of sample spacing
- A single in-memory world heightfield is intentionally not available (callers
  must go through chunk tiles)

# Alternatives Considered

## Single world-spanning heightfield in memory

Rejected for the runtime representation: it does not scale to large worlds, works
against chunk-local streaming, and invites whole-world scans (Scalability Rule).
The source data may still be a single file; this ADR governs the runtime
representation, not the source format (ADR-003).

## Duplicated (independent) chunk edges

Rejected: independent edge samples can diverge between neighbors and produce
terrain cracks in Phase 2 meshes.

## Storing heights at lower precision

Rejected per ADR-003 (quantization artifacts).

# Notes

The provisional 1 m sample spacing (ADR-003 addendum) may change; because spacing
is owned by `WorldConfig` and tiles are derived at import, changing it does not
alter these structures, only their dimensions.
