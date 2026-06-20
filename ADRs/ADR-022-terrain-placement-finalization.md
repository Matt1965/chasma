# ADR-022: Terrain Placement Finalization

# Status

Accepted (Phase 3H — placement finalization)

# Context

ADR-021 added terrain **validation** as a filter step: candidates are rejected
when resident heightfield data violates catalog height/slope constraints.
Validation intentionally does **not** modify candidate positions — it only
accepts or rejects.

Procedural generation emits immutable [`DoodadSpawnCandidate`] values (ADR-018).
Materialization needs a resolved transform (especially terrain-aligned Y)
before creating [`DoodadRecord`] instances (ADR-019).

Phase 3H introduces a dedicated **placement finalization** stage between
validation and materialization.

# Why snapping is separate from validation

| Concern | Validation (ADR-021) | Finalization (ADR-022) |
|---------|----------------------|-------------------------|
| Purpose | Filter by constraints | Resolve transform |
| Mutates candidate | **No** | Produces new type |
| Height check | Compare sampled terrain to bounds | Replace Y with sampled terrain |
| On missing terrain | Skip + report | Skip + report |

Keeping validation read-only preserves deterministic generation output for
replay, debugging, and future re-validation. Finalization owns transform
resolution.

# Decision

## Immutable candidate philosophy

[`DoodadSpawnCandidate`] is **not** modified. Finalization emits
[`FinalizedDoodadPlacement`], a separate type with the same fields but
representing the materialization-ready transform.

Pipeline stages:

```text
Candidate → (filters) → Validated Candidate → Finalized Placement → Instance
```

## Module location

`src/world/doodad/placement/`:

- `pose.rs` — existing [`DoodadPlacement`] on records
- `finalized.rs` — [`FinalizedDoodadPlacement`]
- `finalize.rs` — [`finalize_placements`]

## Placement rules (Phase 3H)

From resident [`WorldData`] heightfield at candidate local `(x, z)`:

- **Y**: replace with sampled terrain height when `snap_to_terrain` is enabled
- **X/Z**: unchanged
- **Rotation / scale**: preserve candidate values unless catalog believability is enabled (R7)

### Catalog believability (R7)

When [`MaterializationOptions::apply_catalog_believability`] is true (default in
[`MaterializationOptions::procedural_default`]), [`finalize_placements`] applies
deterministic scale and optional yaw from [`DoodadDefinition`] fields populated by
Excel import:

| Excel column | Catalog field | Finalization behavior |
|--------------|---------------|------------------------|
| Min Size / Max Size | `min_scale`, `max_scale` | Uniform scale in range; fixed when equal |
| Random Rotation | `random_rotation_y` | Deterministic yaw 0..360° when true; identity when false |

Seeding uses chunk coordinates, procedural instance seed, and definition id.
[`DoodadSpawnCandidate`] is never mutated. Micro-position jitter is deferred until
a schema field exists.

Rust must not introduce hardcoded per-doodad visual tuning; extend the Excel schema
instead.

The snapped Y written to [`DoodadRecord`] is **authoritative terrain height** in
world units. The doodad runtime may multiply render Y by
[`TerrainRenderAssets::vertical_scale`] (ADR-010, ADR-023) for visual alignment
with exaggerated terrain meshes. [`WorldData`] is never scaled.

## Materialization integration

[`MaterializationOptions::snap_to_terrain`] defaults to **true** via
[`MaterializationOptions::procedural_default`] ([`Default`]).

Pipeline in [`materialize_candidates_with_options`]:

```text
generate → exclusion (optional) → terrain validation (optional)
  → placement finalization → materialize finalized placements
```

Materialization consumes [`FinalizedDoodadPlacement`] only; placement logic is
not duplicated in the materialization loop.

## Reporting

[`DoodadMaterializationReport`] gains:

- `placements_finalized`
- `terrain_snaps_applied`

Finalization `skipped_terrain_unavailable` merges into the materialization report.

## Authored vs procedural

| Path | Placement finalization? |
|------|-------------------------|
| Procedural materialization | **Yes** (when enabled via options) |
| [`create_doodad`] / [`move_doodad`] | **No** |

# Future extension seams

Finalization is the intended home for procedural placement refinements
without touching generation output:

- align rotation to terrain normal
- ground offset and species-specific placement
- slope alignment, rock embedding, tree root offsets

R7 implemented catalog-driven random yaw and scale here (`variation.rs`).
Reserve additional refinements as extensions to `finalize_placements` or companion
functions in the same module — not in validation or generation.

# Non-goals (Phase 3H)

No normal alignment, mesh queries, rendering, ECS, streaming, save/load, or
collision.

# References

- ADR-018: Procedural generation
- ADR-019: Procedural materialization
- ADR-020: Exclusion enforcement
- ADR-021: Terrain validation

Height sampling uses [`WorldData::sample_height_at_position`] (shared with ADR-021).

[`DoodadSpawnCandidate`]: ../src/world/doodad/generation/candidate.rs
[`DoodadRecord`]: ../src/world/doodad/record.rs
[`DoodadPlacement`]: ../src/world/doodad/placement/pose.rs
[`FinalizedDoodadPlacement`]: ../src/world/doodad/placement/finalized.rs
[`finalize_placements`]: ../src/world/doodad/placement/finalize.rs
[`WorldData`]: ../src/world/data.rs
[`MaterializationOptions::snap_to_terrain`]: ../src/world/doodad/materialization/options.rs
[`MaterializationOptions::apply_catalog_believability`]: ../src/world/doodad/materialization/options.rs
[`materialize_candidates_with_options`]: ../src/world/doodad/materialization/materialize.rs
[`DoodadMaterializationReport`]: ../src/world/doodad/materialization/report.rs
[`create_doodad`]: ../src/world/doodad/authoring.rs
[`move_doodad`]: ../src/world/doodad/authoring.rs
