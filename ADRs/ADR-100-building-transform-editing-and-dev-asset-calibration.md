# ADR-100: Building Transform Editing and Dev Asset Calibration (DT4)

## Status

Accepted — 2026-07-15

## Context

DT1–DT3 established metric asset sizing, doodad authoritative transforms, ellipse collision, and dev gizmos. Buildings still used position-only `move_building` with no instance scale, no occupancy-aware transform transaction, and gizmo preview without commit.

## Decision

### Authoritative building placement

`BuildingPlacement` stores:

- `position: WorldPosition`
- `rotation: Quat` (navigable: 90° quantized yaw via validation)
- `uniform_scale: FixedScale` (dev instance scale; default `1.0`)

### Safety class policy

Reuse `BuildingTransformSafetyClass` from DT1:

- **Navigable** — translate XYZ, yaw only, uniform scale; reject when units occupy building spaces or active task/reservation dependencies exist.
- **DecorativeNonNavigable** — full XYZ rotation, uniform scale; must not have interior profile.

No non-uniform building scale. No Force Move while occupied.

### Authoritative API

`update_building_transform(world, catalogs, building_id, candidate, options)` applies atomically:

1. Guards (occupancy, tasks, capabilities, scale bounds)
2. Placement + occupancy registration plan
3. In-place space/portal topology update (preserve IDs)
4. Interior child delta/profile reposition
5. Rollback on any failure

### Gizmos and scene persistence

- Building gizmo commit enabled via DT3 pipeline (preview → `update_building_transform` on release).
- Scene format v9 adds `uniform_scale_milli` on building records (legacy scenes default to `1000`).

### Asset calibration

Dev panel remains read-first; `export_calibration_csv` provides workbook-oriented CSV export. Excel is not rewritten automatically.

## Consequences

- Player Build Mode unchanged (definition baseline scale + placement validation only).
- Semantic gameplay values (HP, costs, build time, inventory) do not scale with instance uniform scale.
- Navigable building pitch/roll prohibited; continuous yaw not enabled until dependency proof exists.

## Related

- ADR-097 Metric Asset Sizing
- ADR-098 Doodad Transform Editing
- ADR-099 Dev Transform Gizmos
- ADR-096 Building Placement
