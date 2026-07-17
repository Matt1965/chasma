# ADR-103: Terrain Field Overlay Rendering and Terrain Analysis UI (TF3)

## Status

Accepted — TF3 implemented.

## Context

TF1/TF2 established CPU-authoritative `TerrainFieldStore` with packaged `u16` tiles.
TF3 adds the player-facing **Terrain Analysis** visualization without altering simulation truth.

## Decision

### Authority boundary

- Gameplay and cursor inspection use `sample_terrain_field_at` (CPU).
- GPU overlay meshes are presentation-only vertex colors derived from tiles.
- Overlay selection lives in client `TerrainOverlayState`, not `WorldData`.

### Player UI

- **Terrain Analysis** panel (toggle button + `O` shortcut).
- Field list from `TerrainFieldCatalog` (enabled definitions with `overlay_style.enabled`).
- One active field at a time; `None` disables overlay.
- Opacity in basis points (0–9000 = 90% cap); preserved across field switches when user-adjusted.
- Cursor readout in panel uses authoritative CPU samples.
- `TerrainOverlaySelection` reserves `temporary_override` for TF4 Build Mode.

### Rendering

- Per-resident-chunk overlay mesh (33×33 conforming grid, slight Y offset).
- `StandardMaterial` unlit + vertex color alpha blend (no custom shader in TF3).
- Colors from `TerrainFieldOverlayStyle::vertex_color_for_value`.
- Missing tiles: checker unknown pattern (distinct from true zero transparency).
- `request_revision` invalidates stale uploads on field switch.

### Streaming

- `sync_terrain_field_overlays` runs after `TerrainStreamingSystems`.
- Overlays spawn/despawn with resident terrain chunks.
- `cleanup_orphan_field_overlays` removes overlays when terrain unloads.

### Dev diagnostics

- `TerrainFieldOverlayDiagnostics` tracks uploads, cache hits, missing tiles.
- Dev Fields tab shows overlay revision stats via `sync_terrain_analysis_dev_diagnostics`.

## Consequences

- TF4 can add Build Mode temporary overlay override without API breakage.
- Custom WGSL/material extension remains optional future optimization.
- Production terrain rendering still requires `TerrainRenderAssets` (currently dev preview).

## Non-goals (TF3)

Building requirements, efficiency, build-mode auto-overlay, extraction, depletion, survey
knowledge, multi-field blend, field painting, minimap overlay.
