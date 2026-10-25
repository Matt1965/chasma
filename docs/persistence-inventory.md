# Authoritative persistence inventory (Chunk 1A)

This matrix classifies simulation state for player/default-world saving work.
It is normative for Chunk 1B+ implementation. **Dev scene v19** is a partial,
normalized snapshot format — not the authority model for failure safety or
future player saves.

Legend:

- **Persist**: must round-trip in a full-world player save (future chunks).
- **Package**: immutable or revisioned under `assets/worlds/main` (reference by revision).
- **Rebuild**: derived inside `WorldData` or ECS; omitted from saves, recomputed on load.
- **Transient**: session/scratch; cleared between ticks or on load; not saved.
- **Client**: client-local; never in `WorldData` saves.

## Core session authority (transaction boundary)

| Field / store | Location | Chunk 1B | Notes |
| --- | --- | --- | --- |
| `WorldData` (full graph) | `world::WorldData` | Persist (snapshot) | Transaction backup clones entire resource |
| `SimulationControlState::current_tick` | `simulation::control` | Persist | Outside `WorldData`; included in session backup |
| `SimulationClock::accumulator_seconds` | `simulation::control` | Transient | Reset on load; not gameplay truth |
| `SimulationControlState::paused` / `step_once` | client session | Client | Restored by menu/session layer (Chunk 3) |

## WorldData — terrain and package-bound

| Field / store | Persist | Package | Rebuild | Transient | Notes |
| --- | --- | --- | --- | --- | --- |
| `chunks` (heightfields) | Ref | Yes | — | — | Streamed from world package; reference package revision |
| `extent` / layout | Ref | Yes | — | — | From `WorldConfig` + package manifest |
| `terrain_fields` (base tiles) | Ref | Yes | — | — | Bootstrap from package manifest |
| `terrain_field_modifiers` | Yes | — | — | — | Mutable playthrough/editor state (ADR-106 seam) |
| `biome_mask` | Ref | Yes | — | — | Dev import / package; not in dev scene today |

## WorldData — instances

| Field / store | Persist | Rebuild | Transient | Dev scene v19 | Notes |
| --- | --- | --- | --- | --- | --- |
| Units (`units`, `unit_locations`) | Yes | — | — | Partial | Scene omits vitals/combat; restore **normalizes** (1C) |
| Doodads | Yes | — | — | Yes | Procedural keys preserved |
| Buildings | Yes | — | — | Yes | Interiors/doors in building records |
| Projectiles | — | — | Yes | — | Cleared on dev load; must define tick boundary for saves |
| Corpses / item piles | Yes | — | — | Yes | Inventory persistence block |
| Inventories / item instances | Yes | — | — | Yes | Restore must use **runtime** catalogs (1C) |

## WorldData — simulation / AI / tasks

| Field / store | Persist | Rebuild | Transient | Dev scene v19 | Notes |
| --- | --- | --- | --- | --- | --- |
| `task_store` | Yes | — | — | Yes | Assigned work |
| `command_buffer` / movement smoothing | — | — | Yes | Cleared on dev load | Capture at tick boundary for saves |
| `removal_queue` / kill attributions | — | — | Yes | Cleared | |
| Unit orders / combat state / attack cycle | Yes | — | — | Partial / normalized | 1C fidelity |
| `building_production_store` | Yes | — | — | Yes | |
| Logistics / reservations / hauling | Yes | — | — | Yes | |
| Settlement planner / state / construction | Yes | — | — | Yes | |
| `relationship_standing_store` | Yes | — | — | Yes | Runtime standing edges |
| Unit `faction_id` / `species_id` | Yes | — | — | Partial | Scene fields exist; restore overwrites from catalog today (1C) |

## WorldData — settlements

| Field / store | Persist | Dev scene v19 | Notes |
| --- | --- | --- | --- |
| `settlement_store` / anchors / treasuries | Yes | Yes | Membership indexes rebuilt |
| Settlement membership on units/buildings | Yes | Yes | |

## WorldData — indexes and runtime caches (inside WorldData)

| Field / store | Rebuild | Notes |
| --- | --- | --- |
| Occupancy grids | Rebuild | `rebuild_building_world_indexes` |
| Building navigation runtime | Rebuild | `reconcile_*_navigation_*` |
| Logistics endpoint index | Rebuild | |
| `locomotion_surfaces` | Rebuild | |
| Instance location indexes | Rebuild | `verify_instance_indexes` after restore |

## Outside WorldData (resources)

| Resource | Persist | Package | Client | Notes |
| --- | --- | --- | --- | --- |
| `RoadNetwork` | Yes (mutable) | Default in package | — | Separate from scene; road editor writes package |
| Road deformation store | Yes | Partial | — | Package-backed bake |
| Catalogs (`UnitCatalog`, items, buildings, …) | Ref | Yes | — | Missing definition = load error |
| `AppearanceProfileCatalog` | Ref | Yes | — | Required for inventory/equipment restore |
| `DoodadsRuntimeSettings` (seed) | Session | — | — | Scene stores seed metadata |
| Selection, HUD, trade/dialogue UI | — | — | Client | Chunk 3 teardown |
| Portrait / render caches | — | Rebuild | ECS | |
| Dev preview / procgen ledgers | — | — | Transient | Must not run during menu (Chunk 3) |

## Dev scene loader behavior (not player policy)

`dev_clear_units_and_doodads` clears instances **and** many stores (inventory, tasks, logistics, etc.) before restore. That is a **dev implementation strategy**, not proof those domains are disposable for player saves.

## Failure safety (Chunk 1B)

Full-session backup = `WorldData` clone + `simulation_tick`. **Not** `SceneDefinition` capture/reapply. Rollback restores authoritative session; ECS/presentation may be stale until runtime sync (acceptable).

## Chunk 1B implementation boundaries (done)

| API | Location | Role |
| --- | --- | --- |
| `AuthoritativeSessionSnapshot` | `persistence::session` | Clone/restore world + tick |
| `transactional_session_load` | `persistence::transaction` | Roll back on `Err` |
| Dev adapter | `dev::scenes::apply_scene` | Scene plan + transaction wrapper |

**Not in 1B:** player save format, menu/session teardown, autosaves, baseline migration, scene fidelity (1C), save-root sharing (open until chunk 2).

**Roadmap follow-ups:** Chunk 1C (scene restore fidelity, faction/species preservation policy in loader), Chunk 2 (save root), Chunk 3 (session resources), Chunk 7 (default-world revision on save).
