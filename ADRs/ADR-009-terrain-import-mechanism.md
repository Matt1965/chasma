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

- A source *descriptor* (naming the heightfield path, optional mask paths, and
  any format/scale parameters not already in `WorldConfig`) owns the import
  inputs.
- Import runs once at startup via a system in the World Data Layer, **only when a
  source is configured**. With no source configured, import is a no-op and
  `WorldData` is empty, so the runnable shell still starts (ADR-007).
- World extent (finite bounds, ADR-006) is discovered during import and stored on
  `WorldData`.

> **Superseded in part:** the assumption that the *runtime* imports a monolithic
> source heightfield at startup is replaced by the Phase 1B addenda below.
> Monolithic source heightfields are an *offline / preprocessing* input;
> the runtime loads pre-chunked terrain assets. The source-descriptor type
> itself is deferred (see "Phase 1 Cleanup" addendum) until an offline driver
> consumes it; the implemented decoder takes a path directly.

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

When mask import is implemented, it follows these rules:

- Masks are **optional**. When mask layers are provided, each is decoded to raw
  `f32` and partitioned per the ADR-008 mask-partitioning model, then attached to
  every `ChunkData`. If no masks are provided, chunks have empty mask lists.
- Mask decoding follows the same authoritative-data rule as the heightfield: it
  decodes to plain `f32` data and is never routed through a render/image resource.
- Mask resolution (`M`) need not match the heightfield resolution (`N`); both must
  partition into the same chunk grid (ADR-008 addendum), otherwise import fails.

> See the "Phase 1 Cleanup" addendum below: mask decoding and partitioning are
> **deferred** and not implemented in Phase 1. The rules above describe the
> intended behavior for when a consumer exists.

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

## Build gating

Because the EXR decode / offline partitioning path is not part of the runtime, it
is gated behind an opt-in Cargo feature named `terrain-import`:

- The `exr` dependency is optional and pulled in only by `terrain-import`
  (`terrain-import = ["dep:exr"]`). Default builds do not compile `exr`.
- The offline modules `terrain::import` (`SourceHeightfield`, `import_world`,
  `ImportError`) and `terrain::decode` (`decode_exr_heightfield`, `DecodeError`)
  and their re-exports are compiled only with `terrain-import`.
- The authoritative runtime data types — `Heightfield`, `TerrainMetadata`,
  `TerrainMask`, `ChunkData`, and `WorldData` — remain available without the
  feature. The core world data layer builds and is testable with default
  features.

## Deferred

- The pre-chunked runtime asset **format** and its **loader** are a separate
  concern. They belong with Phase 2 chunk streaming and will get their own ADR;
  they are not built now (AGENTS.md Groundwork Rule). This addendum only fixes
  the boundary: runtime consumes pre-chunked data; monolithic sources are offline
  input.

---

# Addendum: Phase 1 Cleanup — Deferred Mask Import and Source Descriptors

Status: Accepted

The Phase 1 completion review found two pieces of this ADR that described
behavior or types with no current consumer. Per the AGENTS.md Groundwork Rule
("build seams, not fake future systems"), they are deferred rather than built.

## Mask import is deferred

- `TerrainMask` remains the Phase 1 terrain mask **data structure** (ADR-008): a
  plain per-sample `f32` layer, constructible from raw samples.
- **Mask decoding and mask partitioning are not implemented in Phase 1.** They
  are deferred until a real consumer exists — expected at Phase 2 (terrain
  material blending) or Phase 3 (doodad generation).
- Consequently `import_world` produces chunks with empty mask lists. The mask
  decode/partition rules above (and the ADR-008 mask-partitioning addendum) define
  the *intended* model for when that consumer arrives; they are a documented seam,
  not implemented behavior.

## Source descriptors are deferred

- The `TerrainSource` / `MaskSource` descriptor types have been **removed** for
  now. Nothing consumed them: the decoder (`decode_exr_heightfield`) takes a path
  directly, and the partitioner (`import_world`) takes an already-decoded
  `SourceHeightfield`.
- A source-descriptor type (naming heightfield + mask paths and format/scale
  parameters) will be reintroduced when an **offline import driver** that wires
  decode → partition → (eventually) mask import actually exists. That driver is
  not built in this pass.
- This keeps the default and `terrain-import` builds free of an unused public
  type while preserving the design intent here in the ADR.

## Unchanged

- The offline boundary, the `terrain-import` feature gating, and the implemented
  decode/partition path (`decode_exr_heightfield`, `SourceHeightfield`,
  `import_world`) are unchanged.

---

# Addendum: Optional Gaea Albedo Sidecar Export (Phase 2D)

Status: Accepted

## Decision

- Gaea may export optional albedo tiles alongside height tiles:
  - **Primary:** `Albedo_y{z}_x{x}.exr` (float RGB, linear)
  - **Fallback:** `Albedo_y{z}_x{x}.png` (8-bit sRGB, converted at import)
- Height tiles remain **`Export_y{z}_x{x}.exr`** and are **required** for chunk
  import. Albedo tiles are **optional**; missing albedo for a chunk must not fail
  height import.
- Albedo decode follows the authoritative-data rule: decode to plain `f32` RGB
  samples in a [`ChunkAlbedoGrid`]; never route through Bevy `Image`/texture
  assets (ADR-003).
- Albedo tiles use the **same chunk coordinate naming and grid alignment** as
  height (shared-edge or non-overlap stitching rules mirror height import).
- Offline Gaea import writes runtime sidecars (`chunks/<x>_<z>.albedo.exr` or
  `.albedo.ron`) and optional manifest `albedo_path` entries (ADR-011). Height
  RON payloads are unchanged.

## Deferred

- Mesh vertex-color baking from albedo (ADR-013 addendum).
- Near-field high-res texture/material blending (ADR-004).
- Mask import (unchanged deferral above).
