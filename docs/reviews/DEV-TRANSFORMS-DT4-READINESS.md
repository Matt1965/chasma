# Dev Transform Editing Readiness

Assessment date: 2026-07-15 (DT4 implementation pass)

## Architecture Compliance

- Authoritative transforms live on `WorldData` records, not ECS.
- `update_building_transform` mirrors doodad transaction pattern with rollback.
- Building safety class enforced at validation time, not inferred from lifecycle.

## Metric Asset Sizing

- `FixedScale` on building instances; definition `asset_sizing` baseline unchanged.
- `building_model_child_scale` composes baseline × instance uniform scale.

## Doodad Editing

- Unchanged from DT2; gizmo + numeric paths remain authoritative.

## Ellipse Collision

- Unchanged from DT2.

## Gizmos

- Building translate/rotate/scale commit enabled with capability filtering from definition safety class.
- Preview uses shared doodad preview bridge for drag math.

## Building Editing

- Position, 90° yaw, uniform scale (when `allow_instance_scale`).
- Occupied-space and active-task guards implemented.
- Optional `cancel_dependencies` on commit (dev key C during gizmo release).
- Interior spaces/portals updated in-place; children repositioned.

## Persistence

- Scene v9: `uniform_scale_milli` on buildings.
- v8 and earlier migrate to scale 1.0 via serde default.

## Input and UI

- Gizmo W/E/R for buildings; scale enabled.
- Asset sizing panel + CSV export helper.
- Full numeric building inspector panel deferred (gizmo + API complete).

## Performance

- No evidence-backed hotspots addressed in this pass; transform uses existing occupancy planning.

## Asset Calibration Results

- CSV export path added; per-asset workbook migration remains manual.
- Starter hut enables `allow_instance_scale` for dev validation.

## Known Limitations

- No non-uniform building scale.
- No unit transform editing.
- No player Build Mode transform editor.
- Navigable buildings: pitch/roll prohibited; 90° yaw only.
- Movement while occupied rejected (no Force Move).
- Semantic gameplay values do not scale with instance scale.
- Calibration does not rewrite Excel automatically.
- Decorative building safety validation is definition-time; runtime decorative instances without full test matrix.

## Deferred Player Build Mode Seam

Player Build Mode may later reuse definition baseline scale, grounded XZ placement, quantized yaw, and placement validation. It must not expose instance-scale editing, arbitrary rotation, or unsafe occupied moves by default.

## Recommendation

**Ready with non-blocking caveats**

Core DT4 authoritative building transforms, gizmo commit, guards, persistence, and calibration export are in place. Remaining work: expanded integration/stress tests, numeric inspector panel polish, and production asset calibration pass on Robot/Fox/Chest via workbook import.
