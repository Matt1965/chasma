# ADR-131: Volumetric Cloud Rendering Architecture

# Status

Accepted (CLOUD-VOL-R3/R4/V2A — renderer foundation proven; morphology continues under V1)

# Context

Chasma needs world-space volumetric clouds compatible with RTS camera motion, terrain
occlusion, future weather variation, and environment presentation. Earlier cloud
implementations exposed three classes of failure:

1. **Proxy-ray mismatch** — raster coverage geometry did not match the world ray used
   for density sampling.
2. **Depth reconstruction mismatch** — scene occlusion used a different fragment or
   camera authority than the cloud ray, producing angle-dependent terrain bleed.
3. **Discrete layered-alpha sampling** — fixed world-altitude slices with per-step
   alpha that ignored physical segment length produced a stacked translucent-paper
   appearance instead of a continuous volume.

Those failures established which authorities must own cloud geometry, occlusion, and
volume integration. CLOUD-VOL-R3/R4 and V2A validated the resulting contracts under
manual testing (continuous volume, camera stability, mountain occlusion).

Custom volumetric rendering is justified here by a specific visual requirement while
remaining replaceable and non-authoritative, consistent with
[ADR-004](ADR-004-renderer-policy.md).

# Decision

## Presentation scope

Clouds are **renderer/environment presentation**, not authoritative gameplay world
state. Weather simulation, precipitation, and gameplay visibility rules are separate
future systems.

## Raster coverage vs ray authority

The camera-centered proxy mesh owns **raster coverage only**. It does not define the
cloud world ray, sample positions, or density coordinates.

## Ray and depth contract (same fragment)

1. The cloud world ray comes from the Bevy render `View` and the current screen
   fragment.
2. Ray origin and inverse projection must come from the **same** render `View`
   (`view.world_position`, `frag_coord_to_ndc`, `position_ndc_to_world`).
3. Opaque prepass depth sampled for that **same** fragment is the occlusion
   authority.
4. Scene-world reconstruction uses that same fragment XY and sampled depth.

Do not substitute a separately synchronized CPU camera-position uniform for ray
origin when render View matrices are available.

## World-space density

5. Cloud density is sampled in **world coordinates**.
6. Camera motion observes the field; it does not move the density field.
7. Wind offset applies to world XZ sampling only; morphology coordinates remain
   world-anchored.

## Volumetric integration

8. Volumetric opacity uses extinction/transmittance integrated over **physical**
   ray-segment length (Beer–Lambert), not per-step `(1 - alpha)` compositing.
9. Raymarch sample count and step distribution are render-quality/performance
   settings, not the definition of cloud density.
10. Stable spatial ray-sample jitter may decorrelate visible marching structure,
    but it does not own cloud morphology and must not make density camera-relative.
11. Integrated march distance is capped as a **segment after cloud-band entry**, not
    as absolute camera distance. Shallow horizon rays enter the band far away; an
    absolute cap would incorrectly skip those fragments.

## Morphology and weather seam

12. High-level morphology inputs (`coverage`, `macro_scale`, `vertical_development`,
    `density_scale`, `edge_breakup`, altitude band bounds) form the future seam for
    weather; weather simulation itself is a separate future system.
13. Morphology algorithms may evolve (e.g. weather map → 3D body noise → vertical
    profile → erosion) while preserving world-space ownership and ray/depth/integration
    contracts above.

## Environment ownership

14. Environment presentation remains the correct ownership layer
    ([ADR-026](ADR-026-skybox-foundation.md), [ADR-068](ADR-068-environment-singleton-and-input-ownership.md)).
    Weather may later modulate environment lighting and morphology parameters without
    cloud rendering becoming authoritative simulation
    ([ADR-052](ADR-052-time-of-day-visual-environment-system.md)).

# Implementation anchors

| Concern | Location |
|---------|----------|
| Proxy spawn / sync | [`src/environment/procedural_clouds.rs`](../src/environment/procedural_clouds.rs) |
| Layer uniforms / settings | [`src/environment/cloud_material.rs`](../src/environment/cloud_material.rs), [`src/environment/cloud_settings.rs`](../src/environment/cloud_settings.rs) |
| WGSL raymarch + density | [`assets/shaders/environment_cloud.wgsl`](../assets/shaders/environment_cloud.wgsl) |

# Consequences

**Benefits:**

- Stable RTS camera behavior with world-anchored cloud masses
- Continuous terrain occlusion without angle-dependent bleed
- Clear separation between presentation clouds and future weather simulation
- Replaceable renderer implementation bounded by explicit contracts

**Costs:**

- Custom WGSL shader maintenance
- Manual validation required for visual morphology changes

# Non-goals

- Weather simulation, precipitation, or gameplay visibility authority
- Exact sample counts, noise scales, or extinction constants as architectural contracts
  (these remain tuning parameters)
- Finished photoreal cloud lighting in this ADR (lighting may evolve under morphology
  stages without changing ray/depth/integration ownership)

# References

- [ADR-004](ADR-004-renderer-policy.md) — renderer complexity policy
- [ADR-026](ADR-026-skybox-foundation.md) — environment rendering layer
- [ADR-052](ADR-052-time-of-day-visual-environment-system.md) — time-of-day / future weather seam
- [ADR-053](ADR-053-water-rendering-foundation.md) — parallel environment presentation pattern
- [ADR-068](ADR-068-environment-singleton-and-input-ownership.md) — environment ownership
