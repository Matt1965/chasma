# ADR-013: Terrain Mesh Generation and LOD

# Status

Accepted (Phase 2A/2B/2B.6 proven; Phase 2C mesh LOD designed)

# Context

The Terrain Runtime Layer (ADR-010) turns authoritative heightfield tiles into
derived, disposable meshes for rendering. ROADMAP Phase 2 lists both "terrain mesh
generation" and a "terrain LOD system" / "basic far terrain representation," with
success criteria "terrain is visible," "LOD works," and "high-altitude viewing is
functional."

ADR-004 forbids introducing renderer complexity (custom shaders, pipelines,
GPU-driven systems) without profiling evidence. These two forces must be
reconciled: LOD is in scope, but only as far as generated meshes + standard
materials allow.

Phase 2B.6 established async mesh generation: IO, decode, and mesh build run off
the main thread; apply only inserts `WorldData`, registers a prebuilt `Mesh`, and
spawns render entities (ADR-012). Phase 2C adds mesh-resolution LOD on top of
that pipeline without rewriting streaming policy.

# Decision

## Pure mesh builder

Mesh generation is a **pure function** of an authoritative `Heightfield` (plus a
LOD level): it takes the tile and returns a Bevy `Mesh`, with no access to the
ECS world. This makes it unit-testable on synthetic data and keeps it free of
rendering/world side effects (ADR-010).

- Positions come from the `(N+1) x (N+1)` grid; an `N x N` quad grid yields
  `2 * N * N` triangles.
- Per-vertex normals are computed from heightfield finite differences.
- Use Bevy 0.18 mesh APIs: the `try_*` mutation methods
  (`try_with_inserted_indices`, `try_compute_normals`, etc.) per BEVY_REFERENCE,
  and rely on automatic `Aabb`.
- The mesh is positioned by a `Transform` at the chunk's minimum-corner origin
  (`cx * size, 0, cz * size`; ADR-001). Heights are absolute world Y.

The builder consumes authoritative data and writes nothing back; meshes are
disposable and fully regenerable.

## Materials

A single shared `StandardMaterial` is used initially. No custom terrain material,
shader, or pipeline is introduced (ADR-004). Material blending from masks is a
later concern (masks are deferred, ADR-009).

## Phase 2A: single full-resolution LOD

Phase 2A generates exactly **one full-resolution mesh per chunk**:

- No LOD levels, no LOD selection, no skirts (with a single LOD there are no
  inter-LOD cracks to hide).
- This proves the pure data → mesh → render-entity path correctly and is the
  mesh side of the Phase 2A vertical slice.

## Phase 2C: mesh-resolution LOD

Phase 2C implements discrete mesh-resolution LOD using the existing pure builder
and async materialization pipeline (ADR-012 Phase 2B.6).

### LOD levels

Four discrete levels, produced by subsampling the authoritative heightfield at
power-of-two strides (stride must divide the 256 interior quads of a 257×257
tile):

| Level   | Stride | Samples/edge | ~Triangles |
|---------|--------|--------------|------------|
| Full    | 1      | 257          | ~131k      |
| Half    | 2      | 129          | ~33k       |
| Quarter | 4      | 65           | ~8k        |
| Eighth  | 8      | 33           | ~2k        |

This stays within ADR-004 (generated meshes, standard material). No screen-space
error metric, geomorphing, or continuous LOD in Phase 2C.

### LOD selection (ring-based)

LOD is chosen from **Chebyshev chunk-ring distance** between the chunk coordinate
and the stable view focus chunk (`PrimaryViewFocus` + `stable_focus_chunk`, same
seam as ADR-012 streaming):

```
distance 0 → Full
distance 1 → Half
distance 2 → Quarter
distance ≥ 3 → Eighth
```

Ring thresholds live in a terrain-runtime `TerrainLodSettings` resource,
independent of streaming load/unload radii. Selection is a pure function of
focus chunk, chunk coordinate, and settings.

### Authority split

- **`WorldData` / `ChunkData`**: always full-resolution 257×257 heightfield.
  LOD never writes back to authoritative data or chunk assets (ADR-011).
- **Terrain runtime**: LOD selection, lazy mesh cache, active `Mesh3d` handle,
  and async mesh generation only.

### Main-thread invariant

**LOD transitions must never block the main thread. All missing LOD meshes must
be generated asynchronously.**

The main thread may validate residency, read cache entries, swap existing mesh
handles, enqueue async work, and register completed meshes — but must never call
the mesh builder synchronously for LOD purposes.

### Lazy per-resident LOD mesh cache

Each **resident** terrain chunk maintains a terrain-runtime cache of generated
mesh handles keyed by `ChunkLod` (`Full`, `Half`, `Quarter`, `Eighth`).

Rules:

1. **Do not build all LOD meshes immediately.** Only the LOD needed at first
   materialization (or first transition to that level) is built.
2. **Build a LOD mesh only when that LOD is first needed** — on async
   materialization mesh-build start, or when a resident chunk's desired LOD
   changes to a level not yet cached.
3. **Once built, keep the mesh handle while the chunk remains resident.** Insert
   into the per-chunk cache; do not rebuild unless the chunk unloads and reloads.
4. **If desired LOD changes and the cached mesh exists**, swap the active
   `Mesh3d` handle on the main thread immediately (no async work).
5. **If desired LOD changes and the cached mesh does not exist**, enqueue async
   LOD mesh generation on `AsyncComputeTaskPool`; apply registers the mesh into
   the cache and swaps the active handle when complete.
6. **Never build LOD meshes synchronously on the main thread.**
7. **When the chunk unloads**, discard cached LOD mesh handles with render-entity
   / terrain-runtime cleanup. `Assets<Mesh>` handles drop when the cache is
   cleared; no LOD state persists in `WorldData`.
8. The cache is **terrain-runtime render state only** — not authoritative, not
   serialized, not queryable via the public query API (ADR-005 defers
   `chunk_lod`).

One **active** LOD per chunk is displayed at a time (`active_lod` on the render
marker or companion component). The cache may hold up to four handles over a
chunk's resident lifetime as the camera moves.

### Materialization integration

Initial chunk load (async pipeline):

```
IO → decode → async mesh build(desired_lod) → apply
```

- Desired LOD is computed from focus position when the mesh-build task **starts**.
- Apply inserts full `ChunkData`, registers the prebuilt mesh, seeds the LOD
  cache with that level, sets `active_lod`, and spawns `TerrainChunkMesh`.
- If focus moves before apply and desired LOD differs from built LOD: **apply the
  completed mesh anyway**, cache it under its built level, then let the resident
  LOD refresh path swap or enqueue the correct level (do not discard completed
  async mesh work for LOD mismatch alone).

### Resident LOD refresh

After apply, each frame (or on focus change):

1. Compute `desired_lod` for each resident render chunk.
2. If `desired_lod == active_lod`: no work.
3. If `desired_lod` is cached: swap `Mesh3d` + update `active_lod` (main thread).
4. If `desired_lod` is not cached and no in-flight build for that `(chunk, lod)`:
   enqueue async mesh build from resident `WorldData` heightfield.
5. Poll in-flight LOD builds; on completion, insert into cache, swap if still
   desired, discard if chunk unloaded or desired LOD changed again.

In-flight LOD builds respect the same generation/residency cancellation rules as
materialization (chunk unload revokes work; stale completions are discarded).

Streaming policy (load/unload radii, hysteresis, budgets) is **unchanged**
(ADR-012).

### Cracks and skirts

Ring-based selection minimizes LOD transition perimeter: most neighbors within a
ring share the same LOD. Adjacent chunks in different rings may differ by at most
one LOD level, producing **T-junction** cracks without stitching.

**Skirts** (vertical edge geometry, ADR-013 original sketch) are **deferred to
Phase 2C.1** unless preview shows unacceptable ring-boundary artifacts.

Neighbor seam weld / async neighbor mesh refresh (disabled in Phase 2B.6) remains
a separate follow-up, not a Phase 2C blocker.

### Far terrain (Phase 2C)

Distant resident chunks at **Eighth** LOD satisfy ROADMAP "basic far terrain
representation" for Phase 2C. No separate aggregate mesh, clipmap, or impostor
shell.

### Phase 2C non-goals

- Screen-space error LOD selection
- Geomorphing / continuous LOD
- Building all four LODs at load time
- Synchronous mesh generation on any thread used by apply
- LOD in chunk assets or `WorldData`
- `AssetLoader`, region containers, masks, doodads, gameplay, simulation
- Custom shaders, terrain texturing
- Public `chunk_lod` query (ADR-005 deferred)
- Rewriting streaming/residency policy

Deferred beyond Phase 2C (require profiling per ADR-004): geomorphing, GPU
tessellation, clipmaps, impostors, virtual texturing, custom render phases, LOD
edge stitching beyond skirts.

## Known seam-normal nuance

Shared edge **positions** are continuous across chunks (ADR-008), so meshes meet
without gaps. **Normals** at a tile's border, computed only from in-tile samples,
are slightly discontinuous between neighbors. Phase 2 accepts this with the
standard material; computing border normals from neighboring resident tiles is a
documented follow-up, not Phase 2A scope.

# Rationale

A pure builder is the cleanest way to keep mesh generation derived and testable.
Restricting LOD to mesh-resolution subsampling satisfies the ROADMAP LOD criteria
without violating ADR-004.

**Lazy per-resident caching** avoids building four meshes per chunk at load (CPU
and memory waste) while making repeat LOD transitions cheap once a level has been
visited (common when the camera oscillates near a ring boundary). Immediate
handle swap when cached preserves the main-thread invariant: transitions never
wait on mesh generation.

Ring-based Chebyshev selection reuses the same chunk-grid semantics as streaming
and reduces inter-LOD boundaries compared to arbitrary per-chunk selection.

# Consequences

Benefits:

- Testable, side-effect-free mesh generation on synthetic heightfields.
- Phase 2C extends the proven async pipeline without streaming rewrites.
- Cached LOD meshes amortize async cost across camera movement.
- Full heightfield authority preserved in `WorldData`.

Costs:

- Up to four mesh handles per resident chunk over time (bounded by resident
  count × levels actually visited, not always four).
- Per-chunk cache and in-flight LOD build tracking add terrain-runtime complexity.
- T-junction cracks at ring boundaries until skirts (2C.1) or edge stitching.
- Border-normal discontinuity remains until neighbor seam weld returns.

# Alternatives Considered

## Custom terrain shader/material in Phase 2

Rejected: violates ADR-004 without profiling evidence; standard material suffices
for the Phase 2 criteria.

## Multi-LOD and skirts in Phase 2A

Rejected for 2A: LOD is breadth; including it would obscure whether the core
data → mesh → render path is correct and add crack-handling before it is needed.

## Continuous LOD / geomorphing

Rejected for Phase 2: renderer complexity without evidence (ADR-004).

## Rebuild on every LOD change (no cache)

Rejected for Phase 2C: repeat transitions across ring boundaries would re-pay
async mesh build cost and increase main-thread apply churn. Lazy caching gives
better steady-state behavior with bounded memory.

## Build all four LODs at materialization

Rejected: 4× async mesh work and memory at load for levels that may never be
displayed. Lazy build-on-first-need is sufficient.

## Skirts in Phase 2C core

Deferred to 2C.1: ring-based selection is the zero-cost mitigation; skirts added
only if preview requires them.

# Notes

The builder's signature takes a LOD level from the start so Phase 2C is additive.
Cross-references: ADR-010 (runtime owns LOD cache), ADR-012 (async
materialization, streaming unchanged), ADR-014 (`PrimaryViewFocus` for selection).
