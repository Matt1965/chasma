# ADR-015: Doodad Data Ownership

# Status

Accepted (Phase 3A — data foundation)

# Context

ROADMAP Phase 3 introduces environmental world objects (trees, rocks, ruins,
resource nodes, etc.). ARCHITECTURE treats doodads as world data first: rendering
and ECS entities are derived later (Principles 5 and 6).

ADR-008 explicitly deferred doodads:

> `ChunkData` must not own doodads, occupancy, LOD state, or mesh handles.

ADR-002 records that chunks may own authored doodad references at the geography
level. ADR-005 reserves `doodads_near(position, radius)` for a future query API.
ADR-010 separates authoritative world data from terrain runtime derived meshes.

Phase 3A establishes the **data model only**. No rendering, instancing, LOD,
procedural generation systems, streaming, or save/load format changes.

# Decision

## World Data Layer owns authoritative doodad records

All doodad instance data lives in the **World Data Layer** (`src/world/doodad/`),
stored on [`WorldData`] alongside terrain residents.

## Parallel to `ChunkData`, not inside it

[`WorldData`] holds:

```text
ChunkId -> ChunkData          (terrain — existing)
ChunkId -> ChunkDoodadStore   (doodads — new)
Vec<DoodadExclusionZone>      (world-scoped — new)
```

[`ChunkData`] remains terrain-only (heightfield, metadata, masks). Doodads must
not be added as fields on `ChunkData`.

## Chunk-local storage with authoritative positions

Each [`DoodadRecord`] is stored under the [`ChunkId`] that owns its
[`WorldPosition`] (ADR-001). [`WorldData::insert_doodad`] rejects records whose
placement chunk does not match the target bucket.

Doodads may exist for a chunk **without** resident [`ChunkData`]. Evicting terrain
via [`WorldData::remove`] does **not** remove doodad records in Phase 3A.

## Stable instance identity

[`DoodadId`] is a monotonic `u64` newtype assigned by [`WorldData`]. It is **not**
coordinate-derived (unlike [`ChunkId`]).

## Source distinction

[`DoodadSource`] distinguishes `Authored` from `Procedural { seed }` so future
persistence can treat procedural output as baseline and gameplay changes as
overrides (ARCHITECTURE Persistence Rule).

## Exclusion zones are data-only

[`DoodadExclusionZone`] stores `center: WorldPosition` and `radius_meters: f32`
on [`WorldData`]. No generation or query behavior in Phase 3A.

## Metadata placeholder

[`DoodadMetadata`] is an empty struct in Phase 3A. Future harvest/depletion/
regrowth state extends metadata without changing record identity fields.

## Doodad runtime deferred

Rendering, instancing, LOD, ECS entity promotion, streaming, procedural
generation, and save/load formats are **out of scope** for Phase 3A. A future
`DoodadRuntimePlugin` (or equivalent) in a separate layer will consume this
data, mirroring ADR-010's terrain split.

The terrain runtime layer must not import doodad types until a later phase
requires cross-layer integration.

# Rationale

Keeping doodads in `WorldData` but outside `ChunkData` honors ADR-008, supports
chunk-keyed persistence and future streaming, and preserves the heightfield =
truth / mesh = visualization boundary from ADR-010.

Parallel maps allow independent terrain residency and doodad lifecycle policies
later (co-resident vs always-resident catalogs).

# Consequences

Benefits:

- Clear authoritative store for Phase 4 queries and Phase 7 persistence
- No ECS or renderer coupling in the foundation
- Deterministic per-chunk iteration (records sorted by `DoodadId`)

Costs:

- Two chunk-keyed maps on `WorldData` instead of one unified bucket
- Cross-chunk doodad queries require scanning or a future spatial index (Phase 4)

# Alternatives Considered

## Doodads inline in `ChunkData`

Rejected: violates ADR-008; couples terrain load/evict to doodad lifecycle.

## Separate global doodad resource outside `WorldData`

Rejected: splits authoritative world state; complicates persistence and queries.

## Coordinate-derived doodad identity

Rejected: doodads move (Phase 5 authoring); stable IDs required for overrides.

# Notes

- Cross-references: ADR-001, ADR-002, ADR-005, ADR-008, ADR-010, ARCHITECTURE
  Doodad Layer, ROADMAP Phase 3–4–7.
- [`ChunkDoodadStore`] maintains records sorted by [`DoodadId`] after insert.

[`WorldData`]: ../src/world/data.rs
[`ChunkData`]: ../src/world/chunk.rs
[`DoodadRecord`]: ../src/world/doodad/record.rs
[`DoodadId`]: ../src/world/doodad/id.rs
[`DoodadSource`]: ../src/world/doodad/source.rs
[`DoodadExclusionZone`]: ../src/world/doodad/exclusion/zone.rs
[`DoodadMetadata`]: ../src/world/doodad/metadata.rs
[`ChunkDoodadStore`]: ../src/world/doodad/store.rs
[`WorldPosition`]: ../src/world/coordinates.rs
[`ChunkId`]: ../src/world/chunk.rs
