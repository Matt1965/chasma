# ADR-023: Doodad Runtime Layer

# Status

Accepted (Phase 3I — runtime foundation)

# Context

Phase 3A–3H established authoritative doodad **data** on [`WorldData`]: instance
records, catalog definitions, generation, exclusion, terrain validation, and
materialization (ADR-015 through ADR-022).

ADR-010 separated terrain **truth** from terrain **visualization**. Doodads follow
the same split: [`WorldData`] owns records; a runtime layer owns derived ECS
entities and asset handles.

The first runtime milestone uses **real glTF assets** supplied by the author, not
placeholder primitives. Procedural population is required biome-mask tooling are
**deferred** until they match the intended final pipeline — not wired as temporary
dev shortcuts that would need rework.

# Decision

## A distinct Doodad Runtime Layer

Introduce a Doodad Runtime Layer at `src/doodads/`, registered by
`DoodadsRuntimePlugin` in the `AppPlugin` composition root after
`TerrainRuntimePlugin` (ADR-007). Sync systems run after
[`TerrainStreamingSystems`] so visibility follows terrain residency.

## Ownership split

| Layer | Owns |
|-------|------|
| [`WorldData`] | [`DoodadRecord`], chunk stores, procedural key index, exclusion zones |
| [`DoodadCatalog`] | type definitions, [`DoodadRenderKey`] |
| **Doodad runtime** | ECS render entities, glTF scene handles, residency sync state |
| **Not in this ADR** | save/load, instancing, gameplay queries, editor UI |

- The Doodad Runtime Layer depends on the World Data Layer and reads
  [`ChunkResidencyTracker`] for terrain residency. It does not mutate authoritative
  doodad records except through future explicit integration points.
- [`WorldData`] never depends on the Doodad Runtime Layer or rendering.

## One-way data flow

```text
WorldData (authoritative)              Doodad runtime (derived)
  DoodadRecord + DoodadCatalog    ──►  glTF scene entity (Transform from placement)
  ChunkResidencyTracker (terrain) ──►  spawn when chunk terrain resident; despawn on unload
```

Queries continue to read [`WorldData`] only (ADR-005). Render entities are not
authoritative.

## Visibility rule

A doodad **renders** only when:

1. Its owning chunk has **terrain resident** in [`WorldData`] (via
   [`ChunkResidencyTracker::is_resident`]), and
2. A glTF asset resolves for its definition's [`DoodadRenderKey`].

When terrain unloads, render entities despawn. **Records remain** in
[`WorldData`] (ADR-015). When terrain reloads, sync respawns entities from data.

## Asset convention

[`DoodadRenderKey`] maps to glTF under `assets/doodads/`:

```text
render_key "tree/oak"  →  assets/doodads/tree/oak.glb  (Scene 0)
```

- Path: `doodads/{key}.glb` relative to the asset root.
- Scene index: `0` unless a definition later specifies otherwise.
- Models should be authored with origin at ground contact; scale comes from
  [`DoodadPlacement::scale`] on the record.
- Missing assets: skip spawn (log once per key in dev). No primitive placeholders
  in the runtime layer.

Starter catalog keys (`tree/oak`, `tree/dead`, etc.) align with this layout so
uploaded `.glb` files drop in without code changes.

## Fixed world seed (future procgen)

[`DoodadsRuntimeSettings`] holds `world_seed: u64` (fixed default for development).
The seed is **not consumed** by the runtime layer in Phase 3I. It exists so a
future procedural materialization pass (ADR-018/019) uses a single configured
value rather than ad-hoc literals.

**Phase 3I explicitly does not** auto-run `generate_chunk_doodads` or
`materialize_candidates` on chunk load. Population enters through:

- authored placement ([`create_doodad`], ADR-017), or
- a future explicit materialization step once mask + rules match final design.

## Biome / placement masks (deferred, image-based)

In-engine brush tooling is **out of scope**. Future biome-aware procedural
placement will use **authored raster masks** (e.g. 1024×1024 PNG) where pixel
color maps to biome or placement rule ids. Masks are world-scoped data imported
alongside terrain, sampled at world XZ — not painted at runtime.

This ADR records the direction only. Mask import, catalog `biome_tags` wiring,
and generator integration remain future work (ROADMAP Phase 3+).

## Build order (vertical slices)

1. **Phase 3I (this ADR):** plugin, settings, asset path resolution, preload
   handles, sync spawn/despawn for existing [`WorldData`] records.
2. **Phase 3J:** height snap / terrain alignment for authored placement (if needed).
3. **Phase 3K:** procedural materialization hook (seed + image masks + ADR-018/019
   pipeline) once mask format and rules are locked.
4. **Later:** instancing, LOD, persistence overlays (ROADMAP).

**Dev preview (R4):** With the `dev` feature, resident terrain chunks with no
existing doodad records trigger one procedural generate + materialize pass via
[`try_materialize_procedural_chunk_doodads`], using [`DoodadsRuntimeSettings::world_seed`].
Records sync through the existing runtime layer; production streaming remains future work.

# Rationale

Mirroring ADR-010 keeps doodad truth in [`WorldData`] and makes rendering
replaceable. Starting with real glTF avoids a placeholder path that would be
removed. Deferring procgen avoids baking dev-only behavior that diverges from
the hybrid materialization model (ADR-019).

Image-based biome masks match the external authoring workflow used for terrain
(Gaea → export → import) and avoid editor scope creep.

# Consequences

Benefits:

- Clear boundary between instance data and visuals.
- Asset drop-in path for author-supplied trees.
- Terrain residency drives render lifecycle consistently with ADR-012/015.

Costs:

- No visible doodads until assets exist and records are authored (or procgen is
  wired in a later slice).
- Per-instance ECS entities until instancing (acceptable for early validation).

# Alternatives Considered

## Placeholder meshes (capsules/spheres) until glTF

Rejected for this project phase: author will supply real `.glb` files; placeholders
add a code path to remove.

## Auto procedural populate on chunk load (dev preview)

Rejected: does not match final materialization + mask pipeline; would require
rework when image masks and rules land.

## In-engine biome brush

Rejected: author prefers uploaded color images; simpler and consistent with
external tooling.

# Notes

- Cross-references: ADR-010, ADR-015, ADR-016, ADR-017, ADR-018, ADR-019,
  ADR-012, ARCHITECTURE Doodad Layer, ROADMAP Phase 3.
- [`ChunkResidencyTracker`] lives in the terrain runtime layer; doodad runtime
  reads it as a residency signal only (no reverse dependency from terrain to doodads).

[`WorldData`]: ../src/world/data.rs
[`DoodadRecord`]: ../src/world/doodad/record.rs
[`DoodadCatalog`]: ../src/world/doodad/catalog/registry.rs
[`DoodadRenderKey`]: ../src/world/doodad/catalog/render_key.rs
[`DoodadsRuntimeSettings`]: ../src/doodads/settings.rs
[`ChunkResidencyTracker`]: ../src/terrain/residency.rs
[`TerrainStreamingSystems`]: ../src/terrain/lifecycle.rs
[`create_doodad`]: ../src/world/doodad/authoring.rs
