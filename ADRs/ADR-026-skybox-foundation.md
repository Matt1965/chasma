# ADR-026: Environment Rendering Layer

# Status

Accepted (R8 foundation, R9 tuning — prototype complete)

# Context

The dev preview and future production builds need a stable presentation backdrop:
sky color, ambient fill, and a primary directional light. These concerns are
client-local, renderer-facing, and independent of world simulation.

R8 introduced the **Environment** layer. R9 completes prototype tuning so terrain,
doodads, shadows, and skybox render together without further renderer restructuring.

# Decision

## Environment owns presentation backdrop

Introduce the **Environment rendering layer** at `src/environment/`, registered by
[`EnvironmentPlugin`] in the `AppPlugin` composition root after
[`TerrainRuntimePlugin`] and before [`CameraPlugin`] (ADR-007).

| Layer | Owns |
|-------|------|
| **Environment** | [`EnvironmentSettings`], skybox cubemap load, [`GlobalAmbientLight`], directional light |
| **Not in this layer** | [`WorldData`], terrain meshes, doodad instances, weather, water, atmosphere, day/night |

Environment state is **not** stored in [`WorldData`] and is not tied to terrain streaming.
Terrain and doodads must **not** spawn lights.

## Module responsibilities

```text
src/environment/
    mod.rs        — public exports
    plugin.rs     — EnvironmentPlugin registration
    settings.rs   — EnvironmentSettings resource (single tuning authority)
    skybox.rs     — cubemap load + attach to primary camera
    lighting.rs   — ambient + directional light setup
    debug.rs      — dev diagnostics + singleton validation
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
| `skybox_set`, `skybox_brightness`, `skybox_rotation` | Cubemap backdrop |

Default rotation derives from [`DEFAULT_DIRECTIONAL_LIGHT_POSITION`] looking at
[`DEFAULT_DIRECTIONAL_LIGHT_LOOK_AT`] — not hardcoded in lighting setup.

Avoid scattering magic numbers in terrain, doodads, or preview code. When a value
affects the whole scene, it belongs in [`EnvironmentSettings`].

## Singleton expectations

The environment layer maintains exactly:

- **One** [`DirectionalLight`] tagged [`EnvironmentDirectionalLight`]
- **One** [`SkyboxCamera`] marker on the primary RTS camera (when skybox loads)
- **One** [`GlobalAmbientLight`] resource (Bevy default; values from settings)

[`EnvironmentLightingInitialized`] prevents duplicate light spawns on repeated
startup hooks. Dev builds run singleton validation in `PostStartup` via
[`count_environment_singletons`].

Shadows use Bevy directional-light defaults (no custom cascade tuning in R9).
Terrain preview uses a lit [`StandardMaterial`] so heightfield meshes receive
shadows; doodad glTF scenes use default PBR materials.

## Asset layout

All environment presentation assets live under:

```text
assets/environment/
    skyboxes/
        {set_name}/
            cubemap.ktx2    # preferred
            cubemap.png     # optional fallback (6×1 stacked square faces)
```

The initial set is **`default`** at `assets/environment/skyboxes/default/`.
Additional sets are sibling folders — not hardcoded in Rust.

Offline merge of loose face PNGs:

```text
cargo run --bin merge_skybox_cubemap -- {set_name}
```

## Runtime behavior

### Skybox

- Load once at startup; attach [`Skybox`] to the primary RTS [`Camera3d`]
  ([`RtsCamera`], ADR-014).
- [`SkyboxCamera`] marker prevents duplicate attachment.
- Missing or invalid assets: warning only, no panic.
- Camera movement does not require per-frame skybox updates.

### Lighting

- One [`DirectionalLight`] spawned from [`EnvironmentSettings`].
- [`GlobalAmbientLight`] configured from the same resource.
- No time-of-day or weather logic.

### Dev diagnostics (R9)

When the `dev` feature is enabled, startup logs:

- `Environment initialized`
- Full **Environment Settings** report (directional, ambient, skybox)
- Skybox load status
- Singleton validation summary
- `Skybox loaded` when attachment succeeds

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
- Bevy [`Skybox`] example: https://bevy.org/examples/3d-rendering/skybox/

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
