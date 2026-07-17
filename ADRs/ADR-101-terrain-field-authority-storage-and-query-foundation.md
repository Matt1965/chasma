# ADR-101: Terrain Field Authority, Storage, and Query Foundation (TF1)

## Status

Accepted — TF1 implemented.

## Context

Chasma needs a generic, CPU-authoritative representation of continuous environmental
and geological fields (water, iron, copper, stone, and future static/derived fields)
for later building requirements, overlays, and production efficiency (TF2–TF6).

Prior terrain work (`Heightfield`, `TerrainMask`, `BiomeMask`) does not model
normalized continuous potential fields at gameplay resolution.

## Decision

### Generic terrain field model

- Internal term: **TerrainField** (player UI may later say Terrain Overlay / Analysis).
- Stable string IDs (`TerrainFieldId`): `water`, `iron`, `copper`, `stone` in initial catalog.
- Values are **`u16` normalized potential**: `0` = minimum/absent, `65535` = maximum.
- Values represent **stable potential/suitability**, not finite reserves; base fields never deplete.
- Future depletion/irrigation uses a **separate modifier layer** composed after base sampling.

### Storage

- Authoritative store: `WorldData.terrain_fields: TerrainFieldStore`.
- Per-field `TerrainFieldLayer` keyed by `TerrainFieldId`.
- Per-chunk `TerrainFieldTile`: **33×33 shared-edge `u16` grid**, **8 m** sample spacing, **256 m** chunks.
- Row-major layout: column +X, row +Z; index = `row * samples_per_edge + col`.
- Static tiles live in the **world package** (`assets/worlds/<world>/terrain_fields/`), not saves/dev scenes.

### Catalog

- `TerrainFieldDefinition` + `TerrainFieldCatalog` (Bevy `Resource`).
- Production loads committed RON (`assets/terrain_fields/catalog.ron`).
- Dev/data-import loads Excel **Terrain Fields** sheet and exports RON.

### Queries

- Point: `sample_terrain_field_at` — fixed-point bilinear interpolation (fractions quantized 0–255, weights / 65536).
- Area: `sample_terrain_field_area` over **2 m occupancy-cell centers** in deterministic sorted order.
- Coverage reported as **`BasisPoints`** (10000 = 100%).
- Missing data returns explicit `FieldAvailability` — **never silent zero**.

### Dev Mode (TF1)

- Read-only **Fields** tab: catalog + store summary.
- Cursor probe with interpolation diagnostics; optional sparse gizmo markers.

## Consequences

- TF2 can import/generate tiles into the same store without API changes.
- TF3 can consume `TerrainFieldOverlayStyle` from definitions.
- TF4+ can use area reports for building placement and efficiency.
- GPU textures/shaders are not authoritative for gameplay queries.

## Non-goals (TF1)

Overlays, shaders, real production masks, generators, building requirements, efficiency,
build-mode previews, depletion, surveying, painting.

## Follow-up

- TF2 (ADR-102): source profiles, PNG import, deterministic generation, atomic packaging.
