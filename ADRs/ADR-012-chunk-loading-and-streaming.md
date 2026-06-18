# ADR-012: Chunk Loading, Streaming, and Lifecycle

# Status

Accepted (Phase 2B synchronous streaming proven; Phase 2B.5 async materialization
designed)

# Context

ADR-011 defines the pre-chunked asset format and a world manifest. This ADR
decides how those assets are loaded into the authoritative `WorldData` at runtime,
and how chunk residency is managed over time.

Forces:

- ADR-009 deferred a Bevy `AssetLoader` until a concrete consumer exists.
- ADR-010: the Terrain Runtime Layer owns the loading *process*; the World Data
  Layer owns the *data*. Phase 2 is built in vertical slices.
- AGENTS.md Groundwork Rule: do not build infrastructure ahead of a consumer.
- Phase 2B proved synchronous residency lifecycle (catalog, hysteresis, budgets,
  view-focus seam) but exposed main-thread hitches during chunk materialization.
- ADR-014: camera and terrain remain decoupled; view position crosses layers via
  `PrimaryViewFocus` and an app-layer bridge.
- ARCHITECTURE Scalability Rule: localized, chunk-local operations over global
  scans.

# Decision

## Loading mechanism is split from payload decode

The **decode/parsing** of the manifest and chunk payloads (ADR-011) is implemented
as **delivery-agnostic functions**: they take bytes (or text) and produce a
manifest value / `ChunkData`, with no dependency on how the bytes were obtained.
All delivery mechanisms (synchronous read, task-pool async, or a future optional
`AssetLoader`) reuse these same functions.

## Phase 2A: synchronous file loading (no AssetLoader)

Phase 2A loads the world synchronously at startup:

- Read manifest and chunk files from disk.
- Decode via shared functions, `insert` each `ChunkData` into `WorldData`, spawn
  derived render entity (ADR-010, ADR-013).
- No view-distance-driven load or unload, and no LOD switching.

## Phase 2A: Phase 1 `WorldData` semantics are unchanged

Phase 2A keeps Phase 1 `WorldData` as-is (inserted chunks are the loaded world;
extent from inserted chunks). No `remove`, no authored/resident split.

## Data contract

- On **resident** chunks, the runtime validates against `WorldConfig` and holds
  `ChunkData` in `WorldData`, with a derived render entity.
- `height_at` / `is_chunk_loaded` operate on **resident** chunks only (ADR-005).
  Chunks that are authored or in-flight loading return "not loaded."

## Determinism

The **asset data** is deterministic (ADR-011). Residency and load timing are
runtime concerns.

## Phase 2B: synchronous streaming and the residency model (proven baseline)

Phase 2B implemented chunk residency lifecycle **synchronously**. This slice
proved the policy layer; main-thread materialization hitches motivated Phase
2B.5.

### Manifest catalog (terrain layer, separate from residents)

At startup, load manifest once into **`TerrainWorldCatalog`** (metadata only):

- validated `ManifestConfig` against runtime `WorldConfig`
- `ChunkCoord` → chunk file path mapping
- **authored extent** from manifest chunk list

Immutable for the session. Does not populate `WorldData`.

### Authored extent vs resident set (world layer)

- **`authored_extent`**: set once from catalog; `extent()` reports this.
- **Resident set**: `WorldData.chunks` — `ChunkData` currently in memory.
- **`WorldData::remove`**: evicts resident data only; does not change authored
  extent.
- **`insert`**: does not expand authored extent.

`is_chunk_loaded` means **resident**. `WorldData` does **not** track loading or
in-flight states (Phase 2B.5).

### Synchronous on-demand delivery (Phase 2B — superseded for runtime streaming)

Phase 2B read chunk files synchronously in the streaming system:

- `fs::read_to_string` → `decode_chunk` → validate → `insert` → spawn mesh

**Phase 2B.5 replaces this hot path** with async materialization. The synchronous
`load_chunk_from_path` / `load_world_from_manifest` paths are **retained for
tests and offline tooling only**.

### Residency policy (terrain layer — unchanged in Phase 2B.5)

Each frame:

1. Read view center from `PrimaryViewFocus`.
2. `desired_load_set` = authored chunks within **load radius** (O(r²)).
3. `keep_resident_set` = authored chunks within **unload radius** (O(r²)).
4. Hysteresis: **`unload_radius_chunks >= load_radius_chunks`**.
5. Per-frame **request** and **unload** budgets.

Phase 2B.5 changes **how** `to_request` is fulfilled, not this policy.

### Unload ordering (unchanged)

For each resident chunk outside `keep_resident_set`:

1. Despawn `TerrainChunkMesh` entities.
2. `WorldData::remove(chunk)`.
3. `ChunkResidencyTracker` → `Absent` (and cancel any in-flight load for that id).

### View focus seam (ADR-014 — unchanged)

- `PrimaryViewFocus` in `src/view/`; app bridge from `RtsCameraState`.
- Terrain must not import `crate::camera`. Camera must not import terrain/world.

### No streaming messages (Phase 2B through 2B.5)

No `TerrainChunkLoaded`, `TerrainChunkUnloaded`, or other Bevy `Message`/`Event`
types for streaming until a real consumer exists.

---

## Phase 2B.5: async chunk materialization

Phase 2B.5 replaces **only the materialization step** of Phase 2B streaming.
Residency policy, catalog model, manifest/chunk RON format (ADR-011), hysteresis,
budgets, unload order, and view-focus seam are **unchanged**.

### What Phase 2B.5 changes

| Unchanged | Changed |
|-----------|---------|
| `diff_streaming_residency` policy (radii, hysteresis, budgets) | Sync file read + decode + mesh build in `Update` |
| `TerrainWorldCatalog`, manifest paths | Task-pool async pipeline |
| `WorldData` authority | `ChunkResidencyTracker` for in-flight state |
| Unload: despawn → remove | Request scheduler + apply system |
| `decode_chunk`, `build_chunk_mesh` (pure fns) | Where those functions execute |

### What Phase 2B.5 does not introduce

- No `AssetLoader` / `AssetServer` (deferred until region/packed delivery needs it)
- No region containers, masks, gameplay, simulation, pathfinding, multiplayer
- No streaming messages/events
- No camera ↔ terrain imports

Mesh-resolution LOD (ADR-013 Phase 2C) is a **separate** system chain stage
added after Phase 2B.5; it does not change residency policy.

### Execution split

**Main thread (`Update`):**

- Residency diff (pure policy; existing `streaming.rs` functions, extended inputs)
- Request scheduling (start bounded new loads; cancel stale)
- Apply completed async results (validate, `WorldData::insert`, spawn entity)
- Unload residents (despawn mesh → `remove` → tracker `Absent`)
- `Assets<Mesh>::add` (handle registration only; mesh already built off-thread)

**Background threads (Bevy task pools):**

- **`IoTaskPool`**: read chunk file → owned `String` (or bytes)
- **`AsyncComputeTaskPool`**: `decode_chunk` → `(ChunkId, ChunkData)`
- **`AsyncComputeTaskPool`**: `build_chunk_mesh_scaled` → `Mesh`

`decode_chunk` and `build_chunk_mesh` remain pure, ECS-free, and reusable.

### Chunk residency tracker (terrain layer only)

`ChunkResidencyTracker` is the **sole** owner of non-catalog chunk lifecycle
state. `WorldData` remains unaware of loading.

Per `ChunkId`:

| State | Meaning |
|-------|---------|
| **Absent** | Not resident; no in-flight load (or cancelled) |
| **Loading { generation }** | Async materialization in progress |
| **Resident** | `WorldData` holds `ChunkData`; mesh entity may exist |

Rules:

- At most one in-flight load per `ChunkId`.
- Each new load request bumps a **generation** token; completions must match the
  tracker's current generation to apply.
- If completion arrives when coord ∉ `keep_resident_set`, **discard** (no insert,
  no spawn).
- Unload of a `Loading` chunk: set `Absent`, invalidate generation, discard
  completion when it arrives.
- Unload of a `Resident` chunk: despawn → `WorldData::remove` → `Absent`.

### Async pipeline (per chunk)

```
Request (main):
  tracker: Absent → Loading { generation }

IoTaskPool:
  read height chunk → String
  read optional albedo sidecar → bytes (decode deferred)

AsyncComputeTaskPool (chained):
  decode_chunk(&height_text) → (ChunkId, ChunkData)
  decode albedo from IO bytes (when present)
  build_chunk_mesh_scaled(&heightfield, …) → Mesh

Apply (main, when task complete):
  if generation matches AND coord ∈ keep_resident_set:
    validate_loaded_chunk (WorldConfig)
    WorldData::insert
    spawn TerrainChunkMesh with prebuilt Mesh
    tracker → Resident
  else:
    discard
    tracker → Absent (if still Loading with same generation)
```

Albedo sidecar **file reads** run on `IoTaskPool`; decode and mesh build stay on
`AsyncComputeTaskPool`. Albedo is presentation data stored in terrain runtime
(`TerrainChunkAlbedo`), not in `WorldData`.

`max_loads_per_frame` bounds **new requests**, not apply count.

### System ordering (`Update`)

Implemented as one chained set in `TerrainRuntimePlugin`
(`TerrainStreamingSystems`), after camera control and view-focus publish
(ADR-014):

1. **`stream_terrain_chunks`** — discard out-of-ring pipeline work; compute
   residency diff; start bounded IO for `to_load` (nearest-first).
2. **`poll_chunk_materializations`** — advance IO → decode → async mesh build;
   queue materialized results.
3. **`apply_cached_lod_swaps`** — instant mesh handle swaps for cached LOD
   (Phase 2C).
4. **`request_missing_lod_builds`** — enqueue display-driven and predictive LOD
   rebuilds for **already-resident** chunks only.
5. **`poll_lod_builds`** — poll async LOD mesh builds.
6. **`apply_chunk_materializations`** — validate; `WorldData::insert`; spawn
   prebuilt mesh entities; tracker → `Resident`.
7. **`unload_terrain_chunks`** — despawn mesh → `WorldData::remove` → tracker
   `Absent` for chunks outside the keep ring.

Unload runs **after** apply so completed work is not discarded before insertion.
LOD systems run between materialization poll and apply so display swaps and
prefetch builds see up-to-date residency.

**Streaming vs LOD:** [`TerrainStreamingSettings`] controls which chunks exist
(load/keep radii, IO/decode/mesh budgets). [`TerrainLodSettings`] only selects
mesh resolution among resident chunks; predictive prefetch (`prefetch_warmup_lod`)
warms one finer LOD step for resident catalog coords within load radius +2 and
does not expand the load radius.

**In-flight cancellation:** `discard_outside_residency_sets` revokes pipeline
entries outside the current keep or desired load rings when focus moves.

### Sync path retained

`load_world_from_manifest` and `load_chunk_from_path` remain for unit tests and
tooling. Dev preview uses async streaming only.

### Deferred beyond Phase 2B.5

- `AssetLoader` / `AssetServer` (optional future delivery for regions/packed assets)
- Region containers, masks, doodads, gameplay, simulation, streaming messages
- Neighbor seam refresh on apply (explicitly disabled; deferred)

# Rationale

Phase 2B proved residency lifecycle correctness. Hitches came from synchronous
materialization on the main thread (RON parse + full mesh build), not from
streaming policy. Phase 2B.5 moves I/O and CPU materialization off-thread while
keeping the proven diff/hysteresis model.

Task pools (`IoTaskPool`, `AsyncComputeTaskPool`) are the smallest correct Bevy
0.18 approach that moves **mesh build** off the main thread without introducing
`AssetLoader` machinery or blurring authoritative data into `Assets<T>`.

`ChunkResidencyTracker` keeps loading state out of `WorldData` (ADR-010, data
first). Generation tokens prevent stale async writes after cancel or unload.

# Consequences

Benefits:

- Main thread no longer blocked by per-chunk read/decode/mesh build.
- Phase 2B streaming policy reused verbatim.
- `decode_chunk` / `build_chunk_mesh` unchanged; delivery swapped only.
- Clear cancellation and single-flight semantics.

Costs:

- In-flight memory holds decoded data + mesh until apply or discard.
- Tracker + task polling add terrain-runtime complexity.
- Chunk may pop in one frame after async completion (acceptable; no hitch).
- Sync load path remains for tests.

# Alternatives Considered

## AssetLoader for Phase 2B.5

Rejected: does not by itself move mesh build off main thread; adds `TypePath`/handle
complexity without simplifying residency cancellation. May return when region
containers justify engine asset caching.

## Loading state in WorldData

Rejected: conflates authoritative residents with runtime process state (ADR-010).

## Streaming Messages in Phase 2B.5

Rejected: no consumer (AGENTS.md Groundwork Rule).

## Changing hysteresis or catalog model

Rejected: Phase 2B policy is proven; only materialization changes.

# Notes

- Fix historical ADR text: hysteresis requires `unload_radius_chunks >=
  load_radius_chunks` (keep ring ≥ load ring).
- Authored extent from manifest chunk list at catalog init; no ADR-011 format
  change required.
- Cross-references: ADR-010, ADR-011, ADR-013 (mesh builder), ADR-014.
