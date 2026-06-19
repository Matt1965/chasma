# ADR-019: Procedural Doodad Materialization

# Status

Accepted (Phase 3E — materialization foundation)

# Context

Phase 3D (ADR-018) produces deterministic [`DoodadSpawnCandidate`] lists without
touching [`WorldData`]. Phase 3C (ADR-017) provides authoritative instance
placement via the authoring API. A bridge is required to convert candidates into
materialized [`DoodadRecord`] instances for runtime simulation and future rendering.

ARCHITECTURE.md Persistence Rule:

> Procedural generation creates the initial world state. After world creation,
> gameplay changes are represented as overrides. Procedural generation should
> never overwrite persistent world state.

Large-world scalability requires distinguishing **reproducible baseline content**
from **materialized runtime instances** and **persistent overrides**.

Phase 3E is world-data only: no ECS, rendering, streaming, or save/load.

# Design review: materialization models

## Option A — Permanent WorldData records

Every generated doodad becomes a permanent [`WorldData`] record and would be
saved verbatim.

Rejected for large worlds: storing every procedural tree/rock across an infinite
or multi-kilometer world does not scale; duplicates baseline data already
derivable from seed + catalog.

## Option B — Reproducible baseline only

Candidates are never materialized; worlds always regenerate from seed at query time.

Rejected: runtime systems (queries, harvesting, rendering promotion) need
addressable instances with stable [`DoodadId`]. Pure regeneration cannot represent
gameplay overrides or simulation state.

## Option C — Hybrid (selected)

| Layer | Role |
|-------|------|
| Generation (ADR-018) | Reproducible procedural **baseline** candidates |
| Materialization (ADR-019) | Explicit conversion to **runtime instances** in [`WorldData`] |
| Future persistence | **Overrides/deltas** only; baseline recomputable from seed |

Materialized procedural doodads are runtime instances, not the authoritative
long-term store of baseline content. The baseline remains
`world_seed + chunk + catalog → candidates`. [`WorldData`] holds what is
currently materialized for active simulation. Future persistence stores deltas
(harvested, destroyed, moved, authored additions) rather than every generated
doodad.

This aligns with ARCHITECTURE scalability ("existing but not fully simulated")
and the Persistence Rule.

# Decision

## Explicit materialization API

`src/world/doodad/materialization/` provides:

```text
generate_chunk_doodads(...)
  → filter_candidates_by_exclusion_zones (optional)
  → filter_candidates_by_terrain (optional)
  → finalize_placements (optional snap)
  → materialize finalized placements
  → DoodadMaterializationReport
```

[`materialize_candidates`] uses [`MaterializationOptions::procedural_default`]
(full exclusion, terrain validation, snap). [`MaterializationOptions::raw`]
preserves snap-only behavior for tests and custom pipelines.

Callable on demand — **not** an automatic runtime system. No terrain-runtime
involvement (ADR-010).

## Authoring path for insertion

[`materialize_candidates`] uses [`create_doodad`] (ADR-017) for each accepted
candidate, preserving:

- `definition_id`
- `source` (including `Procedural { seed }`)
- `position`, `rotation`, `scale`

[`DoodadId`] allocation and id→chunk index synchronization remain centralized
in [`WorldData`] / authoring — materialization does not bypass them.

## Duplicate prevention

[`ProceduralDoodadKey`] = `(chunk, definition_id, procedural_seed)`.

[`WorldData`] maintains `HashMap<ProceduralDoodadKey, DoodadId>` for O(1)
idempotent materialization: rematerializing the same candidate set skips duplicates
without scanning chunk stores.

[`DoodadId`] is **not** used for duplicate detection (allocated after insert).

Keys are registered on successful materialization, removed on procedural record
delete, and **re-keyed on relocate** when [`WorldData::relocate_doodad`] moves a
[`DoodadSource::Procedural`] instance (old chunk key removed, new chunk key inserted
for the same [`DoodadId`]).

## Materialization report

[`DoodadMaterializationReport`] tracks pipeline and insert-stage counters including:

- `candidates_received`, `inserted`, `excluded_by_zone`
- terrain skips: `skipped_terrain_unavailable`, `skipped_height_constraint`,
  `skipped_slope_constraint`, `skipped_slope_unavailable`
- finalization: `placements_finalized`, `terrain_snaps_applied`
- insert skips: `skipped_duplicate`, `skipped_invalid_definition`,
  `skipped_disabled_definition`, `skipped_validation_failed`

[`DoodadMaterializationReport::skipped_at_insert`] counts insert-loop skips only.
[`skipped_total`] includes all pipeline filter skips plus insert skips.

## Materialization options

[`MaterializationOptions`]:

| Preset | `apply_exclusion_zones` | `validate_terrain` | `snap_to_terrain` |
|--------|-------------------------|--------------------|-------------------|
| `procedural_default()` / `Default` | true | true | true |
| `raw()` | false | false | true |

Implemented in Phases 3F–3H (ADR-020–022); not reserved.

# Future integration

## Persistence

Save files store overrides/deltas (authored placements, harvested/destroyed
procedural instances, moves) keyed by [`ProceduralDoodadKey`] and/or [`DoodadId`].
Baseline procedural content is regenerated from world seed when chunks load;
materialization replays only where no override suppresses or replaces baseline.

## Streaming

A future doodad streaming layer may: generate candidates for entering chunks,
materialize on demand, evict instances when leaving simulation radius — using
the same explicit API and duplicate index.

## Exclusion and terrain

Exclusion zones (ADR-015) and terrain validation attach to generation or
pre-materialization filters; materialization API shape accepts options for this.

# Rationale

Option C keeps baseline reproducible while giving runtime systems stable instance
ids. The procedural key index makes rematerialization idempotent at scale.
Authoring reuse avoids divergent insert semantics.

# Consequences

Benefits:

- Clear seam: generate → materialize → simulate/render
- Idempotent chunk materialization
- Persistence-friendly override model

Costs:

- Two representations (candidates vs records) during transition
- Procedural key tied to instance chunk; relocate updates the index (ADR-019 addendum)

# Alternatives Considered

See Option A and Option B above.

# Notes

- Cross-references: ADR-015, ADR-016, ADR-017, ADR-018, ADR-010, ARCHITECTURE
  Persistence Rule, ROADMAP Phase 3–7.

[`WorldData`]: ../src/world/data.rs
[`DoodadRecord`]: ../src/world/doodad/record.rs
[`DoodadSpawnCandidate`]: ../src/world/doodad/generation/candidate.rs
[`ProceduralDoodadKey`]: ../src/world/doodad/procedural_key.rs
[`DoodadMaterializationReport`]: ../src/world/doodad/materialization/report.rs
[`MaterializationOptions`]: ../src/world/doodad/materialization/options.rs
[`materialize_candidates`]: ../src/world/doodad/materialization/materialize.rs
[`create_doodad`]: ../src/world/doodad/authoring.rs
[`DoodadId`]: ../src/world/doodad/id.rs
