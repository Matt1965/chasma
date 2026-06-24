# ADR-053: Water Rendering Foundation (E11)

# Status

Accepted (E11 — visual water planes)

# Context

ADR-026 established the Environment rendering layer for skybox, ambient fill, and
directional light. ADR-052 added visual time-of-day. Large-world preview needs visible
water surfaces without introducing gameplay water simulation, terrain carving, or
WorldData authority changes.

# Decision

## Visual-only scope

Water in E11 is **client-local presentation**:

- Flat horizontal planes at a configured Y level
- Semi-transparent lit [`StandardMaterial`]
- No swimming, buoyancy, boats, navigation costs, or shoreline generation

[`WorldData`] remains heightfield authority. Water is not terrain and not simulation truth.

## Environment ownership

Module layout:

```text
src/environment/water/
    mod.rs
    plugin.rs       — WaterPlugin
    settings.rs     — WaterSettings resource
    material.rs     — StandardMaterial builder
    spawn.rs        — singleton plane spawn + sync
```

[`WaterPlugin`] registers under [`EnvironmentPlugin`]. One [`EnvironmentWaterPlane`]
entity at most; despawned when [`WaterSettings::enabled`] is false.

## WaterSettings

Tunable fields: `enabled`, `water_level`, `plane_size_meters`, `color`, `alpha`,
`roughness`, `metallic`, `wave_speed`, `wave_scale` (wave fields reserved for future
shaders; E11 does not animate).

## Placement

When [`WorldData::extent`] is set, plane width/depth and center derive from authored
chunk bounds and [`WorldConfig`] layout. Otherwise fallback to `plane_size_meters`
centered at `(size/2, water_level, size/2)` with a dev warning.

## Lighting

Water uses lit [`StandardMaterial`] with alpha blending so existing Environment
lighting and time-of-day changes affect the surface naturally. No explicit
[`TimeOfDaySettings`] coupling.

## Dev controls (feature `dev`)

| Input | Action |
|-------|--------|
| Shift+W | Toggle water enabled |
| Shift+PageUp/Down | Adjust water level |
| Shift+=/- | Adjust alpha |

Startup logs (once): enabled, level, plane size, entity count.

# Future hooks

- Multiple bodies / rivers / ocean tiles
- Custom water shaders, waves using `wave_speed` / `wave_scale`
- Reflections, refraction, shoreline meshing
- Gameplay water depth queries (separate from presentation)
- Terrain-adjacent carving (terrain pipeline, not Environment)

# Non-goals (E11)

No water physics, reflection probes, procedural rivers, terrain modification, save/load.

# References

- ADR-026 Environment Rendering Layer
- ADR-052 Time of Day Visual Environment System
- E11 implementation: `src/environment/water/`
