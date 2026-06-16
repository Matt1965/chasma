# ADR-011: Pre-chunked Terrain Asset Format and Storage Layout

# Status

Accepted

# Context

ADR-009 established that runtime terrain loading consumes **pre-chunked** terrain
assets, while monolithic EXR import and partitioning are offline/preprocessing
only (behind the `terrain-import` feature). It deferred the pre-chunked asset
format and its loader to a Phase 2 ADR. This is that ADR.

Forces:

- ADR-008: authoritative per-chunk data is an `N+1` shared-edge heightfield tile,
  row-major `f32`, plus minimal `TerrainMetadata`. Masks are deferred (ADR-009
  Phase 1 Cleanup).
- ARCHITECTURE Scalability Rule: avoid whole-world scans and whole-world in-memory
  data; prefer chunk-local operations.
- A finite world (ADR-006) can reach thousands of 256 m chunks.
- Determinism is required for persistence/multiplayer (ARCHITECTURE).

# Decision

## Per-chunk asset payload

A pre-chunked terrain asset describes exactly one 256 m chunk and carries the
authoritative subset of `ChunkData`:

- format version
- `ChunkId` (its coordinate identity, ADR-001)
- heightfield tile: `samples_per_edge`, `spacing`, and the row-major `f32`
  `(N+1) x (N+1)` samples (ADR-008)
- `TerrainMetadata` (`height_min`/`height_max`)
- a reserved, versioned slot for mask layers (omitted now; ADR-009 defers mask
  import until a consumer exists)

The payload contains **no** normals, mesh, or LOD data — those are derived at
runtime (ADR-010, ADR-013). Encoding must be lossless for `f32` heights (ADR-003)
and byte-for-byte deterministic for identical inputs.

## World manifest

A separate, small **manifest** file describes the world as a whole. Per the file
layout it is `assets/worlds/main/manifest.ron`:

- a `WorldConfig` snapshot (chunk size, spacing, units-per-meter), used to validate
  loaded chunks against the runtime config
- the list of chunks in the world (their `ChunkId`s) and, for each, the chunk file
  path
- optional finite-extent info (authored bounds, ADR-006) carried as data

The manifest is loaded first to know which chunk files to read and where they are.
It **carries** extent/config information as data; it does **not** require any
`WorldData` refactor in Phase 2A — Phase 1 `WorldData` semantics are kept (inserted
chunks are the loaded world; extent is derived from inserted chunks). See ADR-012.

## Phase 2A: one chunk = one self-contained file

For Phase 2A, each chunk is stored as **one self-contained file** under
`assets/worlds/main/chunks/`, and the manifest maps each `ChunkId` to its chunk
file path. This is the simplest correct storage that proves the load path. Because
each chunk file contains a complete chunk payload (above), it stands alone with no
external index.

## Region containers: a design constraint only

Region/group containers are **not part of Phase 2A** and introduce **no** code or
types in 2A — no `RegionId`, no resolver traits, no offset tables, no container
indexes, and no region abstractions. They are kept open purely as a *design
constraint*:

- The per-chunk payload (above) is **self-contained**, so a chunk's bytes are the
  same whether stored as a standalone file or, later, packed into a region
  container. The chunk decoder therefore does not change when regions arrive.
- Region containers can be added later as a new storage option behind the manifest
  (which already names where each chunk lives), **without changing the chunk
  payload format**. That is the only compatibility guarantee being made now.

Region containers will be considered only when file count, filesystem locality, or
prefetch needs justify them (ADR-004-style "evidence before complexity"; AGENTS.md
Groundwork Rule). Until then, the runtime addresses individual 256 m chunks and the
manifest maps each `ChunkId` to a file path.

## Producer vs consumer

- The **writer** that produces these assets (and, later, region containers) is
  offline/preprocessing and lives behind the `terrain-import` feature. It consumes
  `import_world`'s `WorldData` (ADR-009).
- The **reader/loader** is runtime and always compiled (ADR-012).

# Rationale

A **self-contained** per-chunk payload plus a manifest that maps each `ChunkId` to
a file path is the smallest storage that proves the load path, and it is exactly
what makes "one file per chunk now, regions later" a non-breaking evolution: since
a chunk's bytes do not depend on neighbors or an index, region containers can be
added later without changing the chunk payload format. Keeping the payload to the
authoritative subset preserves the truth/derived split (ADR-010) and keeps assets
small and deterministic. No region indirection or resolver is built now (AGENTS.md
Groundwork Rule).

# Consequences

Benefits:

- Smallest correct storage for Phase 2A, with a non-breaking path to regions that
  costs no code today.
- Deterministic, lossless, render-independent assets.
- Manifest supplies the chunk list (and optional extent/config) the loader needs,
  without forcing a `WorldData` refactor in 2A.

Costs:

- One-file-per-chunk does not scale to very large worlds; acceptable for Phase 2A,
  and the migration to regions is already designed.
- A manifest must be produced and kept consistent with the chunk files.

# Alternatives Considered

## One file per chunk as the permanent format

Rejected as a permanent decision: thousands of tiny files hurt filesystem
locality, handle counts, and atomic offline writes. Accepted only as the Phase 2A
storage, with regions designed in.

## Region containers in Phase 2A

Rejected for 2A: it adds storage complexity before the vertical path is proven and
before file-count pressure exists.

## Per-chunk EXR or Bevy `Image` assets

Rejected: routes authoritative heights through a rendering/image path, contrary to
ADR-003/ADR-009.

# Notes

The chunk payload is versioned so the reserved mask slot and future fields are
additive. Any change to the heightfield encoding must bump the version and update
the offline writer and runtime reader together.

---

# Addendum: Optional Albedo Sidecar Reference (Phase 2D)

Status: Accepted

## Decision

- [`ManifestChunk`] gains optional **`albedo_path: Option<String>`** (relative to
  the manifest directory). Omitted or `None` means no albedo sidecar; existing
  manifests without the field remain valid (`#[serde(default)]`).
- The height **`ChunkFile`** payload is **unchanged** — albedo is never embedded
  in the height RON.
- Supported sidecar formats (v1):
  - **`*.albedo.exr`** — preferred; square RGB float grid, row-major, matching
    height `samples_per_edge`.
  - **`*.albedo.ron`** — compact versioned DTO (`AlbedoFile`) with
    `samples_per_edge` and flat RGB triples.
- Sidecar dimensions must **exactly match** the decoded height chunk grid; runtime
  decode must not silently resample.
- If `albedo_path` is set but the file is missing at load time: log a warning and
  continue without albedo (height load succeeds).

## Compatibility

- Region-container evolution unchanged: sidecar path is manifest-level metadata,
  parallel to the height chunk path.
- Height-only worlds load unchanged.
