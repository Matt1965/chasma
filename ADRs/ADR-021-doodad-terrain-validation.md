# ADR-021: Doodad Terrain Validation

# Status

Accepted (Phase 3G — terrain validation filter)

# Context

ADR-016 defined optional placement constraints on [`DoodadDefinition`]:

- `min_height` / `max_height` (world meters)
- `max_slope_degrees`

ADR-018–020 established procedural generation, materialization, and exclusion
filtering without terrain constraint enforcement. Phase 3G adds a **data/filter
step** that validates candidates against **resident** world terrain before
materialization.

This phase is **not** terrain snapping, collision, rendering, ECS, streaming,
save/load, or terrain-runtime coupling.

# Design review

## Height authority

[`WorldData::height_at`] and [`ChunkData::heightfield`] are the authoritative
height sources (ADR-005). Validation reads [`Heightfield::sample`] only — not
terrain meshes, render entities, or the terrain runtime module.

## Catalog constraints

[`DoodadCatalog`] owns placement constraint fields; validation interprets them
during procedural materialization only.

## Pipeline placement

Terrain validation runs **after** exclusion filtering and **before**
materialization:

```text
generate_chunk_doodads
  → filter_candidates_by_exclusion_zones (optional)
  → filter_candidates_by_terrain (optional)
  → finalize_placements (optional)
  → materialize finalized placements
```

# Decision

## Module location

`src/world/doodad/terrain_validation/`:

- `filter.rs` — [`filter_candidates_by_terrain`], [`TerrainValidationResult`]
- `slope.rs` — deterministic slope estimate from heightfield samples

Pure, deterministic, side-effect free. No [`WorldData`] mutation.

## Resident terrain requirement

For each candidate, validation requires the owning chunk to be **resident** in
[`WorldData::chunks`]. If the chunk is missing or heightfield data cannot be
sampled:

- skip the candidate
- increment `skipped_terrain_unavailable`
- **no fallback** (no default height, no mesh query)

## Height constraint behavior

Sample terrain height at the candidate's chunk-local `(x, z)` via the resident
heightfield. Compare **sampled terrain height** against `min_height` /
`max_height` when set.

Candidate `WorldPosition` Y is **not** modified in this phase. Procedural
candidates often carry `y = 0`; constraints apply to the terrain surface at the
placement XZ, not to an authored vertical offset on the candidate.

Future terrain snapping is implemented in ADR-022 (placement finalization).

## Slope constraint behavior

When `max_slope_degrees` is set, estimate slope using forward finite differences
over one heightfield sample spacing at the candidate local position. Slope is
returned in **degrees** for comparison with the catalog field.

If the neighborhood required for finite differences lies outside the heightfield
domain, skip the candidate and increment `skipped_slope_unavailable`.

## Definition gate

Before terrain checks, validation requires the definition to exist and be
enabled (same rules as materialization). Invalid/disabled candidates are
counted and never reach materialization when terrain validation is enabled.

## Materialization integration

[`MaterializationOptions::validate_terrain`] enables the filter inside
[`materialize_candidates_with_options`]. Existing materialization logic is not
duplicated.

[`DoodadMaterializationReport`] gains terrain rejection counters:

- `skipped_terrain_unavailable`
- `skipped_height_constraint`
- `skipped_slope_constraint`
- `skipped_slope_unavailable`

`candidates_received` remains the pre-filter batch size.

## Authored vs procedural

| Path | Terrain validation enforced? |
|------|------------------------------|
| Procedural candidates → materialize with `validate_terrain` | **Yes** |
| [`create_doodad`] / [`move_doodad`] (authoring) | **No** |

Intentional: designers may place authored doodads outside catalog terrain bounds.

# Why not mesh / render terrain

Mesh and render representations may be LOD-reduced, stitched asynchronously, or
absent for non-resident chunks. [`WorldData`] heightfields are the simulation
authority and match procedural generation inputs. Using render data would
introduce coupling, nondeterminism, and false rejects/accepts.

# Future work

- **Biome / density filters**: additional pre-materialization filters in the same pipeline
- **Cross-chunk slope**: central differences spanning chunk boundaries when needed

# References

- ADR-015: Doodad data foundation
- ADR-016: Doodad catalog
- ADR-018: Procedural generation
- ADR-019: Procedural materialization
- ADR-022: Terrain placement finalization

Shared height sampling: [`WorldData::sample_height_at_position`].

[`WorldData::height_at`]: ../src/world/data.rs
[`WorldData::chunks`]: ../src/world/data.rs
[`WorldData`]: ../src/world/data.rs
[`ChunkData::heightfield`]: ../src/world/chunk.rs
[`ChunkData`]: ../src/world/chunk.rs
[`Heightfield::sample`]: ../src/world/terrain/heightfield.rs
[`DoodadDefinition`]: ../src/world/doodad/catalog/definition.rs
[`DoodadCatalog`]: ../src/world/doodad/catalog/registry.rs
[`filter_candidates_by_terrain`]: ../src/world/doodad/terrain_validation/filter.rs
[`TerrainValidationResult`]: ../src/world/doodad/terrain_validation/filter.rs
[`MaterializationOptions::validate_terrain`]: ../src/world/doodad/materialization/options.rs
[`materialize_candidates_with_options`]: ../src/world/doodad/materialization/materialize.rs
[`DoodadMaterializationReport`]: ../src/world/doodad/materialization/report.rs
[`create_doodad`]: ../src/world/doodad/authoring.rs
[`move_doodad`]: ../src/world/doodad/authoring.rs
