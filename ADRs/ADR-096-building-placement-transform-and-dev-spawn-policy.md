# ADR-096: Building Placement Transform, Freeform Anchoring, and Dev Spawn Policy (BP-CLEANUP)

# Status

Accepted (BP-CLEANUP)

# Context

Player build mode snapped anchors to 2 m occupancy-cell boundaries, producing a rigid
“board game” feel. Ghost GLB models, footprint gizmos, and committed buildings could
diverge because placement math was duplicated and render roots ignored optional model
offsets. Dev F12 spawns needed an explicit Complete policy with construction progress
and interior activation decoupled from spawn success.

# Decision

## Canonical anchor

- Authoritative anchor: continuous world XZ (fine-quantized at **0.1 m**), grounded Y.
- Footprint rasterization derives occupancy cells from the anchor; cells are not the anchor.
- Rectangle/Circle: footprint centered on anchor. Baked masks: `local_origin` relative to anchor.
- Optional `BuildingDefinition::model_local_offset` and `model_yaw_correction_degrees` for GLB pivot correction.

## BuildingPlacementPlan

`build_building_placement_plan` produces grounded anchor, cells, rotation, and validation.
Player ghost footprint overlay and commit path share this plan.

## Player vs Dev spawn

| Path | Lifecycle | Construction |
|------|-----------|--------------|
| `place_player_building` | Planned | Vulnerable HP, progress 0 |
| `create_dev_complete_building` | Complete | Full HP, progress 1.0 |

Dev interior activation is best-effort after spawn; failures do not roll back the building.

## Render transforms

`building_model_render_transform` applies anchor + model offset + yaw correction + terrain
vertical scale. Used by runtime sync, build-mode ghost GLB, and diagnostic fallbacks (anchor only).

# Consequences

- Placement feels continuous while occupancy remains deterministic.
- Preview and commit share one cell set.
- Asset pivot fixes are data-driven, not per-building runtime hacks.
