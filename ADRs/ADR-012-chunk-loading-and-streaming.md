# ADR-012: Chunk Loading, Streaming, and Lifecycle

# Status

Accepted (revised for Phase 2B — synchronous streaming slice)

# Context

ADR-011 defines the pre-chunked asset format and a world manifest. This ADR
decides how those assets are loaded into the authoritative `WorldData` at runtime,
and how chunk residency is managed over time.

Forces:

- ADR-009 deferred a Bevy `AssetLoader` until streaming is in scope. Streaming
  is Phase 2B, not Phase 2A. BEVY_REFERENCE notes an `AssetLoader` requires
  `TypePath` (0.18) and pulls in async/handle machinery.
- ADR-010: the Terrain Runtime Layer owns the loading *process*; the World Data
  Layer owns the *data*. Phase 2 is built in vertical slices, and Phase 2A is the
  smallest complete correct path.
- AGENTS.md Groundwork Rule: do not build infrastructure ahead of a consumer.
  Phase 2A's consumer is a one-time eager load. Phase 2B's first consumer is
  synchronous on-demand residency; `AssetLoader` has no consumer until that path
  is proven.
- Phase 1 `WorldData` tracks extent as the bounds of *inserted* chunks. That is
  correct as long as nothing unloads. Only streaming (which unloads) needs a
  different model.
- ADR-014: the camera layer must not import terrain or world; terrain must not
  import camera. View position for streaming crosses layers through a generic
  presentation resource and an app-layer bridge.
- ARCHITECTURE Scalability Rule: localized, chunk-local operations over global
  scans.

# Decision

## Loading mechanism is split from payload decode

The **decode/parsing** of the manifest and chunk payloads (ADR-011) is implemented
as **delivery-agnostic functions**: they take bytes (or a path) and produce a
manifest value / `ChunkData`, with no dependency on how the bytes were obtained.
Phase 2A eager loading, Phase 2B on-demand loading, and any future `AssetLoader`
delivery all reuse these same functions. This is the reusable core; the
*delivery mechanism* may change in later slices.

## Phase 2A: synchronous file loading (no AssetLoader)

Phase 2A loads the world synchronously at startup, with no Bevy `AssetLoader`,
`TypePath`, `AssetServer`, handles, or async:

- Read `assets/worlds/main/manifest.ron`, then read each chunk file under
  `assets/worlds/main/chunks/` named by the manifest.
- Decode via the shared functions, `insert` each `ChunkData` into `WorldData`, and
  spawn the derived render entity (ADR-010, ADR-013).
- No view-distance-driven load or unload, and no LOD switching.

This proves the full vertical path — asset file → decode → `WorldData` insert →
pure mesh generation → derived render entity — with the least machinery.

## Phase 2A: Phase 1 `WorldData` semantics are unchanged

Phase 2A keeps the Phase 1 World Data Layer as-is:

- Inserted chunks **are** the loaded world; `extent()` is derived from inserted
  chunks (Phase 1 behavior). Because nothing unloads in 2A, this is correct.
- No `WorldData::remove`, and no authored-extent vs resident-set distinction in
  2A.
- The manifest may **carry** extent and `WorldConfig` info as data (for a
  consistency check against the runtime `WorldConfig`), but it does **not** require
  any `WorldData` refactor in 2A.

## Data contract

- On loading a chunk, the runtime layer validates it against the runtime
  `WorldConfig` and **inserts `ChunkData` into `WorldData`**, then spawns the
  derived render entity.
- `height_at`/`is_chunk_loaded` operate on resident chunks; queries for chunks not
  resident return "not loaded" (ADR-005). This is acceptable through Phase 2.

## Determinism

Loading order and residency are runtime concerns. The **asset data** is
deterministic (ADR-011); persistence/multiplayer guarantees rely on data
determinism, not on which chunks are resident.

## Phase 2B: synchronous streaming and the residency model

Phase 2B implements chunk residency lifecycle **fully synchronously**. It does
**not** introduce `AssetLoader`, `AssetServer` terrain loading, async I/O, or
load/unload `Message` types. Those are deferred until synchronous streaming is
proven and a real consumer exists (AGENTS.md Groundwork Rule).

### Manifest catalog (terrain layer, separate from residents)

At startup the terrain runtime loads the manifest once into a **catalog** resource
(metadata only — no height samples):

- validated `ManifestConfig` snapshot against runtime `WorldConfig`
- mapping from each authored `ChunkCoord` to its chunk file path (relative to the
  manifest directory)
- **authored extent** derived from the manifest chunk list (min/max coordinates)

The catalog is immutable for the session. It answers "which chunks exist in the
authored world" and where their files live. It does not populate `WorldData`.

### Authored extent vs resident set (world layer)

`WorldData` gains a distinction required because streaming unloads chunks:

- **`authored_extent`**: set once from the catalog at startup; immutable for the
  session; `extent()` reports this (ADR-006 finite world bounds).
- **Resident set**: the existing `chunks` map — chunks whose `ChunkData` is
  currently in memory.
- **`WorldData::remove(chunk)`**: evicts resident `ChunkData` only; does not
  change `authored_extent` or delete disk assets.
- **`insert`**: no longer expands authored extent; only adds/replaces resident
  data.

`is_chunk_loaded` means resident. Coords in the catalog but not resident are
authored-but-unloaded. Coords outside the catalog are outside the authored world.

### Synchronous on-demand delivery (Phase 2B)

Chunk files are read synchronously on demand, one chunk at a time:

- `fs::read_to_string` (or equivalent) for the catalog path
- `decode_chunk` → validate → `WorldData::insert` → spawn derived mesh entity

The Phase 2A `load_world_from_manifest` eager path is retained for tests and
tooling but is not used by the dev preview after Phase 2B.

### Residency policy (terrain layer)

Each frame, synchronously:

1. Read the active view center from a generic presentation resource (below).
2. Compute the **desired resident set**: authored chunks within the load radius
   of the focus chunk, using O(r²) iteration around the focus coordinate (never
   scan the full manifest chunk list).
3. Diff desired vs resident → `to_load`, `to_unload`.
4. Apply per-frame load and unload budgets (cap how many chunks load/unload per
   frame).
5. **Hysteresis**: unload radius is smaller than load radius so border chunks do
   not thrash.

Load/unload is chunk-local and driven by view movement; it must never load the
entire world into memory.

### Unload ordering

For each chunk to unload, in a fixed system order within the same frame:

1. Despawn derived `TerrainChunkMesh` render entities for that `ChunkId`.
2. `WorldData::remove(chunk)`.

Meshes are disposable derived state (ADR-010). Despawn does not read mesh data
back into `WorldData`.

### View focus seam (ADR-014)

Streaming needs the local view center. Layer boundaries forbid terrain importing
camera and camera importing terrain/world.

- A generic **presentation resource** (e.g. `PrimaryViewFocus` or `ViewFocus`)
  holds the world-space center of the active local view. It is **not**
  authoritative world state.
- The **camera layer** does not write this resource directly from terrain code.
- The **app composition layer** (`src/app/`) runs a bridge system that reads
  `RtsCameraState` (camera layer) and writes the presentation resource.
- **Terrain streaming** reads the presentation resource to compute desired chunks.
- Terrain must not import `crate::camera`. Camera must not import
  `crate::terrain` or `crate::world`.

### No streaming messages in Phase 2B

Phase 2B does not add `TerrainChunkLoaded`, `TerrainChunkUnloaded`, or any other
Bevy `Message` types for streaming. Signals are deferred until a real consumer
exists (AGENTS.md Groundwork Rule). If added later, they use the `Message` trait
(BEVY_REFERENCE), not `Event`.

### Deferred beyond Phase 2B (this slice)

The following remain out of Phase 2B scope:

- Bevy `AssetLoader` / `AssetServer` / async terrain delivery (future slice after
  synchronous streaming is proven)
- Load/unload `Message` types
- Mesh-resolution LOD, skirts, far terrain (Phase 2C, ADR-013)
- Region containers and region prefetch (ADR-011)
- Masks, doodads, occupancy, gameplay, simulation, pathfinding, multiplayer
- Custom shaders and renderer-specific complexity (ADR-004)

Region-level prefetch (ADR-011) may inform load batching if/when region containers
exist.

## Phase 2B+ (future): AssetLoader delivery swap

After synchronous streaming is proven, a future slice may replace synchronous
file reads with Bevy `AssetLoader`s driven by `AssetServer`, reusing the same
decode functions. Residency policy, catalog diff, `WorldData::remove`, mesh
despawn, and view-focus seam remain unchanged; only I/O delivery changes.

This slice is **not** part of Phase 2B.

# Rationale

Phase 2A's only loading consumer is a one-time eager load, which a synchronous
reader satisfies completely. Phase 2B's consumer is residency lifecycle — which
chunks are in memory and which render entities exist — not async I/O. Proving
lifecycle with synchronous on-demand reads is the smallest correct path before
introducing `AssetLoader`, handles, `TypePath`, and load-state polling.

The genuinely reusable asset is the **decode logic**, not the delivery mechanism.
Isolating decode lets a future `AssetLoader` swap in without rewriting residency
or `WorldData` semantics.

Keeping Phase 1 `WorldData` semantics in 2A avoided an authoritative-store
refactor with no 2A consumer. The authored-extent/resident-set split lands with
streaming because unloading is what makes it necessary.

A generic view-focus resource preserves ADR-014 layer boundaries without
terrain-specific presentation naming that would not generalize to future
consumers (doodads, far representation).

# Consequences

Benefits:

- Phase 2A remains the smallest complete correct path.
- Phase 2B proves streaming lifecycle without async/asset machinery.
- Decode/delivery split means a future `AssetLoader` extends rather than replaces
  meaningful code.
- Clean data contract: process in the terrain runtime layer, data in `WorldData`.
- Layer boundaries preserved via app-layer view-focus bridge.

Costs:

- Synchronous per-chunk file reads may hitch on large loads; acceptable for
  Phase 2B proof; per-frame budgets mitigate spikes.
- `load_world_from_manifest` remains as a test/tooling path alongside streaming.
- Eager loading is not viable for large worlds; streaming is required before
  scale (Phase 2B).

# Alternatives Considered

## AssetLoader in Phase 2B

Rejected for Phase 2B: synchronous residency lifecycle must be proven first.
`AssetLoader` adds async/handle/`TypePath` machinery whose consumer is efficient
delivery, not the residency model itself. Deferred to Phase 2B+ per AGENTS.md
Groundwork Rule.

## AssetLoader in Phase 2A

Rejected: no on-demand/unloadable streaming consumer in 2A.

## WorldData remove / authored-extent split in Phase 2A

Rejected: nothing unloads in 2A, so Phase 1 semantics are correct until streaming.

## Distance-based streaming in Phase 2A

Rejected: residency is breadth; would obscure whether the load → data → render
contract is correct.

## TerrainViewFocus (terrain-specific presentation resource)

Rejected: view center is a generic local presentation concern, not terrain-owned.
Use `PrimaryViewFocus` / `ViewFocus` in a presentation-appropriate module so
future layers can share it without terrain naming.

## Streaming load/unload Messages in Phase 2B

Rejected: no consumer exists yet (AGENTS.md Groundwork Rule).

# Notes

The decode functions are the contract between delivery mechanisms: keep them free
of `AssetServer`/IO-source assumptions so synchronous reads and a future
`AssetLoader` can both call them.

Streaming tuning (load radius, unload radius, per-frame budget) is a Phase 2B
implementation concern; defaults are chosen at implementation time.

Authored extent may be computed from the manifest chunk list at catalog init
without an ADR-011 format change. An explicit `authored_extent` field in the
manifest remains an optional future additive validation.

Cross-references: ADR-010 (terrain runtime boundaries), ADR-011 (asset format),
ADR-014 (camera layer; view-focus bridge).
