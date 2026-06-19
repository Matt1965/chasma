# ADR-016: Doodad Catalog Definitions

# Status

Accepted (Phase 3B — catalog foundation)

# Context

Phase 3A (ADR-015) established authoritative doodad **instance** records on
[`WorldData`]. Instances currently carry a coarse [`DoodadKind`] discriminant
(e.g. `Tree`, `Rock`). That is insufficient for procedural generation, authoring
tools, rendering, and gameplay: multiple tree variants share one kind but differ
in placement rules, scale ranges, and future mesh assets.

ROADMAP Phase 3 requires authored and procedural doodads. ARCHITECTURE Principle 5
(data first) and ADR-010 (runtime layer boundaries) require type definitions to
live outside ECS, rendering, and terrain runtime.

Phase 3B defines **what a doodad type is**, not how instances are spawned or drawn.

# Decision

## Catalog owns type definitions; WorldData owns instances

[`DoodadCatalog`] is a read-only Bevy [`Resource`] in the World Data Layer
(`src/world/doodad/catalog/`). It is initialized at startup and is **not** stored
on [`WorldData`].

| Concern | Owner |
|---------|--------|
| Instance records (position, source, id) | [`WorldData`] (ADR-015) |
| Type definitions (placement rules, scale, render key) | [`DoodadCatalog`] |
| Terrain heightfields | [`WorldData`] / [`ChunkData`] |
| Terrain meshes | Terrain runtime (ADR-010) |

Definitions are world-independent configuration. A single catalog can serve
multiple worlds or save files without duplicating type metadata.

## Explicit definition identifiers

[`DoodadDefinitionId`] is a stable string newtype (e.g. `tree_oak`, `rock_large`).
It is **not** an enum variant and **not** coordinate-derived.

Future procedural generation, persistence, and authoring reference
definition ids. [`DoodadKind`] remains a coarse category for grouping;
[`DoodadRecord`] stores [`DoodadDefinitionId`] as the authoritative type (ADR-017).

## DoodadDefinition contents (Phase 3B)

Each [`DoodadDefinition`] includes:

- `id`, `kind`, `display_name`
- `placement_radius_meters`, `min_scale`, `max_scale`
- optional `min_height`, `max_height`, `max_slope_degrees`
- `enabled`
- reserved `render_key` (no asset loading)
- reserved procgen fields: `placement_tags`, `biome_tags`, `spawn_weight`, `rule_ref`

No validation systems, generators, or renderers consume these fields in Phase 3B.

## Read-only after construction

[`DoodadCatalog::from_definitions`] builds indexes and rejects duplicate ids.
There are no runtime mutation APIs. Starter content ships in code
([`starter_definitions`]) until a file format is justified.

## Layer boundaries

- Terrain runtime must not import catalog types until cross-layer integration is required.
- Catalog must not spawn ECS entities, load assets, or reference mesh handles.
- [`ChunkData`] remains terrain-only (ADR-008).

# Future integration

## Rendering

[`DoodadRenderKey`] is a placeholder string for a future asset lookup. A later
`DoodadRuntimePlugin` (or renderer adapter) will map keys to meshes/materials,
mirroring ADR-010's separation of authoritative data from derived visuals.

## Procedural generation

Reserved tags, weights, and `rule_ref` support future biome-aware placement without
changing definition identity. Generators will read the catalog and write
[`DoodadRecord`] instances to [`WorldData`].

## Persistence

Save formats may store definition ids on instances. Catalog content may eventually
move to data files; ids must remain stable across versions.

# Rationale

Separating catalog from `WorldData` matches [`WorldConfig`] (static layout) vs
`WorldData` (dynamic state). Keeping definitions in the World Data Layer (not
terrain runtime) preserves ADR-010 boundaries while making types available to
future query, authoring, and simulation layers.

# Consequences

Benefits:

- Multiple variants per [`DoodadKind`]
- Stable ids for procgen and persistence
- No ECS or renderer coupling in the foundation

Costs:

- Two doodad-related resources (`WorldData` + `DoodadCatalog`) until unified tooling exists
- Phase 3A records initially used `kind` only; ADR-017 added `definition_id` on instances

# Alternatives Considered

## Definitions inline on WorldData

Rejected: conflates static type metadata with per-world instance state; complicates
multi-world and catalog hot-reload.

## Enum-only identity (extend DoodadKind)

Rejected: does not scale to many variants; unstable for persistence when variants grow.

## Catalog in terrain runtime

Rejected: violates ADR-010; terrain catalog is chunk asset metadata, not gameplay types.

# Notes

- Cross-references: ADR-015, ADR-010, ADR-008, ARCHITECTURE Doodad Layer, ROADMAP Phase 3.
- Starter catalog: seven definitions covering all five [`DoodadKind`] values with
  two tree and two rock variants.

[`WorldData`]: ../src/world/data.rs
[`WorldConfig`]: ../src/world/config.rs
[`ChunkData`]: ../src/world/chunk.rs
[`DoodadCatalog`]: ../src/world/doodad/catalog/registry.rs
[`DoodadDefinition`]: ../src/world/doodad/catalog/definition.rs
[`DoodadDefinitionId`]: ../src/world/doodad/catalog/definition_id.rs
[`DoodadRenderKey`]: ../src/world/doodad/catalog/render_key.rs
[`DoodadKind`]: ../src/world/doodad/kind.rs
[`DoodadRecord`]: ../src/world/doodad/record.rs
[`starter_definitions`]: ../src/world/doodad/catalog/starter.rs
