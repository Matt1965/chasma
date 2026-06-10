# ADR-009: Terrain Import Mechanism

# Status

Accepted

# Context

ADR-003 defines the external terrain pipeline (Gaea → EXR heightfield → masks →
runtime import → chunk data) and that the runtime imports externally-authored
data. ADR-008 defines the runtime terrain representation (per-chunk heightfield
tiles, metadata, masks). This ADR decides *how* import happens in Phase 1.

Constraints and forces:

- ROADMAP Phase 1 success criteria: terrain data loads successfully; world data
  can be queried; rendering is not required.
- ARCHITECTURE requires deterministic procedural/import behavior for future
  multiplayer and persistence.
- Phase 1 explicitly excludes streaming, hot-reload, LOD, and mesh generation
  (those are Phase 2).
- The heightfield is authoritative data, not a render texture (ADR-003).
- BEVY_REFERENCE.md notes that a Bevy `AssetLoader` requires `TypePath` in 0.18,
  and that buffered signals use the `Message` trait (not `Event`) since 0.17.

# Decision

## Synchronous, deterministic importer (not an AssetLoader yet)

Phase 1 import is a **plain, synchronous, deterministic importer module**, not a
Bevy `AssetLoader`.

- Import decodes the source, partitions it into per-chunk tiles (ADR-008), builds
  `ChunkData`, and populates the `WorldData` resource.
- Import is deterministic: identical inputs produce identical `WorldData`
  (ARCHITECTURE multiplayer/persistence requirement).
- A Bevy `AssetLoader` (with the `TypePath` requirement and async/streamed
  loading) is deferred to Phase 2, where streaming is in scope.

## EXR decoding to raw f32

- The source heightfield is decoded with a dedicated OpenEXR decoder (the `exr`
  crate) into a plain `Vec<f32>`.
- The heightfield is **not** loaded as a Bevy `Image`/texture. Loading it through
  the rendering image path would conflate authoritative data with a derived
  render resource, violating ADR-003.
- Adding the `exr` crate is the one new non-Bevy dependency introduced in
  Phase 1. Bevy remains 0.18.

## Source description and invocation

- A `TerrainSource` descriptor owns the import inputs: source heightfield path,
  optional mask paths, and any format/scale parameters not already in
  `WorldConfig`.
- Import runs once at startup via a system in the World Data Layer, **only when a
  `TerrainSource` is configured**. With no source configured, import is a no-op
  and `WorldData` is empty, so the runnable shell still starts (ADR-007).
- World extent (finite bounds, ADR-006) is discovered during import and stored on
  `WorldData`.

> **Superseded in part:** the assumption that the *runtime* imports a monolithic
> source heightfield at startup is replaced by the Phase 1B addendum below.
> Monolithic source heightfields are an *offline / preprocessing* input;
> the runtime loads pre-chunked terrain assets.

## Construction from raw samples

- `Heightfield` exposes construction from raw samples (independent of any file),
  so import, synthetic data, and tests can build terrain without an EXR on disk.
- This keeps the Phase 1 success criteria testable with small in-memory fixtures
  and does not require shipping binary assets.

## No completion event in Phase 1

- Phase 1 does not emit a "terrain loaded" signal; consumers (Phase 2) do not
  exist yet. If such a signal is added later it must use the `Message` trait and
  `MessageWriter` (BEVY_REFERENCE.md), not `Event`.

# Rationale

A synchronous importer is the smallest mechanism that satisfies Phase 1 and keeps
import deterministic. Deferring the `AssetLoader` avoids building Phase 2
streaming infrastructure prematurely (AGENTS.md Groundwork Rule). Decoding EXR to
raw `f32` preserves the authoritative-data boundary from ADR-003. Construction
from raw samples makes loading testable without rendering or binary fixtures.

# Consequences

Benefits:

- Minimal, deterministic import that meets Phase 1 criteria
- Authoritative data kept out of the render/image path
- Testable without shipping EXR assets
- Clean upgrade path to a Phase 2 streaming `AssetLoader`

Costs:

- Synchronous import blocks during load (acceptable: no streaming in Phase 1)
- A second, file-backed import path (`AssetLoader`) will be added in Phase 2 and
  must reuse this importer's partitioning logic
- New dependency (`exr`)

# Alternatives Considered

## Bevy AssetLoader in Phase 1

Rejected for now: it pulls in async/streaming concerns that belong to Phase 2,
and adds the `TypePath` requirement (BEVY_REFERENCE.md) without a Phase 1 need.
The synchronous importer's partitioning logic is designed to be reused by a
Phase 2 loader.

## Loading the heightfield as a Bevy Image

Rejected: it routes authoritative terrain through a rendering resource and risks
precision/format conversion, contrary to ADR-003.

## Lazy / on-demand import in Phase 1

Rejected: on-demand loading is streaming, which is Phase 2. Phase 1 loads
configured terrain once at startup.

# Notes

This ADR governs the Phase 1 import mechanism only. Streaming, hot-reload, and
the `AssetLoader` are expected in Phase 2 and should extend, not replace, the
deterministic partitioning defined here and in ADR-008.

---

# Addendum: EXR Row Orientation and Mask Import (Phase 1B)

Status: Accepted

The Phase 1B partitioner (`import_world`) operates on an already-decoded source
grid (`SourceHeightfield`). This addendum fixes the conventions the EXR decoder
must follow so the decode -> partition seam is unambiguous before the decoder is
written.

## Source grid orientation

The partitioner treats source row index as advancing along `+Z` and column index
along `+X` (ADR-008 addendum). To make decoding unambiguous:

- Source row `0` is the minimum-`Z` edge of the world; row index increases toward
  `+Z`. Source column `0` is the minimum-`X` edge.
- The EXR decoder maps the image's **first scanline to source row `0`**. If an
  authoring tool exports with the opposite vertical convention, the decoder is the
  single place that applies a vertical flip; the data model always assumes
  `row 0 = minimum Z`.

A configurable flip is intentionally not introduced now. It can be added as a
decode option if and when a real authoring pipeline requires it (AGENTS.md
Groundwork Rule); it does not affect the partitioner or stored data.

## Sample validity

The decoder produces a `SourceHeightfield` whose samples must all be finite.
Non-finite values (NaN/infinite) are rejected at construction, because heights
are authoritative data (ADR-003) and would corrupt derived metadata and bilinear
sampling. EXR's float channels can carry such values, so this is validated rather
than assumed.

## Mask import behavior

- Masks are **optional**. If a `TerrainSource` provides mask layers, each is
  decoded to raw `f32` and partitioned per the ADR-008 mask-partitioning model,
  then attached to every `ChunkData`. If no masks are provided, chunks have empty
  mask lists (the current partitioner's behavior).
- Mask decoding follows the same authoritative-data rule as the heightfield: it
  decodes to plain `f32` data and is never routed through a render/image resource.
- Mask resolution (`M`) need not match the heightfield resolution (`N`); both must
  partition into the same chunk grid (ADR-008 addendum), otherwise import fails.

---

# Addendum: Runtime Pre-chunked Assets vs. Offline Monolithic Import (Phase 1B)

Status: Accepted

Supersedes the "Source description and invocation" section's assumption that the
runtime imports a monolithic source heightfield at startup.

## Decision

- **Runtime terrain loading uses pre-chunked terrain assets.** Terrain is loaded
  per chunk, already partitioned to the chunk grid (ADR-008). The runtime never
  loads a monolithic world heightfield. This is consistent with ADR-008's
  rejection of a single in-memory world heightfield and with the Scalability Rule
  (no whole-world scans or whole-world allocations at runtime).
- **Monolithic source heightfields are supported only as offline import /
  preprocessing input.** A single EXR covering a region or the whole world is
  authored content fed to the offline tools, not the runtime.

## What this means for the importer and decoder

- The deterministic partitioner (`import_world`: `SourceHeightfield` ->
  `WorldData`) and the EXR decoder (`decode_exr_heightfield`: EXR file ->
  `SourceHeightfield`) are **offline / preprocessing tools**. They convert
  authored monolithic data into per-chunk data.
- Neither is wired as a runtime startup system. The library exposes them as
  functions; an offline import/preprocessing step (tool, example, or build step)
  drives them. The runtime's terrain load path consumes the resulting per-chunk
  assets.
- Determinism (see Decision, top of this ADR) still applies to the offline
  partitioner, so generated chunk assets are reproducible — preserving the
  multiplayer/persistence guarantees.

## Deferred

- The pre-chunked runtime asset **format** and its **loader** are a separate
  concern. They belong with Phase 2 chunk streaming and will get their own ADR;
  they are not built now (AGENTS.md Groundwork Rule). This addendum only fixes
  the boundary: runtime consumes pre-chunked data; monolithic sources are offline
  input.
