# ADR-106: Terrain Fields Persistence, Packaging, and Readiness

## Status

Accepted (TF6)

## Context

TF1–TF5 established authoritative terrain fields, world-package loading, overlays, building assessments, and terrain-driven output rates. TF6 closes the branch by hardening persistence boundaries, rebuild/invalidation paths, future modifier seams, and production readiness.

## Decision

### World package ownership

- Base `u16` field tiles live in `assets/worlds/<world_id>/terrain_fields/` (manifest + per-field chunk RON tiles).
- Runtime never depends on Excel or source images.
- `TERRAIN_FIELD_MANIFEST_VERSION` / `TERRAIN_FIELD_TILE_VERSION` gate load; unsupported versions reject clearly.

### Save / world boundary

| Persist in save (future) | Do not persist |
|--------------------------|----------------|
| `BuildingOperationSaveState` (fractional progress) | Base terrain field tiles |
| Sparse `TerrainFieldModifierStore` (future) | `BuildingTerrainAssessmentStore` |
| Building placements/IDs | GPU overlay textures |
| | Cursor/panel UI state |

Assessments rebuild from buildings + catalogs + field tiles via `rebuild_all_building_terrain_assessments`.

### Load order

1. World config / manifest extent
2. Field definition catalogs
3. World-package manifest + tiles (`bootstrap_world_terrain_fields`)
4. Authoritative world records
5. Future sparse modifiers
6. Assessment rebuild (on reload or explicit dev action)
7. Presentation overlays

### Assessment rebuild

- `rebuild_all_building_terrain_assessments` — single entry point, deterministic building order, per-building failures isolated.
- `ensure_building_terrain_assessment` compares `BuildingTerrainAssessmentKey` before trusting cache.
- `invalidate_buildings_for_changed_fields` + `TerrainFieldPackageDiff` for selective invalidation on package reload.
- `reload_terrain_fields_with_invalidation` — dev/production reload seam.

### Modifier and override seams

- `TerrainFieldModifierStore` on `WorldData` (empty by default).
- `compose_terrain_field_value(base, field, chunk, modifiers)` in query path only.
- Authored override package sections reserved; not implemented in TF6.

### Operation state

- `BuildingOperationSaveState` serializes progress by building raw id.
- Progress restores exactly; efficiency recomputed on subsequent ticks.

### Package format

- **Keep per-chunk RON** at current scale (8 km world, 4 fields).
- Revisit when tile count or load time exceeds measured thresholds documented in readiness report.

## Consequences

- Production builds load packaged fields via `bootstrap_terrain_fields_on_startup`.
- Dev **R** reload diffs package, invalidates affected buildings, rebuilds assessments; **Shift+A** rebuilds assessments only.
- Tile `tile_revision` increments on replace for overlay/assessment revision tracking.
- Full field painting, depletion, and recipe output remain deferred.
