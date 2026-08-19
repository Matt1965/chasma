# ADR-026: Environment Rendering Layer

# Status

Accepted (R8 foundation, R9 tuning — prototype complete)

**Amended (SKY-1 / ENV-CLOUD-CHECKPOINT):** Static cubemap skybox loading (`skybox.rs`,
`merge_skybox_cubemap`, `assets/environment/skyboxes/`) is **removed**. Sky presentation is
**procedural only** via [`EnvironmentVisualState`] → [`EnvironmentSkyMaterial`] →
`assets/shaders/environment_sky.wgsl`, with volumetric clouds in ADR-131. Sections below that
describe cubemap skybox, [`SkyboxCamera`], and skybox settings are **historical** unless this
amendment restates them.

# Context

The dev preview and future production builds need a stable presentation backdrop:
sky color, ambient fill, and a primary directional light. These concerns are
client-local, renderer-facing, and independent of world simulation.

R8 introduced the **Environment** layer. R9 completes prototype tuning so terrain,
doodads, shadows, and sky render together without further renderer restructuring.

**Current (post-amendment):** procedural sky dome + volumetric cloud proxies + singleton lighting.

# Decision

## Environment owns presentation backdrop

Introduce the **Environment rendering layer** at `src/environment/`, registered by
[`EnvironmentPlugin`] in the `AppPlugin` composition root after
[`TerrainRuntimePlugin`] and before [`CameraPlugin`] (ADR-007).

| Layer | Owns |
|-------|------|
| **Environment** | [`EnvironmentSettings`], [`EnvironmentVisualState`], procedural sky, volumetric clouds, [`GlobalAmbientLight`], directional light |
| **Not in this layer** | [`WorldData`], terrain meshes, doodad instances, weather simulation |

Environment state is **not** stored in [`WorldData`] and is not tied to terrain streaming.
Terrain and doodads must **not** spawn lights.

## Module responsibilities

```text
src/environment/
    mod.rs              — public exports
    plugin.rs           — EnvironmentPlugin registration
    settings.rs         — EnvironmentSettings resource (lighting/shadow tuning)
    visual_state.rs     — derived sky/lighting evaluation (time-of-day input)
    procedural_sky.rs   — procedural sky dome spawn + sync
    procedural_clouds.rs — volumetric cloud render proxy (ADR-131)
    sky_material.rs     — procedural sky material + uniforms
    lighting.rs         — ambient + directional light setup
    debug.rs            — dev diagnostics + singleton validation
```

Gameplay systems must not spawn or tweak lights directly. Future presentation
systems modify [`EnvironmentSettings`] only.

## Tuning philosophy (R9)

All environment presentation values live in [`EnvironmentSettings`]:

| Field | Controls |
|-------|----------|
| `directional_light_illuminance`, `directional_light_color`, `directional_light_rotation` | Sun/moon |
| `directional_shadows_enabled` | Shadow casting |
| `ambient_brightness`, `ambient_color` | Global fill |
| Shadow cascade fields | RTS-scale directional shadows |

Sky color, sun disc, and twilight are evaluated in [`EnvironmentVisualState`] from
[`TimeOfDaySettings`] and art-directed [`SkyColorPalette`] — not cubemap assets.

~~`skybox_set`, `skybox_brightness`, `skybox_rotation`~~ — **removed** (historical cubemap path).

Default rotation derives from [`DEFAULT_DIRECTIONAL_LIGHT_POSITION`] looking at
[`DEFAULT_DIRECTIONAL_LIGHT_LOOK_AT`] — not hardcoded in lighting setup.

Avoid scattering magic numbers in terrain, doodads, or preview code. When a value
affects the whole scene, it belongs in [`EnvironmentSettings`].

## Singleton expectations

The environment layer maintains exactly:

- **One** [`DirectionalLight`] tagged [`EnvironmentDirectionalLight`]
- **One** [`GlobalAmbientLight`] resource (Bevy default; values from settings)
- **One** procedural sky dome ([`EnvironmentProceduralSky`]) and **one** cloud render proxy when enabled

~~[`SkyboxCamera`] on the RTS camera~~ — **removed** with static cubemap path.

[`EnvironmentLightingInitialized`] prevents duplicate light spawns on repeated
startup hooks. Dev builds run singleton validation in `PostStartup` via
[`count_environment_singletons`].

Shadows use Bevy directional-light defaults (no custom cascade tuning in R9).
Terrain preview uses a lit [`StandardMaterial`] so heightfield meshes receive
shadows; doodad glTF scenes use default PBR materials.

## Asset layout (current)

```text
assets/environment/
    project_defaults.ron   — authored time-of-day + manual lighting baseline
assets/shaders/
    environment_sky.wgsl   — procedural sky
    environment_cloud.wgsl — volumetric clouds (ADR-131)
```

## Historical asset layout (superseded)

~~`assets/environment/skyboxes/{set}/cubemap.*`~~ and `merge_skybox_cubemap` — removed.

## Runtime behavior

### Procedural sky (current)

- Spawn camera-centered sky dome at startup ([`setup_procedural_sky`]).
- Sync dome translation to RTS camera; uniforms from [`EnvironmentVisualState`] each frame.
- No cubemap load or [`Skybox`] component.

### Lighting

- One [`DirectionalLight`] spawned from [`EnvironmentSettings`].
- [`GlobalAmbientLight`] configured from the same resource.
- No time-of-day or weather logic.

### Dev diagnostics (R9)

When the `dev` feature is enabled, startup logs:

- `Environment initialized`
- Full **Environment Settings** report (directional, ambient)
- Singleton validation summary

Debug helpers in `debug.rs`: [`log_environment_configuration`],
[`count_environment_singletons`], [`validate_environment_singletons`].

## Future extension points

Extend [`EnvironmentSettings`] and companion systems in `src/environment/` —
**do not restructure the module** for:

- Weather (fog, precipitation)
- Day/night cycle (sun arc, ambient curves)
- HDR environments and image-based lighting
- Atmosphere (aerial perspective)
- Biome-tinted ambient overrides
- Water reflections and caustics

# Consequences

- **Positive:** Single tuning authority; prototype environment considered feature-complete.
- **Positive:** Terrain preview no longer uses unlit materials; shadows visible on terrain.
- **Positive:** Singleton guards prevent duplicate lights during development.
- **Neutral:** No weather, water, atmosphere, or post-processing in R9.

# References

- ADR-007 (composition root)
- ADR-014 (primary RTS camera)
- ADR-023 (runtime vs world-data split pattern)
- ADR-131 (volumetric clouds)
- ADR-052 (time-of-day visual environment)

[`WorldData`]: ../src/world/data.rs
[`EnvironmentPlugin`]: ../src/environment/plugin.rs
[`EnvironmentSettings`]: ../src/environment/settings.rs
[`EnvironmentDirectionalLight`]: ../src/environment/lighting.rs
[`EnvironmentLightingInitialized`]: ../src/environment/lighting.rs
[`DEFAULT_DIRECTIONAL_LIGHT_POSITION`]: ../src/environment/settings.rs
[`DEFAULT_DIRECTIONAL_LIGHT_LOOK_AT`]: ../src/environment/settings.rs
[`TerrainRuntimePlugin`]: ../src/terrain/mod.rs
[`CameraPlugin`]: ../src/camera/mod.rs
[`RtsCamera`]: ../src/camera/components.rs
[`count_environment_singletons`]: ../src/environment/debug.rs
[`log_environment_configuration`]: ../src/environment/debug.rs
[`validate_environment_singletons`]: ../src/environment/debug.rs
