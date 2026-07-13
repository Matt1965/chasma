# ADR-086: Building Persistence, Scaling, and Architecture Freeze

## Status

Accepted (B9)

## Context

B1–B8 established building definitions, authoritative `BuildingRecord`s, occupancy, build mode, construction/vitals/ruins, spaces/portals/visibility, interiors/doors, and worker tasks. B9 must stabilize persistence, derived-index rebuild, and freeze the building architecture for downstream gameplay (economy, hauling, production).

## Decision

### Scene format v5 (dev persistence seam)

- `SCENE_VERSION = 5` extends ADR-045 scenes with:
  - `task_records` (`SceneTaskRecord`)
  - runtime ID counters: `next_task_id`, `next_door_id`, `next_space_id`, `next_portal_id`
  - `SceneUnitState::Working { task_id }`
- v4 scenes remain loadable; missing task/counter fields default safely.
- Missing building definitions **fail load** (no silent fallback).

### Authoritative restore validation

- `validate_building_for_restore` mirrors unit/doodad restore policy (definition exists/enabled, vitals, progress, duplicate IDs).
- Task restore validates building and assigned-unit references before mutation.
- `SceneApplyError` remains structured; failed loads roll back via `DevWorldEntityBackup`.

### Derived index rebuild API

- `rebuild_building_world_indexes(world, building_catalog, footprint_catalog, doodad_catalog, tick)`:
  1. `rebuild_occupancy_index`
  2. `sync_construction_tasks`
  3. `prune_invalid_building_tasks`
  4. `verify_instance_indexes` (dev/test builds)
- Occupancy grids, passability caches, portal graphs, and task indexes are **not serialized**; they are rebuilt on load.

### ID allocator safety

- After restore: `dev_restore_id_counters` + `dev_restore_building_runtime_counters` advance `TaskStore`, `DoorStore`, and `SpaceRegistry` allocators to `max(restored, scene.next_*)`.

### Catalog policy on load

| Condition | Policy |
|-----------|--------|
| Missing definition | Fail load |
| Disabled definition | Fail load |
| Missing render asset | Authoritative load succeeds; presentation may use static proxy |
| Changed footprint | Rebuild occupancy from **current** catalog (dev scenes; production saves will require explicit migration ADR) |

### Runtime presentation (multi-chunk)

- One render entity per `BuildingId` when anchor or any footprint-intersected chunk is resident (`BuildingsRuntimePlugin` single-owner index).
- Authoritative doors/tasks/construction continue when presentation is absent.
- LOD beyond residency gating is deferred until profiling warrants impostors.

### Architecture freeze assertions

- `BuildingDefinition` / catalogs own content truth.
- `BuildingRecord` on `WorldData` owns instance truth.
- Occupancy is derived, not render truth.
- Spaces/portals own multi-level navigation; `ActiveViewedSpace` is client presentation.
- `DoorRecord` owns door passability state.
- Construction advances only through fixed-tick worker labor (`step_all_worker_tasks`).
- Tasks/reservations are authoritative on `TaskStore`.
- ECS is disposable presentation.
- Save/load reconstructs derived indexes via `rebuild_building_world_indexes`.
- Below-surface `SpaceId` values and portal transitions are supported without architectural rewrite.

## Consequences

- Dev scene save/load uses runtime Excel-resolved catalogs (`BuildingCatalog`, `FootprintCatalog`, `InteriorProfileCatalog`), not `::default()` starters.
- Production world serialization reuses the same authoritative structures when Phase 7 lands; B9 does not invent an isolated building-only save system.
- ADR-078–085 remain authoritative for their domains; this ADR adds persistence/rebuild/freeze only.

## Deferred (explicit)

Full resource economy, inventories, hauling, recipes/production, room bonuses, upgrades/repair, destructible wall pieces, underground excavation/content, advanced scheduler AI, building impostors beyond residency policy, power networks.
