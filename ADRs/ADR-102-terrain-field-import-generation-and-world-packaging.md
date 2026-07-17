# ADR-102: Terrain Field Import, Generation, and World Packaging (TF2)

## Status

Accepted — TF2 implemented.

## Context

TF1 (ADR-101) established authoritative `TerrainFieldStore`, shared-edge 33×33 `u16` tiles,
deterministic CPU queries, and world-package loading. TF2 adds **authoring pipelines** that
produce those base tiles from imported masks or seeded generators.

## Decision

### Source profiles

- `TerrainFieldSourceProfileDefinition` + `TerrainFieldSourceProfileCatalog` (`Resource`).
- Committed RON: `assets/terrain_fields/source_profiles.ron`.
- `TerrainFieldSourceKind`: `ImportedMask`, `Generated`, `Combined` (reserved; not implemented in TF2).
- One active profile per field; imported **or** generated — no combining in TF2.

### Imported masks

- PNG 8-bit / 16-bit grayscale (optional RGB/RGBA channel select).
- **Linear data masks** — no sRGB/gamma correction.
- 8-bit expansion: `u16 = u8 × 257`.
- `TerrainFieldImageOrientation`:
  - `RowZeroIsMinimumZ` (default, matches ADR-024 biome convention).
  - `RowZeroIsMaximumZ` (image row 0 = north; rows flipped when sampling world grid).
- Deterministic offline resampling (`Nearest`, `Bilinear`) with fixed-point weights (`FP_SCALE = 256`).
- `TerrainFieldValueRemap` for input/output clamp and invert.
- Full-world image mapped to authored chunk extent; target grid:
  `chunks_x × 32 + 1` by `chunks_z × 32 + 1` samples.

### Generated fields

- Offline evaluation at global XZ — no per-chunk seeding, no runtime RNG.
- `field_seed = hash(world_seed, field_id, profile_id, generator_version)`.
- Typed `TerrainFieldGeneratorKind` enum (not script plugins).
- Initial generators:
  - **Water** `LowlandWaterPotential` — broad aquifer FBM, lowland bias, optional height suppression.
  - **Iron** `GeologicalVeins` — warped ridged domains, thresholded rich pockets.
  - **Copper** `CopperPockets` — distinct scattered pocket noise (not recolored iron).
  - **Stone** `StoneExposure` — elevation/slope/biome-correlated suitability (baked to tiles).
- Declared `TerrainFieldGeneratorDependency`: `Heightfield`, `BiomeMask`.
- `TerrainFieldSourceProvenance` + deterministic `source_version_hash` on every build.

### Packaging

- Build writes to `.build_tmp`, validates shared edges, then atomically commits to
  `assets/worlds/<world>/terrain_fields/`.
- Manifest written last; prior package preserved on failure.
- `TerrainFieldStatistics` (min/max/avg, histogram, zero%, edge validation).

### Dev Mode (TF2)

- **Fields** tab shows source profile, generator/import metadata.
- Hotkeys: `B` build field, `Shift+B` build all, `V` validate, `R` reload package, `G` gizmos.
- Build uses authoritative bake APIs — no direct store mutation before validation.

## Consequences

- Base terrain fields remain **immutable** world potential after packaging.
- TF1 query APIs unchanged; runtime cannot distinguish import vs generation.
- TF3 can add GPU overlays without altering tile authority.
- TF4+ can use provenance hashes to detect stale building assessments.

## Non-goals (TF2)

Player overlays, terrain shaders, building requirements, build-mode previews, efficiency,
extraction/depletion, survey knowledge, player painting, combined sources, hydrology simulation.
