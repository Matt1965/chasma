# ADR-079: Building Runtime Foundation (B2)

# Status

Accepted (B2 — authoritative building instances)

# Context

ADR-078 established building **type definitions** and the Excel/RON catalog pipeline.
Gameplay systems (placement, construction, occupancy, navigation) require authoritative
**instances** on [`WorldData`] before those phases can consume them.

# Decision

## WorldData owns building instances

[`BuildingRecord`] lives in chunk-keyed [`ChunkBuildingStore`] with a required
[`BuildingId`] → [`ChunkId`] index, mirroring units and doodads (ADR-027 U2, ADR-015).

| Concern | Owner (B2) |
|---------|------------|
| Building type definitions | [`BuildingCatalog`] (B1) |
| Building instances | [`WorldData`] via [`BuildingRecord`] |
| Render entities | `src/buildings/` runtime layer (disposable) |

## BuildingRecord (runtime only)

Each record stores:

- `id`, `definition_id`, `placement`, `ownership`, `current_hp`
- `lifecycle_state`, `spaces` (empty), `construction` (empty placeholder)
- `source` (`Authored` / `Dev`)

Catalog fields (footprint spec, build time, render keys) remain on
[`BuildingDefinition`].

## Authoring API

[`create_building`], [`move_building`], [`remove_building`], [`lookup_building`]
validate against [`BuildingCatalog`] and mutate [`WorldData`] only — no ECS.

## Runtime sync

`BuildingsRuntimePlugin` registers `BuildingRuntimeSystems` inside
`RuntimeSyncSystems` (after doodads, before units).

`sync_building_render_entities`:

1. Collects visible building ids from **resident** terrain chunks.
2. Despawns stale render entities when chunks unload.
3. Spawns/updates placeholder cuboid meshes sized from catalog footprint.
4. Colors meshes by runtime `ownership.affiliation`.

Presentation is intentionally disposable; [`WorldData`] is never modified for render.

## Dev mode (B2 scope)

- **Buildings** catalog tab + dev spawn via existing brush/batch tools.
- Inspector Alt+click picks building render entities.
- No player placement UI, no scene save/load for buildings yet.

## Explicitly deferred

- Placement validation / ghost preview for players (B3+)
- Construction simulation (`lifecycle_state` seam only)
- Occupancy baking and navigation (B3+)
- Interiors, doors, destruction, worker tasks

# Consequences

- B3 can add placement rules consuming footprint data from definitions.
- B4 can populate `construction` and drive `lifecycle_state`.
- Occupancy baker reads `collision_render_key` from definitions, not records.

[`WorldData`]: ../src/world/data.rs
[`BuildingRecord`]: ../src/world/building/record.rs
[`BuildingId`]: ../src/world/building/id.rs
[`ChunkBuildingStore`]: ../src/world/building/store.rs
[`BuildingCatalog`]: ../src/world/building/catalog/registry.rs
[`BuildingDefinition`]: ../src/world/building/catalog/definition.rs
[`create_building`]: ../src/world/building/authoring.rs
