# ADR-026: Skybox Foundation

# Status

Accepted (R8 — dev preview rendering)

# Context

The dev preview renders terrain and doodads against a clear default background.
A skybox improves spatial orientation and establishes a permanent asset location
for future time-of-day, weather, and biome presentation — without coupling to
world simulation.

# Decision

## Rendering-only ownership

Introduce a small **Skybox layer** at `src/skybox/`, registered by `SkyboxPlugin`
in the `AppPlugin` composition root when the `dev` feature is enabled (ADR-007).

| Layer | Owns |
|-------|------|
| **Skybox runtime** | cubemap load, [`SkyboxSettings`], primary-camera [`Skybox`] component |
| **Not in this ADR** | [`WorldData`], terrain, biomes, weather, day/night, HDR, reflections |

Skyboxes are **not** stored in [`WorldData`] and are not tied to terrain streaming.

## Asset layout

All skybox sets live under:

```text
assets/skyboxes/
    {set_name}/
        cubemap.ktx2    # preferred
        cubemap.png     # optional fallback (6×1 stacked square faces)
```

The initial dev preview loads set **`default`** from
`assets/skyboxes/default/`.

## Runtime behavior

- Load once at startup; attach [`Skybox`] to the primary RTS [`Camera3d`]
  (identified by [`RtsCamera`], ADR-014).
- [`SkyboxCamera`] marker prevents duplicate attachment.
- Missing or invalid assets: log a warning, continue running (no panic).
- Camera movement does not require per-frame skybox updates.

## Future extension

[`SkyboxSettings::active_set`] is the seam for swapping cubemap folders.
Day/night, weather, and biome-dependent skies will update this renderer-facing
resource (or replace the cubemap handle) — not world simulation state.

# Consequences

- **Positive:** Clear art drop-in path; isolated from terrain/doodad pipelines.
- **Positive:** Matches Bevy’s built-in [`Skybox`] component (no custom shader).
- **Negative:** PNG cubemaps require stacked layout or manual cubemap metadata;
  authors should prefer KTX2 for production sets.
- **Neutral:** No lighting or IBL changes in R8 ([`EnvironmentMapLight`] deferred).

# References

- ADR-007 (composition root)
- ADR-014 (primary RTS camera)
- ADR-023 (runtime vs world-data split pattern)
- Bevy [`Skybox`] example: https://bevy.org/examples/3d-rendering/skybox/
