# ADR-012: Chunk Loading, Streaming, and Lifecycle

# Status

Accepted

# Context

ADR-011 defines the pre-chunked asset format and a world manifest. This ADR
decides how those assets are loaded into the authoritative `WorldData` at runtime,
and how chunk residency is managed over time.

Forces:

- ADR-009 deferred a Bevy `AssetLoader` to "where streaming is in scope." Streaming
  is Phase 2B, not Phase 2A. BEVY_REFERENCE notes an `AssetLoader` requires
  `TypePath` (0.18) and pulls in async/handle machinery.
- ADR-010: the Terrain Runtime Layer owns the loading *process*; the World Data
  Layer owns the *data*. Phase 2 is built in vertical slices, and Phase 2A is the
  smallest complete correct path.
- AGENTS.md Groundwork Rule: do not build infrastructure ahead of a consumer. The
  consumer of async/on-demand loading is streaming (2B); Phase 2A's consumer is a
  one-time eager load.
- Phase 1 `WorldData` tracks extent as the bounds of *inserted* chunks. That is
  correct as long as nothing unloads. Only streaming (which unloads) needs a
  different model.
- ARCHITECTURE Scalability Rule: localized, event-driven operations over global
  scans.

# Decision

## Loading mechanism is split from payload decode

The **decode/parsing** of the manifest and chunk payloads (ADR-011) is implemented
as **delivery-agnostic functions**: they take bytes (or a path) and produce a
manifest value / `ChunkData`, with no dependency on how the bytes were obtained.
Both the Phase 2A synchronous loader and the Phase 2B `AssetLoader` reuse these
same functions. This is the reusable core; the *delivery mechanism* is what
changes between slices.

## Phase 2A: synchronous file loading (no AssetLoader)

Phase 2A loads the world synchronously at startup, with no Bevy `AssetLoader`,
`TypePath`, `AssetServer`, handles, or async:

- Read `assets/worlds/main/manifest.ron`, then read each chunk file under
  `assets/worlds/main/chunks/` named by the manifest.
- Decode via the shared functions, `insert` each `ChunkData` into `WorldData`, and
  spawn the derived render entity (ADR-010, ADR-013).
- No camera/view-distance-driven load or unload, and no LOD switching.

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
- `height_at`/`is_chunk_loaded` operate on inserted chunks; queries for chunks not
  loaded return "not loaded" (ADR-005). This is acceptable through Phase 2.

## Determinism

Loading order and (later) residency are runtime concerns. The **asset data** is
deterministic (ADR-011); persistence/multiplayer guarantees rely on data
determinism, not on which chunks are resident.

## Phase 2B: AssetLoader, streaming, and the residency model (deferred)

The following are designed now and implemented in Phase 2B, reusing the
delivery-agnostic decode functions:

- **Bevy `AssetLoader`s** for the manifest and chunk assets (each deriving
  `TypePath`), driven by `AssetServer`, replacing the 2A synchronous reader as the
  delivery mechanism over the same files. The loaders remain read-only and never
  route heights through an image/render path (ADR-003, ADR-011).
- **Authored extent vs resident set.** `WorldData` gains a distinction between the
  finite authored bounds (from the manifest, fixed for the session, what `extent()`
  reports) and the resident set (chunks currently in memory), plus
  `WorldData::remove(chunk)`. This is required only because streaming unloads
  chunks, so it lands with streaming, not before.
- **Residency policy.** Desired chunks are selected from camera position and a view
  distance, diffed against the resident set, and loaded/unloaded asynchronously.
  Load/unload is event-driven and chunk-local; it must never scan the whole world.
  If a "terrain loaded/unloaded" signal is added, it uses the `Message` trait and
  `MessageWriter` (BEVY_REFERENCE), not `Event`.
- Region-level prefetch (ADR-011) may inform load batching if/when region
  containers exist.

None of these are implemented in Phase 2A.

# Rationale

Phase 2A's only loading consumer is a one-time eager load, which a synchronous
reader satisfies completely. The genuinely reusable asset is the **decode logic**,
not the delivery mechanism, so isolating decode lets Phase 2B introduce the
`AssetLoader` over the same files and same decode without a rewrite. Keeping Phase 1
`WorldData` semantics in 2A avoids an authoritative-store refactor that has no 2A
consumer; the authored-extent/resident-set split earns its keep only once streaming
unloads chunks.

# Consequences

Benefits:

- Phase 2A is the smallest complete correct path: no async, handles, `TypePath`, or
  `WorldData` refactor.
- The decode/delivery split means 2B extends rather than replaces the meaningful
  code.
- Clean data contract: process in the runtime layer, data in `WorldData`.

Costs:

- The 2A synchronous reader is replaced as the *delivery* mechanism in 2B (cheap;
  the decode functions are retained).
- Eager loading is only viable for the small Phase 2A world; streaming is required
  before large worlds (Phase 2B).

# Alternatives Considered

## AssetLoader in Phase 2A

Rejected: its real consumer is on-demand/unloadable streaming (Phase 2B). Mandating
it in 2A builds 2B infrastructure ahead of a consumer (AGENTS.md Groundwork Rule)
and adds async/handle/`TypePath` machinery the eager load does not need.

## WorldData remove / authored-extent split in Phase 2A

Rejected: nothing unloads in 2A, so Phase 1 semantics (inserted == loaded; extent
from inserted chunks) are correct. The split is introduced with streaming (2B),
which is the feature that needs it.

## Distance-based streaming in Phase 2A

Rejected: residency is breadth, not the core vertical path, and would obscure
whether the load → data → render contract is correct.

# Notes

The decode functions are the contract between slices: keep them free of
`AssetServer`/IO-source assumptions so the 2A synchronous reader and the 2B
`AssetLoader` can both call them. Streaming tuning (view distance, hysteresis,
per-frame budget) is a Phase 2B concern, intentionally unspecified here beyond
"chunk-local and event-driven."
