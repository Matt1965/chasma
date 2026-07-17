# Terrain Fields Final Readiness Review

**Date:** July 2026  
**Branch:** TF1–TF6 (ADR-101 through ADR-106)  
**Recommendation:** **Ready with non-blocking caveats**

---

## Scope

This review covers the complete Terrain Fields branch: authoritative CPU field storage, world-package loading, player overlays, building terrain assessments, terrain-driven operational efficiency, and TF6 persistence/readiness hardening.

Out of scope (explicitly deferred): harvesting economy, recipes, depletion, field painting, multi-field overlay blending, settlement AI, power systems.

---

## Architecture Summary

| Layer | Owner | Notes |
|-------|-------|-------|
| Definitions | `TerrainFieldCatalog` + RON | Production load from `assets/terrain_fields/catalog.ron` |
| Source profiles | `TerrainFieldSourceProfileCatalog` | Dev/data-import only at bake time |
| Base tiles | `TerrainFieldStore` on `WorldData` | World package, not saves |
| Modifiers | `TerrainFieldModifierStore` on `WorldData` | Empty seam (TF6) |
| Queries | `sample_terrain_field_at` / `sample_terrain_field_area` | Single CPU authority |
| Assessments | `BuildingTerrainAssessmentStore` | Derived, rebuildable |
| Operations | `BuildingOperationStore` | Serializable via `BuildingOperationSaveState` |
| Overlay | `TerrainFieldOverlayPlugin` | Presentation only |

---

## Pre-Cleanup Audit Findings (TF1–TF5)

1. **Assessment cache** did not compare `BuildingTerrainAssessmentKey` — fixed in `ensure.rs` (TF6).
2. **`mark_all_dirty` unwired** on field reload — fixed via `reload_terrain_fields_with_invalidation`.
3. **`tile_revision` stuck at 1** on replace — fixed in `TerrainFieldLayer::replace_tile`.
4. **Production manifest load** only in dev preview — fixed via `bootstrap_terrain_fields_on_startup`.
5. **World package incomplete** — manifest lists 4 fields; only `water/0_0.ron` committed (non-blocking for dev; build with **B**/**Shift+B**).
6. **Duplicate bilinear debug** in dev probe — retained for diagnostics; query authority unchanged.
7. **`OperationalEfficiencyError::TerrainAssessmentStale`** unused — ensure path now validates keys.

---

## World Package and Persistence

**Format:** RON manifest + per-chunk tiles per field directory.

```
assets/worlds/main/terrain_fields/
  manifest.ron
  water/<x>_<z>.ron
  iron/ ...
```

**Versions:** `TERRAIN_FIELD_MANIFEST_VERSION = 1`, `TERRAIN_FIELD_TILE_VERSION = 1`.

**Save boundary:**
- Serialize: `BuildingOperationSaveState` (progress by building raw id)
- Do not serialize: base tiles, assessments, overlay GPU state
- Rebuild assessments after load via `rebuild_all_building_terrain_assessments`

**Load:** `bootstrap_world_terrain_fields` / startup system; dev synthetic fallback when package missing.

---

## Field Definitions and Data Sources

- Water/Iron/Copper/Stone definitions in catalog + source profiles.
- Dev **B** / **Shift+B** builds from profiles to world package atomically.
- Dev **R** reloads with diff + selective invalidation + assessment rebuild.
- Dev **Shift+A** rebuilds all building assessments.

---

## Query Correctness

All gameplay paths use `sample_terrain_field_at` / `assess_building_terrain`:
- Terrain Analysis cursor
- Build Mode preview
- Placement commit assessment
- Selected building panel
- Operational efficiency query
- Dev probes

Modifiers compose in query path only; bilinear interpolation returns base tile values.

---

## Overlay Rendering

- CPU authoritative; vertex-color chunk meshes.
- Invalidates on `tile_revision` and overlay `request_revision`.
- Base terrain renders independently if overlay fails.

---

## Building Sampling and Assessments

- `rebuild_all_building_terrain_assessments` — deterministic, per-building error isolation.
- `invalidate_buildings_for_changed_fields` — selective invalidation from package diff.
- Transform edits mark assessment dirty (dev gizmo).
- Poor terrain never blocks placement.

---

## Operational Efficiency

- Terrain scales output rate only (ADR-105).
- Fixed-point progress with `BuildingOperationSaveState` for save seam.
- Blocked workstations use `TaskState::BlockedWaiting`.

---

## Determinism

Verified by tests: package diff, rebuild ordering, compose identity, operation save round-trip, query repeat, progress tick parity (TF5).

---

## Performance

| Test | Result |
|------|--------|
| 1M point queries | `#[ignore]` stress benchmark available |
| Query repeat | <1ms (unit test) |
| 32×32 chunk constant field bootstrap | Used in stress fixture |

Full 1000-building assessment stress deferred to profiling session; architecture supports localized rebuild.

---

## Memory and Disk Usage

| Item | Estimate |
|------|----------|
| CPU per tile | ~2.2 KB (33×33 u16) |
| 4 fields × 32×32 chunks | ~9 MB CPU |
| 16 km world (64×64) | ~36 MB CPU per field layer |
| Package (RON) | ~50–100 KB per tile file; acceptable at current scale |

**Package format decision:** Keep RON. Revisit at >10k tiles or >2s cold load.

---

## Error Handling

Typed errors across TF modules preserved. Missing tiles fail soft (`FieldAvailability::TileMissing`). Package version mismatch fails hard on load.

---

## Dev Tools

Fields tab: probe, build, validate, reload (with diff), reassess, gizmos, package summary.

---

## Known Limitations

1. Only water fixture tile committed; iron/copper/stone require dev build.
2. Authored override layer not implemented (seam documented).
3. Full save-game integration for operation progress not wired to scene format yet (`BuildingOperationSaveState` ready).
4. Client overlay preferences not persisted to disk (recommended local settings).
5. Catalog hot-reload revision bumps not fully wired for live play.

---

## Deferred Features

- Field painting / authored override authoring
- Runtime depletion / irrigation modifiers
- Recipe/item output from `completion_count`
- Multi-field overlay blending
- Faction field knowledge

---

## Technical Debt

- Consolidate `collect_tile_revisions` with area sampling pass
- Wire `OperationalEfficiencyError::TerrainAssessmentStale` or remove
- Production RON loaders for response/requirement catalogs (starters still used in dev)
- Complete committed world package for all four fields

---

## Final Recommendation

**Ready with non-blocking caveats.**

The branch is architecturally complete for continued feature work (extraction, settlement systems). Before shipping field-dependent gameplay broadly:

1. Run dev **Shift+B** to bake and commit all four field packages.
2. Run shared-edge validation across baked tiles.
3. Wire `BuildingOperationSaveState` into the scene/save format when saves land.

No TF6-specific blockers remain for downstream systems that consume terrain assessments and operational efficiency.
