# ADR-067: Terrain Validation and Runtime Contracts (REVIEW-B4)

# Status

Accepted (REVIEW-B4 terrain query consistency; REVIEW-B6 runtime heightfield contracts)

# Context

Terrain height data is authoritative in [`WorldData`](../src/world/data.rs) via per-chunk
[`Heightfield`](../src/world/terrain/heightfield.rs) samples (ADR-008). Runtime terrain
meshes, LOD, and render exaggeration are derived presentation only (ADR-010).

REVIEW-B4 audited simulation consumers and found:

- Inconsistent validation (some paths checked `world.get` and height separately)
- `Heightfield::sample` silently clamped out-of-domain coordinates to chunk edges
- Slope-unavailable positions were misclassified as "too steep" via
  [`is_position_slope_walkable`](../src/world/terrain/query.rs)
- No shared structured error type across grounding, navigation, doodad validation, and dev tools
- Per-domain `TerrainUnavailable` enums without a common query contract

# Decision

## Authoritative terrain query contract

Simulation terrain queries must **never fabricate data**.

| Outcome | Meaning |
|---------|---------|
| `Ok(value)` | Resident chunk + valid domain + computable result |
| `Err(TerrainQueryError::ChunkNotResident)` | Owning chunk not in `WorldData` |
| `Err(TerrainQueryError::InvalidTerrainCoordinate)` | Chunk-local XZ outside heightfield domain |
| `Err(TerrainQueryError::SlopeUnavailable)` | Height present but slope cannot be estimated |

**Fail-closed policy:** movement, grounding, navigation, doodad validation, and combat
standoff treat all errors as blocking/unavailable. Presentation layers (camera bind)
may fail-open where documented (ADR-014).

## Shared query layer (`src/world/terrain/query.rs`)

| API | Role |
|-----|------|
| [`try_sample_height_at_position`](../src/world/terrain/query.rs) | Authoritative height sample |
| [`try_ground_world_position`](../src/world/terrain/query.rs) | Height + grounded `WorldPosition` |
| [`ground_world_position`](../src/world/terrain/query.rs) | Convenience `Option` wrapper |
| [`slope_at`](../src/world/terrain/query.rs) | ADR-005 slope query |
| [`estimate_slope_degrees`](../src/world/terrain/query.rs) | Heightfield-local slope estimate |
| [`classify_slope_walkability`](../src/world/terrain/query.rs) | Walkable / Unavailable / TooSteep |

[`Heightfield::try_sample`](../src/world/terrain/heightfield.rs) enforces domain checks.
[`Heightfield::sample`](../src/world/terrain/heightfield.rs) retains edge clamping for
render/import convenience only — **not** for simulation.

[`WorldData::sample_height_at_position`](../src/world/data.rs) delegates to
`try_sample_height_at_position` and maps failures to `None`.

## Chunk residency

**Simulation authority:** `WorldData::is_chunk_loaded` / `WorldData::get`.

**Runtime tracker:** [`ChunkResidencyTracker`](../src/terrain/residency.rs) tracks mesh
lifecycle and gates render visibility — it does not override simulation truth.

Non-resident chunk → `TerrainQueryError::ChunkNotResident`. No subsystem may assume
terrain exists because a render mesh is visible.

## Slope classification (ADR-066 alignment)

Consumers that need distinct failure reasons must use
[`classify_slope_walkability`](../src/world/terrain/query.rs), not boolean
`is_position_slope_walkable`:

| Consumer | Unavailable handling |
|----------|---------------------|
| Movement | `BlockedMovementReason::SlopeUnavailable` |
| Doodad validation | `skipped_slope_unavailable` counter |
| Dev placement | `PlacementRejectReason::SlopeUnavailable` |
| Interaction query | Returns `None` (fail-closed) |
| Navigation grid | Cell not walkable |

## Runtime validation

- [`TerrainDataError`](../src/world/terrain/mod.rs) — heightfield construction
- [`validate_heightfield_against_config`](../src/world/terrain/contract.rs) — spacing/samples vs [`WorldConfig`]
- [`validate_loaded_chunk`](../src/terrain/load.rs) — on-disk chunk metadata vs heightfield
- Import/decode paths return structured `DecodeError` / `TerrainAssetError`

**REVIEW-B6 heightfield contract:**

| Rule | Enforcement |
|------|-------------|
| All samples finite | `Heightfield::from_samples` rejects NaN/±Inf |
| Spacing > 0 and finite | `TerrainDataError::NonPositiveSpacing` |
| Spacing matches `WorldConfig::meters_per_sample` | `TerrainDataError::SpacingMismatch` (±1e-5 m) |
| Samples per edge matches contract | `TerrainDataError::InvalidDimensions` |

`WorldConfig` owns the geometric contract; loaded chunk metadata is validated against it.

Recoverable query failures return `Result` / `TerrainQueryError` — no panics in
simulation paths.

**REVIEW-B6 mesh finalization:**

- [`seam_weld_heights`](../src/terrain/spawn.rs) + [`build_chunk_mesh_finalized`](../src/terrain/spawn.rs)
  are the shared seam-welding entry for initial materialization and LOD rebuilds.
- Async mesh tasks receive a precomputed [`ChunkMeshSeamWeld`] captured on the main
  thread before spawn — identical rules to synchronous rebuilds.

**REVIEW-B6 async completion safety:**

- Stale IO/decode/mesh results are discarded via generation checks in
  [`ChunkMaterializationQueue::poll_in_flight`](../src/terrain/materialize.rs).
- Recoverable races (unload, LOD change, superseded generation) log and reject —
  no `expect`/`unwrap` on mesh LOD state in production paths.

**REVIEW-B6 production albedo policy:**

- Missing albedo sidecar logs a warning and uses [`production_albedo_fallback`](../src/terrain/albedo.rs)
  (`AlbedoFallback::Neutral`).
- `AlbedoFallback::HeightGradient` is dev-only via explicit opt-in — never the
  production default.

## Future integration

Building placement, water surface queries, and biome masks should call the same
`try_*` query layer and map `TerrainQueryError` at subsystem boundaries.

# Consequences

**Benefits:**

- One contract for height/slope across movement, grounding, navigation, doodads, combat
- Out-of-domain coordinates fail explicitly instead of returning clamped edge heights
- Structured errors aid debug inspector and future player feedback

**Costs:**

- Callers that relied on clamped `sample` at chunk edges must use in-domain coordinates
- Slightly more explicit error mapping at subsystem boundaries

# References

- ADR-005 (query API — `slope_at` implemented)
- ADR-008 (heightfield model)
- ADR-010 (runtime layer boundaries)
- ADR-021 (doodad terrain validation)
- ADR-029 (unit grounding)
- ADR-032 (navigation)
- ADR-066 (movement blocking semantics)
