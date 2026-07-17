# ADR-097: Metric Asset Sizing and Authoring Transform Foundations (DT1)

# Status

Accepted (DT1)

# Context

GLB-backed Units, Doodads, and Buildings lacked a shared metric sizing contract. Runtime
systems used ad-hoc render scales (for example `ROBOT_DEFAULT_RENDER_SCALE = 2.15`), producing
obviously mis-scaled assets. Future Dev transform editing (DT2–DT4) requires deterministic
authoritative rotation and scale types, transform capability policy, and offline GLB
source-bounds measurement without runtime geometry parsing.

# Decision

## Authoritative transform types (`world/authoring_transform`)

- **Position:** continue using existing `WorldPosition` (0.01 m Dev UI step later).
- **`QuantizedOrientation`:** signed `i32` millidegrees per axis, canonical range
  `(-180_000, 180_000]`, **YXZ** Euler order for `Quat` conversion.
- **`FixedScale` / `AuthoringScale`:** scale-milliunits (1000 = 1.0), valid range 50–20000
  (0.05–20.0).
- **`AuthoringTransform`:** position + orientation + scale (shared contract; not yet universal
  runtime record type).
- **`TransformCapabilities`:** typed policy per content kind (Doodad full XYZ; navigable
  building yaw + uniform scale only; units no Dev editing; etc.).

## Metric asset sizing (`world/asset_sizing`)

- **`AssetSizingDefinition`** embedded on `UnitDefinition`, `DoodadDefinition`,
  `BuildingDefinition` — sizing columns remain on each Excel sheet (no profile catalog).
- **Offline source-bounds** (`data_import/asset_sizing/bounds.rs`, `data-import` feature only):
  1. Explicit catalog dimensions
  2. Named `source_bounds_node`
  3. Default node `size_reference`
  4. Combined visible mesh bounds (collision/portal/helper nodes excluded)
  5. Structured failure
- **Baseline scale precedence:** explicit baseline OR desired-dimension calculation — never both.
  Quantized output uses `FixedScale` / `AuthoringScale`.
- **Unit/Building:** uniform baseline from one reference axis.
- **Doodad:** non-uniform when all three desired dimensions supplied; reference-axis uniform
  otherwise; ambiguous partial XYZ rejected.
- **Building topology safety:** navigable buildings reject/warn when visual resize diverges
  >25% from footprint without matching topology data.
- **Migration states:** `MetricConfigured`, `LegacyExplicitScale`, `MissingSizingData`.

## Presentation composition (`world/asset_sizing/composition.rs`)

Model transform order (child under anchor where applicable):

```
placement × instance rotation × definition rotation correction × baseline scale
  × future instance override × model-local offset
```

**Model-local offset is visual only** — does not alter collision, occupancy, anchors, or portals.

Runtime applies:

- Units: `unit_baseline_render_scale`
- Doodads: `doodad_final_render_scale` = baseline × existing placement instance scale
- Buildings: `building_model_child_local_transform` when model child is used

Gameplay collision and occupancy are **unchanged in DT1**.

## Import and dev reporting

- Excel columns parsed by header name (`data_import/asset_sizing/columns.rs`).
- `finalize_*_definition` resolves sizing at import; runtime does not reopen GLBs.
- Dev startup aggregates `AssetSizingReport` entries → `logs/asset_sizing_report.md`.
- Dev panel shows read-only sizing calibration for selected catalog definition.

## DT1 non-goals

No transform gizmos, numeric editors, doodad transform APIs, ellipse collision, building
instance Dev editing, player Build Mode changes, undo/redo, or runtime GLB measurement.

# Consequences

- Mis-scaled assets are corrected through catalog data, not filename-specific runtime hacks.
- DT2–DT4 have deterministic types, capability policy, and sizing seams ready.
- Visual/collision mismatch warnings exist for doodads until DT2 collision scaling.
- Production builds without `data-import` use legacy/missing sizing fallbacks without GLB parsing.

# Related

- ADR-096 — building placement anchor and model correction
- ADR-095 — building runtime asset integration
- Dev Transform Editing roadmap DT2–DT4 (editing UI and collision scaling)
