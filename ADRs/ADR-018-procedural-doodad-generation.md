# ADR-018: Procedural Doodad Generation

# Status

Accepted (Phase 3D — generation foundation)

# Context

Phase 3A–3C established doodad instance storage ([`WorldData`], ADR-015),
type definitions ([`DoodadCatalog`], ADR-016), and authoritative placement
([`authoring`], ADR-017). Procedural content (forests, rocks, resource scatter)
must be generated deterministically before instances are created.

ROADMAP Phase 3 requires procedural doodads alongside authored content.
ARCHITECTURE Principle 5 (data first) and the scalability rule ("existing but not
simulated/rendered") require separating **generation output** from **world
instances**.

Phase 3D answers:

> Given a chunk and a world seed, what doodads *would* exist here?

It does **not** insert candidates into [`WorldData`], spawn ECS entities, or
touch terrain runtime (ADR-010).

# Decision

## Generation vs world ownership

| Layer | Owns |
|-------|------|
| [`DoodadCatalog`] | Type definitions |
| [`WorldData`] | Materialized instances ([`DoodadRecord`]) |
| `generation` module | Ephemeral [`DoodadSpawnCandidate`] output |

Generation is pure: no [`WorldData`] reads or writes, no ECS, no rendering.

## Candidate concept

[`DoodadSpawnCandidate`] describes a procedural placement proposal:

- `definition_id` (from catalog)
- `source: Procedural { seed }`
- `position`, `rotation`, `scale`

No [`DoodadId`], no metadata, no chunk-store ownership. A later phase maps:

```text
candidate → authoring / materialization → DoodadRecord → WorldData
```

## Deterministic generation requirements

[`generate_chunk_doodads`] must be:

- **Deterministic** — same `world_seed` + [`ChunkId`] → identical candidate list
- **Side-effect free** — no mutation of catalog or world state
- **Catalog-driven** — enabled definitions selected via [`DoodadCatalog`] queries,
  not hardcoded definition ids

Chunk-local RNG seed: `chunk_seed(world_seed, chunk.x, chunk.z)` feeding
SplitMix64 ([`DeterministicRng`]). Each candidate receives its own procedural
`source` seed from the stream.

Output is sorted for stable ordering (definition id, local xz, procedural seed).

## Starter rules (Phase 3D)

[`DoodadGenerationSettings`] defines per-kind counts (trees, rocks, bushes; ruins
and resource nodes default to zero). For each slot:

1. Pick an enabled definition of that kind from the catalog using
   [`DoodadDefinition::spawn_weight`] (catalog-driven weighted selection)
2. Sample local XZ within chunk bounds respecting `placement_radius_meters`
3. Sample uniform scale in `[min_scale, max_scale]`
4. Sample Y-axis rotation

Biome membership, terrain validation, and exclusion run during materialization
(ADR-020–ADR-025), not in the generator.

## Extension points reserved

Reserved boolean fields on [`DoodadGenerationSettings`] mirror future generation
hooks; filtering is applied during materialization ([`MaterializationOptions`]),
not in the generator.

# Future integration

## Biome generation

[`DoodadDefinition::allowed_biomes`] (ADR-025) filters eligible definitions during
materialization. Generation context does not read biome data directly.

## Exclusion zones

[`DoodadExclusionZone`] on [`WorldData`] (ADR-015) suppress candidates during
the materialization pipeline ([`MaterializationOptions::apply_exclusion_zones`],
ADR-020) — not during generation (Phase 3D).

## Persistence

Procedural baseline: regenerate from `world_seed` + chunk + catalog version.
Gameplay overrides remain authored instance records (ADR-015 `DoodadSource`).

Save formats may store materialized instances only, or seed + delta; generation
layer stays reproducible either way.

## World materialization

A future phase calls `create_doodad` (ADR-017) per accepted candidate, or batch
inserts after validation. Streaming may generate candidates when chunks enter
the procedural ring without loading all instances upfront.

# Rationale

Separating candidates from instances keeps procedural preview, validation, and
authoring-blend logic independent of world mutation. Deterministic chunk seeds
enable identical worlds across machines and stable regression tests.

# Consequences

Benefits:

- Testable procgen without world state
- Clear seam for biomes, terrain validation, exclusion
- Catalog remains single source of type truth

Costs:

- Extra conversion step before instances exist
- Starter rules are placeholder density, not gameplay-tuned

# Alternatives Considered

## Generate directly into WorldData

Rejected: couples procgen to instance lifecycle; blocks preview/dry-run and violates
Phase 3D scope.

## Global RNG without chunk mixing

Rejected: same sequence offset would correlate placements across chunks.

## Hardcoded definition ids in generator

Rejected: violates ADR-016; breaks when catalog content changes.

# Notes

- Cross-references: ADR-015, ADR-016, ADR-017, ADR-010, ROADMAP Phase 3–4–7.
- Module: `src/world/doodad/generation/`.

[`WorldData`]: ../src/world/data.rs
[`DoodadCatalog`]: ../src/world/doodad/catalog/registry.rs
[`DoodadRecord`]: ../src/world/doodad/record.rs
[`DoodadSpawnCandidate`]: ../src/world/doodad/generation/candidate.rs
[`DoodadGenerationContext`]: ../src/world/doodad/generation/context.rs
[`DoodadGenerationSettings`]: ../src/world/doodad/generation/settings.rs
[`generate_chunk_doodads`]: ../src/world/doodad/generation/generator.rs
[`DeterministicRng`]: ../src/world/doodad/generation/rng.rs
[`DoodadExclusionZone`]: ../src/world/doodad/exclusion/zone.rs
[`authoring`]: ../src/world/doodad/authoring.rs
[`ChunkId`]: ../src/world/chunk.rs
