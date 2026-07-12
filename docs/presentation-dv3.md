# Gameplay Presentation (DV3)

Presentation-only polish for movement feedback, health bars, selection rings, and lighting.

## Move destination validation

Before a `MoveTo` order is issued, formation targets pass through `resolve_move_destination`:

- Checks occupied unit footprints using per-definition `collision_radius_meters`
- If the click overlaps another unit, projects to the nearest valid XZ point outside combined radii
- Deterministic (occupants processed in `UnitId` order; coin-stack escapes use hashed angle)
- Works for single-unit and group moves (batch assignments resolve in sorted order)

Simulation movement, steering, and collision after arrival are unchanged.

## Health bar billboarding

Overhead health bars (`sync_unit_health_bars` + `billboard_unit_health_bars`):

- Each frame, bar rotation faces the active `RtsCamera`
- World-up locked to reduce flip artifacts on slopes
- Presentation only — no gameplay or vitals changes

## Terrain-conforming selection rings

Selection rings (`sync_unit_selection_indicators`):

- Annulus mesh rebuilt each frame with vertices sampled on terrain height
- 32 segments; inner/outer radii from `selection_ring_radius` (2× collision, min 0.9 m)
- Local to unit render root; slight lift to reduce z-fighting

## Shadow / close-camera lighting (DV3)

**Root cause:** Bevy’s default directional shadow cascades use `maximum_distance: 150` world units. RTS orbit distances (40–5000+) exceed that range, so near-camera terrain often fell outside stable cascade coverage—appearing as abrupt darkening when zoomed in.

**Fix:** Environment directional light now uses RTS-scaled `CascadeShadowConfig`:

| Setting | Default |
|---------|---------|
| `maximum_distance` | 2500 |
| `first_cascade_far_bound` | 120 |
| `minimum_distance` | 0.5 |
| `num_cascades` | 4 |
| `shadow_normal_bias` | 2.0 |

Tuned in `EnvironmentSettings`; applied at `setup_environment_lighting`.
