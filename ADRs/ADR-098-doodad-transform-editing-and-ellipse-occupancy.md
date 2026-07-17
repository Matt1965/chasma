# ADR-098: Doodad Transform Editing and Ellipse Occupancy (DT2)

# Status

Accepted (DT2)

# Context

DT1 established metric asset sizing, `QuantizedOrientation`, `AuthoringScale`, transform
capability policy, and presentation composition. Doodad collision still used circle-only
footprints that ignored instance scale and non-uniform sizing.

DT2 adds authoritative doodad transform editing in F12 Dev Mode, true rotated ellipse
occupancy under non-uniform X/Z scaling, and scene persistence for quantized transforms.

# Decision

## Authoritative doodad placement (`world/doodad/placement/pose.rs`)

`DoodadPlacement` stores:

- `WorldPosition` translation
- `QuantizedOrientation` (full XYZ rotation, YXZ Euler for `Quat`)
- `AuthoringScale` (non-uniform XYZ permitted when definition allows)

Helpers: `rotation_quat()`, `scale_vec3()`, `yaw_radians()`, `collision_yaw_radians()`,
`collision_scale_xz()`. No parallel ECS `Transform` truth.

## Transform edit API (`world/doodad/transform_edit.rs`)

Single transaction entry point:

`update_doodad_transform(world, catalog, doodad_id, candidate, options) -> Result<TransformEditReport, TransformEditError>`

Options: `allow_overlap`, `follow_ground`, `bypass_placement_validation` (dev-only).

Flow: validate capabilities → validate translation/scale → plan occupancy → mutate world
record (including cross-chunk relocate) → apply occupancy → report. World mutation rolls back
if occupancy application fails.

`move_doodad` delegates to this API when occupancy catalogs are supplied.

## Collision projection policy

- Ground collision uses **yaw only**; pitch/roll are visual.
- Instance **X/Z scale** affects horizontal collision; Y scale is visual-only for ground blocking.
- `resolve_doodad_collision` maps definition `DoodadCollisionShape` + baseline scale + instance
  scale to `FootprintShape` (Circle, Ellipse, Rectangle, BakedFootprint).
- Circle with unequal effective X/Z radii becomes `FootprintShape::Ellipse`.
- `tilted_blocker_projection_warning` diagnostic when pitch/roll exceed threshold — does not
  auto-disable collision.

## Ellipse occupancy (`world/occupancy/ellipse.rs`)

- `ellipse_contains_point`, `circle_overlaps_rotated_ellipse` (closest-point on axis-aligned
  ellipse in local space), `ellipse_overlaps_cell`, `cells_for_rotated_ellipse`.
- Doodads use **continuous yaw** via `occupied_cells_for_footprint_yaw`; buildings keep
  `QuantizedRotation` (90° steps).
- Rectangle and baked footprints scale with instance X/Z and yaw through the same continuous
  planner.

## Overlap policy

`DoodadRegistrationOptions::allow_overlap` skips doodad-vs-doodad grid conflicts during dev
edits. Geometric passability still considers all blockers. Removing one overlapping blocker
does not unblock cells registered by another.

## Dev inspector (DT2 scope)

- Pick doodads via render entity ray-sphere (scaled radius fallback).
- Inspector panel shows position, rotation, scale, visual size, collision shape, cell count,
  tilt warning.
- Keyboard hotkeys: arrows/PageUp/Down move, `[`/`]` yaw, hold G follow ground, hold O allow
  overlap.
- Full numeric fields with increment/decrement buttons deferred to DT3 (gizmo + richer UI).

## Scene format v8

`SceneDoodadRecord` adds canonical fields:

- `orientation_*_mdeg` (millidegrees)
- `scale_*_milli` (milliunits, `1000` = 1.0)

v7 files load through legacy `rotation` + `scale` when `scale_x_milli == 0`. v8 saves write
quantized fields; occupancy rebuilt from restored transform on load.

## DT2 non-goals

No on-screen gizmos, building/unit transform editing, player Build Mode changes, generalized
undo history, runtime mesh collision, or ECS-as-truth dev mutation.

# Consequences

- Edited doodad transforms survive save/load and chunk streaming.
- Non-uniform scale produces true ellipses for movement, placement validation, and occupancy.
- DT3 can add gizmos and full numeric controls on the same `update_doodad_transform` seam.

DT3 gizmos implemented in ADR-099; building commit remains DT4.

# Related

- ADR-097 — authoring transform types and metric sizing (DT1)
- ADR-048 — world inspector foundations
- ADR-045 — dev scene format
- Roadmap DT3 — gizmos and full numeric inspector
