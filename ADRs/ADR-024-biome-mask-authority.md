# ADR-024: Biome Mask Authority

# Status

Accepted (Phase R1 — biome mask foundation)

# Context

Future procedural doodad placement (ADR-018, ADR-023) and resource distribution
require a world-scale spatial classification layer. The author intends to supply
biome coverage as a raster image (e.g. `source_data/test/biome_mask.png`,
initially 1024×1024) rather than paint biomes in-engine.

ADR-008 places per-chunk [`TerrainMask`] layers on [`ChunkData`] for terrain
material blending. Those masks are chunk-local heightfield-aligned samples.
Biome classification is **world-scoped**, covers the full authored extent, and
is independent of terrain chunk residency.

ADR-015 stores doodads in a parallel map on [`WorldData`], not in [`ChunkData`].
Biome masks follow the same pattern: a **separate world layer** beside terrain
and doodads, owned by [`WorldData`].

Phase R1 is **data only**: no doodad generation, rendering, gameplay, streaming,
save/load, or editor tooling.

# Decision

## Separate world layer on WorldData

[`WorldData`] owns an optional [`BiomeMask`]. Biome data does **not** live in
[`ChunkData`], the terrain runtime, doodad runtime, or ECS.

```text
WorldData
  ChunkId → ChunkData          (terrain heightfields — existing)
  ChunkId → ChunkDoodadStore   (doodads — ADR-015)
  Option<BiomeMask>            (world-scale biome authority — new)
```

## Image-based authoring workflow

1. Author exports a PNG covering the full world extent.
2. Pixel RGB color maps to [`BiomeId`] via [`BiomeColorMapping`].
3. Import decodes PNG, classifies each pixel, stores compact [`BiomeId`] grid.
4. Image crate types are not retained after import.

PNG only in Phase R1. No EXR requirement.

## Starter color classification (Phase R1)

| Color | Biome |
|-------|-------|
| Red | Desert |
| Green | Forest |
| Blue | Marsh |
| Yellow | Plains |
| Black | Unassigned / invalid |
| Unmapped colors | Unassigned |

Classification is data only — no gameplay behavior is attached to biomes.

## World-position sampling

[`WorldData::biome_at`] composes global XZ from [`WorldPosition`] and samples
the mask. Sampling is:

- deterministic
- pure lookup (no terrain height, slope, or chunk residency)
- valid even when terrain chunks are unloaded

When no mask is loaded, queries return `None`. When loaded, out-of-bounds
positions and unmapped pixels return [`BiomeId::Unassigned`] inside a
[`BiomeSample`].

## Coordinate mapping

[`BiomeMaskBounds`] maps world XZ units to image pixels:

- Origin: southwest corner of the authored extent (`ChunkExtent` × [`ChunkLayout`])
- `extent_x` / `extent_z`: full world width and depth in units
- Column `x` ← world X; row `z` ← world Z (row 0 = minimum Z / south edge)
- Pixel index: `z * width + x` (row-major)

Mapping functions are explicit, tested, and documented on [`BiomeMaskBounds`].

## Future extension points (not implemented)

The mask model reserves seams for later channels without changing ownership:

- multiple biome channels
- density masks
- doodad placement masks
- resource masks
- moisture, temperature, fertility
- imported splat maps

These may attach as additional layers on [`BiomeMask`] or sibling world data
structures; Phase R1 stores a single classified biome grid only.

## Relationship to future systems

- **Doodad generation (ADR-018):** generators will read [`BiomeId`] at candidate
  positions to filter catalog definitions by `biome_tags` (ADR-016).
- **Resource generation:** future resource rules will sample the same authority.
- **Rendering:** may visualize biomes in dev tools later; no render coupling in R1.

# Rationale

World-scoped rasters do not belong on per-chunk terrain geometry (ADR-008). A
parallel layer on [`WorldData`] matches doodad ownership (ADR-015), keeps queries
independent of terrain residency, and supports author workflow (external PNG).

# Consequences

Benefits:

- Deterministic biome lookup before terrain load
- Clear import path aligned with external tooling
- No runtime layer or ECS coupling

Costs:

- Single raster resolution for entire world (acceptable at 1024² for Phase R1)
- Extent must be set before import bounds are computed

# Alternatives Considered

## Biome masks on ChunkData

Rejected: biome coverage is world-global, not chunk-local heightfield-aligned data.

## ECS Resource for BiomeMask

Rejected: splits authoritative world state from [`WorldData`]; complicates
persistence and queries.

## In-engine biome brush

Rejected: author prefers uploaded color images (ADR-023).

# Notes

- Cross-references: ADR-008, ADR-015, ADR-016, ADR-018, ADR-023, ARCHITECTURE
  Principle 2 (chunks own masks at geography level — world-scale masks are
  world data, not chunk terrain geometry).
- **Dev startup (Phase R3):** with the `dev` feature, terrain preview startup
  imports `source_data/test/biome_mask.png` into [`WorldData`] via
  [`try_load_default_dev_biome_mask`]. Bounds derive from authored
  [`ChunkExtent`] + [`WorldConfig`]. Missing files warn; import failures error;
  startup continues without a mask. No runtime copy or ECS resource.

[`WorldData`]: ../src/world/data.rs
[`ChunkData`]: ../src/world/chunk.rs
[`TerrainMask`]: ../src/world/terrain/mask.rs
[`BiomeMask`]: ../src/world/biome/mask.rs
[`BiomeId`]: ../src/world/biome/id.rs
[`BiomeSample`]: ../src/world/biome/sample.rs
[`BiomeColorMapping`]: ../src/world/biome/mapping.rs
[`BiomeMaskBounds`]: ../src/world/biome/mask.rs
[`WorldPosition`]: ../src/world/coordinates.rs
[`ChunkExtent`]: ../src/world/data.rs
[`ChunkLayout`]: ../src/world/coordinates.rs
