# ADR-017: Doodad Authoring and Placement

# Status

Accepted (Phase 3C — authoring foundation)

# Context

Phase 3A (ADR-015) established doodad **instance** storage on [`WorldData`].
Phase 3B (ADR-016) established **type definitions** in [`DoodadCatalog`].
Instances initially carried only a coarse [`DoodadKind`]; multiple catalog
definitions share a kind (e.g. `tree_oak` and `tree_dead` are both `Tree`).

ROADMAP Phase 3 and ARCHITECTURE Principle 5 require a definition-driven placement
workflow before editor tools, procedural generation, imported POIs, gameplay
placement, or persistence. Phase 3C adds authoritative world-data authoring only:
no ECS, rendering, streaming, save/load, or terrain validation.

# Decision

## Definition-driven instances

[`DoodadRecord`] now stores [`DoodadDefinitionId`] as the **authoritative type
reference**. [`DoodadKind`] remains as denormalized cache copied from the catalog
at creation time so coarse filters avoid catalog lookups.

All future systems (editor, procgen, persistence, rendering) should operate from
definition ids, not kind alone.

## Authoring API ownership

Authoritative placement operations live in `src/world/doodad/authoring.rs`:

| Function | Purpose |
|----------|---------|
| `create_doodad` | Validate definition, allocate id, insert record |
| `move_doodad` | Relocate instance, including cross-chunk moves |
| `remove_doodad` | Remove by id, return record |
| `lookup_doodad` | Borrow instance by id |

These operate on [`WorldData`] + [`DoodadCatalog`]. They are **not** ECS systems
and do not touch terrain runtime (ADR-010).

## Create workflow

```text
DoodadDefinitionId + WorldPosition + optional overrides
    → validate definition exists and is enabled
    → validate scale within definition min/max
    → allocate DoodadId
    → build DoodadRecord (definition_id + cached kind)
    → insert into chunk-local store
```

Slope, terrain height, exclusion zones, and collisions are **not** validated in
Phase 3C.

## Move semantics

[`WorldData::relocate_doodad`] removes the record from its current chunk store,
updates [`DoodadPlacement::position`], and re-inserts under the new owning
[`ChunkId`]. Preserved fields: id, definition_id, kind, source, metadata,
rotation, scale.

Cross-chunk moves are transparent to callers via `move_doodad`.

## Required id index on WorldData

[`WorldData`] **must** maintain `doodad_locations: HashMap<DoodadId, ChunkId>`
alongside the chunk-keyed stores. This is required architecture for O(1) lookup,
move, and remove by id — not an optional optimization.

Every doodad mutation path (`insert_doodad`, `remove_doodad`, `remove_doodad_by_id`,
`relocate_doodad`, and authoring create/move/remove) keeps the index synchronized
with the chunk stores. Tests assert bidirectional index integrity after create,
move, and remove.

Cross-chunk spatial queries still need a future spatial index (Phase 4).

## Explicit authoring errors

[`DoodadAuthoringError`] covers `DefinitionNotFound`, `DefinitionDisabled`,
`DoodadNotFound`, `ScaleOutOfRange`, and `ChunkPlacementMismatch`. No string
errors.

## Layer boundaries unchanged

- [`DoodadCatalog`] owns definitions; [`WorldData`] owns instances.
- Terrain runtime does not import doodad authoring.
- No ECS entity promotion in this phase.

# Future integration

## Editor tools

Editor UI will call `create_doodad`, `move_doodad`, and `remove_doodad` against
live [`WorldData`], reading constraints from [`DoodadCatalog`].

## Procedural generation

Generators will select [`DoodadDefinitionId`] from the catalog (by weight/tags in
later phases), call `create_doodad` with `DoodadSource::Procedural { seed }`, and
rely on the same validation path as authored placement.

## Persistence

Save formats will store definition ids on instances. Kind cache is reconstructible
from the catalog and need not be authoritative on disk.

# Rationale

A single authoring path prevents editor, procgen, and import code from duplicating
validation and index maintenance. Definition ids as authority align with ADR-016
and support multiple variants per kind without enum explosion.

# Consequences

Benefits:

- Clean seam for editor, procgen, and persistence
- O(1) instance lookup by id
- Cross-chunk move without full-world scan

Costs:

- `DoodadRecord` schema change (breaking for in-memory tests only; no save format yet)
- Index must stay synchronized with chunk stores on all mutation paths

# Alternatives Considered

## Keep kind-only records

Rejected: cannot distinguish `tree_oak` from `tree_dead`; blocks definition-driven procgen.

## Scan all chunks on lookup/remove

Rejected: O(chunks × doodads) does not scale; index is cheap at expected instance counts.

## Authoring as ECS systems

Rejected: violates data-first principle; world data must be authoritative before entities exist.

# Notes

- Cross-references: ADR-015, ADR-016, ADR-010, ROADMAP Phase 3–5–7.
- [`DoodadPlacementOverrides`] supplies optional rotation and scale at create time.

[`WorldData`]: ../src/world/data.rs
[`DoodadCatalog`]: ../src/world/doodad/catalog/registry.rs
[`DoodadRecord`]: ../src/world/doodad/record.rs
[`DoodadDefinitionId`]: ../src/world/doodad/catalog/definition_id.rs
[`DoodadKind`]: ../src/world/doodad/kind.rs
[`DoodadAuthoringError`]: ../src/world/doodad/authoring.rs
[`DoodadPlacementOverrides`]: ../src/world/doodad/authoring.rs
[`ChunkId`]: ../src/world/chunk.rs
