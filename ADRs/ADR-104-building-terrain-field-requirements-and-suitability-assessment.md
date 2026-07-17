# ADR-104: Building Terrain-Field Requirements and Suitability Assessment

## Status

Accepted (TF4)

## Context

TF1–TF3 established authoritative terrain fields, import/generation, and player overlay inspection. Buildings that depend on local terrain quality (mines, farms, wells) need a deterministic way to evaluate fields beneath operational footprints, preview suitability in Build Mode, and cache assessments for placed instances—without blocking placement or producing items yet.

## Decision

### Response profiles

- Reusable `FieldResponseProfileDefinition` curves map field `u16` values to `EfficiencyBasisPoints` (`10000` = 100%, max `30000` = 300%).
- Piecewise-linear integer interpolation; below/above endpoints clamp.
- Profiles live in `FieldResponseProfileCatalog` (committed RON + starter fallback).

### Building requirements

- `BuildingFieldRequirementDefinition` rows reference building id, terrain field id, response profile, minimum average, usable threshold, minimum coverage, optional sampling footprint, and primary overlay flag.
- `BuildingFieldRequirementCatalog` provides deterministic per-building queries and primary overlay resolution.
- Required efficiencies combine with **minimum** (weakest field wins).

### Operational sampling

- `resolve_building_field_sample_cells` resolves footprint in order: requirement footprint → building default sampling footprint → placement footprint.
- Sampling uses existing 2 m occupancy cell centers and `sample_terrain_field_area`.
- Same planner drives Build Mode preview, placement commit, and placed-building reassessment.

### Assessment

- `BuildingTerrainAssessment` and per-requirement rows report average, coverage, response efficiency, `can_operate`, warnings, and tile revision context.
- `BuildingTerrainAssessmentStore` caches assessments by `BuildingId` with revision keys; poor terrain never rejects placement.

### Build Mode UX

- Ghost status line shows field averages, coverage, expected output, and operational status separately from placement validity colors.
- Selecting a field-dependent building sets `TerrainOverlayState.selection.temporary_override` without overwriting manual overlay; cleared on exit/cancel.

### TF5 seam

- `terrain_efficiency_basis_points` is computed and cached but does not scale production output until TF5.

## Consequences

- Catalog authors maintain profiles and requirements in Excel/RON instead of per-building columns.
- Transform edits and tile reloads must invalidate cached assessments (store `mark_dirty` seam).
- UI must distinguish unknown field data from zero quality.
