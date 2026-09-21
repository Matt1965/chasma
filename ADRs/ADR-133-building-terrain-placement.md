# ADR-133: Building Terrain Placement Modes

## Status

Accepted

## Context

Building placement previously grounded only the anchor XZ point and kept all buildings level,
causing large footprints (especially farms) to intersect sloped terrain. Terrain-field
assessment (ADR-104) evaluates operational suitability but does not adjust placement geometry.

## Decision

### Terrain placement mode (authored per building)

```text
TerrainPlacementMode::LevelFoundation   — houses, workshops, chests, wells, …
TerrainPlacementMode::ConformToTerrain  — farms, quarries, mines, …
```

Stored on `BuildingDefinition.terrain_placement_mode`. No building-id conditionals.

### Canonical footprint

All geometric sampling uses `effective_building_footprint_for_placement` (ADR-080/096).
Occupancy remains horizontal and yaw-quantized; placement rotation may include pitch/roll
for conforming buildings.

### Single resolver

`resolve_building_placement` samples authoritative heightfield vertices inside the rotated
footprint (spacing ≤ half heightfield sample spacing), fits a plane for conforming buildings,
and returns:

- authoritative anchor `WorldPosition` (simulation Y)
- full `Quat` rotation (yaw preserved for conforming mode)
- optional derived `FoundationSkirtSpec` (level mode only)

Preview and commit both call the resolver from the same candidate XZ + yaw; commit revalidates.

### LevelFoundation

- Yaw only; building level.
- Floor Y = max terrain under footprint + clearance epsilon.
- Reject when foundation depth (floor − min terrain) exceeds 2 m (default).
- Derived foundation skirt mesh fills perimeter gaps (presentation only).

### ConformToTerrain

- Plane orientation from least-squares fit; yaw preserved.
- Vertical offset lifts rigid base so no sample protrudes through (clearance envelope).
- Reject when plane RMS > 0.35 m or peak residual > 0.75 m (defaults).

### Simulation vs render height

Authoritative placement Y lives in simulation/world space. Presentation applies existing
`render_height` / `render_height_above_base` so buildings and skirts align with rendered
terrain vertical scale. No parallel terrain truth.

### Navigation

No navigation blueprint or editor changes. Level buildings with blueprints remain level.
Current conforming buildings have no interior navigation blueprints.

## Consequences

- Farms and quarries tilt with hillsides without terrain clipping when placement is valid.
- Houses gain derived foundations on slopes within tolerance.
- Extreme terrain rejects placement via existing build-mode invalid UX.
