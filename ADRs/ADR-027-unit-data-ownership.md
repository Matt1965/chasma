# ADR-027: Unit Data Ownership

# Status

Accepted (U2 — authoritative unit instances on WorldData)

# Context

Units require a data-first foundation matching doodads (ADR-015, ADR-016) and
ARCHITECTURE Principle 6. The workbook `Chasma Design.xlsx` already authors unit
stats on the **Units** sheet; locomotion and render columns are added for runtime
prototypes.

U1 established **type definitions**. U2 adds **authoritative instances** on
[`WorldData`]. Simulation, rendering, and pathfinding remain deferred.

# Decision

## Catalog owns type definitions; WorldData owns instances (U2)

| Concern | Owner |
|---------|-------|
| Type definitions | [`UnitCatalog`] resource |
| Instance records | [`WorldData`] — [`UnitRecord`] in [`ChunkUnitStore`] |
| Simulation state | [`UnitState`] on [`UnitRecord`] (placeholder); full [`UnitSimulationState`] envelope U3+ |
| ECS visuals | `UnitsRuntimePlugin` (U3+) |

[`UnitCatalog`] is a read-only Bevy [`Resource`] in the World Data Layer
(`src/world/unit/catalog/`), registered by [`WorldFoundationPlugin`]. It is **not**
stored on [`WorldData`].

## Instance storage (U2)

[`WorldData`] stores units parallel to doodads (ADR-015, ADR-017):

```text
units: HashMap<ChunkId, ChunkUnitStore>
unit_locations: HashMap<UnitId, ChunkId>   // required O(1) index
next_unit_id: u64
```

- Chunk-local iteration via [`ChunkUnitStore`] (sorted by [`UnitId`])
- Units may exist when terrain [`ChunkData`] is not resident
- Evicting terrain does **not** remove unit records
- Empty chunk stores are removed after the last unit leaves

[`UnitRecord`] fields: `id`, `definition_id`, `placement`, `state`, `source`,
`metadata`. No faction ownership on the record.

## Definition vs runtime separation

[`UnitDefinition`] describes **what** a unit type is:

- Identity (`UnitDefinitionId` from Excel `Unit ID`)
- Display name, stats (imported now; combat consumes later)
- Locomotion tuning (`move_speed_mps`, `collision_radius_meters`, `max_slope_degrees`)
- Presentation (`UnitRenderKey` from `File Path`)
- `faction_tag` — **content metadata only**

[`UnitDefinition`] does **not** represent runtime ownership. [`UnitRecord`]
instances do **not** copy faction as authoritative ownership. Dynamic
affiliation uses separate runtime ids (`OwnerId`, `TeamId`, `AffiliationId` or
equivalent) on instance/simulation state — deferred past U2.

## Simulation state naming

Do not use movement-only state types (e.g. `UnitMovementState` as the top-level
instance state). [`UnitRecord::state`] is [`UnitState`] — a broad placeholder
(`Idle` in U2). Future orders, combat, and AI extend via [`UnitSimulationState`]
subfields; movement is one concern among many.

## ECS is not authoritative

Runtime entities (U3+) are derived from [`UnitRecord`] and disposable. Queries,
persistence, and simulation read [`WorldData`] first — not ECS components.

## Excel import ownership

Offline import lives in `src/data_import/unit/` (feature `data-import`):

```text
Excel Units sheet → UnitImportRow → validate → UnitDefinition → UnitCatalog
```

Rules:

- Column names only; order irrelevant
- `Enabled=false` rows excluded at import
- `Total Stats` ignored (computed workbook column)
- All stat columns preserved on [`UnitDefinition`] even when unused
- Optional locomotion columns default when absent from legacy sheets
- Dev startup: import `Chasma Design.xlsx`; on failure warn and use starter catalog

Stat columns (STR, DEX, CON, AGI, CHR, INT, PER when added) are **design inputs** per
[ADR-070](ADR-070-progression-and-attributes.md). Workbook `Level` is authoring metadata —
runtime progression will be use-based skills, not global level.

## Obstacle and navigation (future)

Obstacle caches and pathfinding grids are **world systems**, not unit submodules.
Future ownership:

```text
src/world/navigation/   or   src/world/obstacle/
```

Do not place obstacle caches under `src/world/unit/`.

## Query seam (future)

Reserved module: `src/world/unit/query.rs`

Future queries (U3+):

- `units_near(position, radius)`
- `units_in_chunk(chunk)`
- `nearest_unit(position)`
- friendly/enemy filters via runtime affiliation ids

Queries read authoritative world data, not ECS render entities.

# Module layout

```text
src/world/unit/
    mod.rs
    id.rs
    placement.rs
    source.rs
    metadata.rs
    state.rs
    record.rs
    store.rs
    authoring.rs
    catalog/
        ...
    query.rs          # reserved, documented only
```

# Consequences

- **Positive:** Matches proven doodad instance + index pattern
- **Positive:** Clear seam for runtime, navigation without U2 scope creep
- **Neutral:** Legacy workbook rows without locomotion columns receive defaults
- **Deferred:** ECS sync, movement, pathfinding, rendering, save/load, affiliation ids

# References

- ADR-015 (doodad instance ownership)
- ADR-016 (catalog pattern)
- ADR-005 (future queries)
- ARCHITECTURE Principle 6 (Data First)

[`UnitCatalog`]: ../src/world/unit/catalog/registry.rs
[`UnitDefinition`]: ../src/world/unit/catalog/definition.rs
[`WorldData`]: ../src/world/data.rs
[`UnitRecord`]: ../src/world/unit/record.rs
[`UnitId`]: ../src/world/unit/id.rs
[`ChunkUnitStore`]: ../src/world/unit/store.rs
[`UnitState`]: ../src/world/unit/state.rs
[`WorldFoundationPlugin`]: ../src/world/mod.rs
