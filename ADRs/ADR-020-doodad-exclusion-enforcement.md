# ADR-020: Doodad Exclusion Enforcement

# Status

Accepted (Phase 3F — exclusion filter)

# Context

ADR-015 introduced [`DoodadExclusionZone`] as world-scoped, data-only spherical
regions on [`WorldData`]. ADR-018–019 established procedural generation and
materialization without enforcing those zones.

Designers need authored exclusion regions (settlements, roads, POIs) to suppress
procedural doodad placement while still allowing intentional authored placements.

Phase 3F is a **generation filter only**: no terrain, slope, biome, rendering,
ECS, streaming, or persistence.

# Design review: existing zone structure

[`DoodadExclusionZone`] with `center: WorldPosition` and `radius_meters: f32` is
**sufficient** for Phase 3F circular exclusion. No schema extension required.

# Decision

## Filter before materialization

Exclusion applies to **procedural candidates** after generation and **before**
materialization:

```text
generate_chunk_doodads → filter_candidates_by_exclusion_zones → materialize_candidates
```

Filtering at candidate stage keeps generation deterministic and materialization
focused on accepted placements. Excluded candidates never reach `create_doodad`.

## World-space radius rule

A candidate is excluded when, for any zone:

```text
distance(candidate.position, zone.center) <= zone.radius_meters
```

Distance uses authoritative [`WorldPosition::to_global`] (3D Euclidean). The
boundary is **inclusive** (`<=`).

## Pure filter API

`src/world/doodad/exclusion/filter.rs` provides:

- [`filter_candidates_by_exclusion_zones`] — pure, deterministic, side-effect free
- [`position_excluded_by_zones`] — single-position test helper

Uses existing [`WorldData::doodad_exclusion_zones`] — no second exclusion store.

## Materialization integration

[`materialize_candidates_with_exclusion`] reads zones from [`WorldData`], filters,
then delegates to existing materialization logic (no duplication).

[`MaterializationOptions::apply_exclusion_zones`] enables the same path.

[`DoodadMaterializationReport::excluded_by_zone`] counts filtered candidates.
`candidates_received` reflects the **pre-filter** batch size.

## Authored vs procedural

| Path | Exclusion enforced? |
|------|---------------------|
| Procedural candidates → materialize with exclusion | **Yes** |
| [`create_doodad`] / [`move_doodad`] (authoring) | **No** |

Intentional: designers may place authored doodads inside exclusion zones.

# Future integration

## Polygon zones

[`ExclusionFilterOptions`] reserves polygon support. Filter API can gain shape
dispatch without changing materialization entry points.

## Terrain validation

Terrain height/slope checks remain separate filters (reserved on
[`MaterializationOptions::validate_terrain`]). Exclusion runs first on XZ/world
position; terrain validation would compose after exclusion in the pipeline.

## Persistence

Exclusion zones are authored world data; persistence stores zone definitions.
Procedural baseline regenerates; exclusion filter re-applies on materialization.

# Rationale

Pre-materialization filtering preserves ADR-018 determinism, avoids orphan
[`DoodadId`] allocation for rejected placements, and keeps exclusion out of the
authoring API. Circular zones match Phase 3F scope without premature geometry
generalization.

# Consequences

Benefits:

- Clear pipeline stage for procedural suppression
- Authored placement flexibility preserved
- Reuses existing WorldData zone storage

Costs:

- 3D distance may include Y delta; terrain-specific refinement deferred
- Large zone lists are O(candidates × zones); spatial indexing deferred

# Alternatives Considered

## Enforce exclusion in authoring API

Rejected: blocks intentional authored placements inside zones.

## Filter during generation

Rejected: couples generator to WorldData zones; breaks pure generation boundary.

## Post-materialization removal

Rejected: wastes ids and complicates duplicate index; violates filter-before-insert.

# Notes

- Cross-references: ADR-015, ADR-017, ADR-018, ADR-019, ARCHITECTURE Doodad Layer.
- Module: `src/world/doodad/exclusion/`.

[`WorldData`]: ../src/world/data.rs
[`DoodadExclusionZone`]: ../src/world/doodad/exclusion/zone.rs
[`filter_candidates_by_exclusion_zones`]: ../src/world/doodad/exclusion/filter.rs
[`materialize_candidates_with_exclusion`]: ../src/world/doodad/materialization/materialize.rs
[`create_doodad`]: ../src/world/doodad/authoring.rs
[`WorldPosition::to_global`]: ../src/world/coordinates.rs
[`MaterializationOptions`]: ../src/world/doodad/materialization/options.rs
[`ExclusionFilterOptions`]: ../src/world/doodad/exclusion/options.rs
