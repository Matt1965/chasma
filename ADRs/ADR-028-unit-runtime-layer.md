# ADR-028: Unit Runtime Layer

# Status

Accepted (U3 — runtime visual sync)

# Context

U1 established unit type definitions in [`UnitCatalog`]. U2 added authoritative
[`UnitRecord`] instances on [`WorldData`]. Rendering, movement, and simulation
remain separate concerns per ADR-027 and ARCHITECTURE Principle 6.

ADR-010 separated terrain **truth** from terrain **visualization**. ADR-023
established the doodad runtime pattern: [`WorldData`] owns records; a disposable
ECS layer owns glTF scene entities and sync state. Units follow the same split.

# Decision

## A distinct Unit Runtime Layer

Introduce a Unit Runtime Layer at `src/units/`, registered by `UnitsRuntimePlugin`
in the `AppPlugin` composition root:

```text
TerrainRuntimePlugin
DoodadsRuntimePlugin
UnitsRuntimePlugin
EnvironmentPlugin
CameraPlugin
```

Sync systems run in [`UnitRuntimeSystems`], after [`DoodadRuntimeSystems`], so
unit visibility follows the same terrain residency signal as doodads.

## Ownership split

| Layer | Owns |
|-------|------|
| [`WorldData`] | [`UnitRecord`], chunk stores, id index |
| [`UnitCatalog`] | type definitions, [`UnitRenderKey`] |
| **Unit runtime** | ECS render entities, glTF scene handles, residency sync state |
| **Not in U3** | movement, pathfinding, collision, selection, AI, combat, save/load |

- The Unit Runtime Layer depends on the World Data Layer and reads
  [`ChunkResidencyTracker`] for terrain residency. It does **not** mutate
  authoritative unit records or simulation state.
- [`WorldData`] never depends on the Unit Runtime Layer or rendering.

## One-way data flow

```text
WorldData (authoritative)              Unit runtime (derived)
  UnitRecord + UnitCatalog        ──►  glTF scene entity (Transform from placement)
  ChunkResidencyTracker (terrain) ──►  spawn when chunk terrain resident; despawn on unload
```

ECS entities are disposable. Queries and simulation read [`WorldData`] only.

## Visibility rule

A unit **renders** only when:

1. Its owning chunk has **terrain resident** in [`WorldData`] and
   [`ChunkResidencyTracker::is_resident`], and
2. [`UnitCatalog`] contains the definition, and
3. A glTF asset resolves for the definition's [`UnitRenderKey`].

When terrain unloads, render entities despawn. **Records remain** in
[`WorldData`]. When terrain reloads, sync respawns entities from data.

## Vertical scale (presentation)

[`WorldData`] stores authoritative placement Y in world units. Render transforms
apply [`TerrainRenderAssets::vertical_scale`] to Y only, matching doodads and
terrain mesh exaggeration (ADR-010). Simulation and persistence use unscaled
[`WorldData`] Y. Sync never writes back to [`UnitRecord`].

## Asset convention

Excel `File Path` cells normalize to bare render keys via
`normalize_file_path_to_render_key` (`\units\wolf.glb` → `wolf`). Runtime maps:

```text
render_key "wolf"  →  assets/units/wolf.glb  (Scene 0)
```

- Path: `units/{key}.glb` relative to the asset root.
- Keys that still include `units/` or `assets/units/` prefixes are stripped at resolve time.
- Scene index: `0` unless a definition later specifies otherwise.
- Missing assets: skip spawn, log once per key. No panic, no placeholder meshes.

Starter catalog keys (`wolf`, `bandit`, `deer`) align with this layout.

## Components (U3)

- [`UnitRenderEntity`] — links ECS entity to [`UnitId`]
- [`UnitSceneRoot`] — marker on spawned glTF root

No gameplay, selection, or movement components in U3.

## Deferred (explicit non-goals for U3)

- Movement and locomotion systems
- Pathfinding and obstacle integration
- Animation state machines
- Combat, AI, commands, selection
- Save/load overlays
- Auto dev spawn / unit procgen

# Consequences

**Benefits:**

- Clear boundary between instance data and visuals, matching doodads.
- Asset drop-in path for author-supplied unit glTF files.
- Terrain residency drives render lifecycle consistently with ADR-012/015/023.

**Costs:**

- No visible units until assets exist and records are authored in resident chunks.
- Per-instance ECS entities until instancing (acceptable for early validation).

# References

- ADR-010 (terrain visualization scale)
- ADR-023 (doodad runtime pattern)
- ADR-027 (unit data ownership)
- ADR-007 (composition root)
- ARCHITECTURE Principle 6 (Data First)

[`WorldData`]: ../src/world/data.rs
[`UnitRecord`]: ../src/world/unit/record.rs
[`UnitCatalog`]: ../src/world/unit/catalog/registry.rs
[`UnitRenderKey`]: ../src/world/unit/catalog/render_key.rs
[`UnitRenderEntity`]: ../src/units/components.rs
[`UnitsRuntimeSettings`]: ../src/units/settings.rs
[`ChunkResidencyTracker`]: ../src/terrain/residency.rs
[`DoodadRuntimeSystems`]: ../src/doodads/sync.rs
[`UnitRuntimeSystems`]: ../src/units/sync.rs
