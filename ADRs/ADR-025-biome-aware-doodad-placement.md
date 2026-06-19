# ADR-025: Biome-Aware Doodad Placement

# Status

Accepted (Phase R2 — biome filter foundation)

# Context

Phase R1 (ADR-024) established world-scale [`BiomeMask`] authority on
[`WorldData`]. Phase 3D–3H built procedural generation and a materialization
filter pipeline (ADR-018 through ADR-022):

```text
Generate → Exclusion → Terrain Validation → Finalization → Materialize
```

Procedural doodads must respect biome membership before instances are created.
Phase R2 connects biome authority to that pipeline without rendering, ECS, or
runtime streaming changes.

# Decision

## Pipeline order

```text
Generate Candidates
    → Biome Filter (new)
    → Exclusion Filter
    → Terrain Validation
    → Placement Finalization
    → Materialize → WorldData
```

Biome filtering runs **first** among materialization filters so disallowed
candidates are rejected before exclusion/terrain work.

## Catalog-owned permissions

[`DoodadDefinition`] gains `allowed_biomes: Vec<BiomeId>`. Materialization
reads the catalog — no hardcoded `if biome == Forest { tree }` logic.

A candidate is **accepted** when the sampled biome at its world position is
listed in its definition's `allowed_biomes`. Unmapped or out-of-bounds biomes
sample as [`BiomeId::Unassigned`] and fail unless explicitly allowed.

## Biome authority

Sampling uses [`WorldData::biome_at`] / [`BiomeMask`] only (ADR-024). No terrain
runtime, chunk residency, or render coupling.

When [`MaterializationOptions::apply_biome_filter`] is enabled and no mask is
loaded, **all** candidates are skipped with `skipped_biome_unavailable`. No
fallback to permissive behavior.

## Materialization options

| Preset | `apply_biome_filter` |
|--------|----------------------|
| [`MaterializationOptions::procedural_default()`] | `true` |
| [`MaterializationOptions::raw()`] | `false` |

## Reporting

[`DoodadMaterializationReport`] adds:

- `skipped_biome_disallowed`
- `skipped_biome_unavailable`

Both count toward [`DoodadMaterializationReport::skipped_total`].

## Starter catalog (Phase R2)

| Definition | `allowed_biomes` |
|------------|------------------|
| `tree_oak`, `tree_dead` | Forest only |
| All other starter definitions | All assigned biomes (Desert, Forest, Marsh, Plains) |

# Rationale

Catalog-driven permissions scale to new biomes and definitions without code
changes. Filtering before exclusion/terrain avoids wasted work on candidates
that will never materialize. Strict unavailable-mask behavior prevents silent
procgen without authored biome data.

# Consequences

Benefits:

- Deterministic biome-gated materialization
- Clear report counters for tuning
- Foundation for resource/creature rules using the same mask

Costs:

- Production procedural materialization requires a loaded biome mask
- Definitions must maintain `allowed_biomes` explicitly

# Alternatives Considered

## Biome tags string matching (existing reserved field)

Deferred: `biome_tags` remain reserved; typed [`BiomeId`] permissions are
authoritative for Phase R2.

## Filter after terrain validation

Rejected: wastes terrain reads on biome-rejected candidates.

## Permissive fallback when mask missing

Rejected: would materialize without biome authority and hide configuration errors.

# Notes

- **Future resource generation:** resource spawn rules will sample the same
  [`BiomeMask`] and use catalog-style allow lists.
- **Future creature spawning:** same pattern — biome at position vs definition
  permissions; no shared gameplay system in R2.
- Cross-references: ADR-016, ADR-018, ADR-019, ADR-020, ADR-021, ADR-022,
  ADR-024.

[`WorldData`]: ../src/world/data.rs
[`BiomeMask`]: ../src/world/biome/mask.rs
[`BiomeId`]: ../src/world/biome/id.rs
[`DoodadDefinition`]: ../src/world/doodad/catalog/definition.rs
[`MaterializationOptions`]: ../src/world/doodad/materialization/options.rs
[`DoodadMaterializationReport`]: ../src/world/doodad/materialization/report.rs
