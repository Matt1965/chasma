# ROADMAP.md

# Purpose

This document defines implementation order.

It does not define final project scope.

The final project vision is described in ARCHITECTURE.md.

Game design goals and planned mechanics are described in [DESIGN.md](DESIGN.md).

Future systems may appear in ARCHITECTURE.md or DESIGN.md long before they appear in this roadmap.

---

# Review Closure Status (REVIEW-CLOSE, July 2026)

Pre-feature-development checkpoint after audit passes A1–B6.

## Implemented foundations (not “complete game”)

| Area | Status |
|------|--------|
| WorldData authority + chunk terrain heightfields | Foundation complete; runtime validation hardened (B6) |
| Terrain streaming / LOD / materialization | Runtime path complete; fail-closed validation |
| Doodad procedural + obstacle queries | Foundation complete; fail-closed obstacles (B6) |
| Units: movement, orders, grounding, death | Foundation complete; ADR-066 outcomes |
| Navigation (grid A*) | Foundation complete; consumes obstacle layer |
| Client intent → command pipeline | Complete for current command set |
| Selection, ownership, controllability | Foundation complete |
| Formations / steering / movement feel | Foundation complete |
| Weapons / combat engagement / strikes | Foundation complete; projectile path present |
| Combat AI (auto-acquire) | Basic foundation |
| Health bars | Presentation sync |
| Unit locomotion animation (Idle/Walk/Run) | A1 foundation (ADR-074) |
| Weapon-driven combat animation | A2 (ADR-074) |
| Death presentation + hit reactions | A3 (ADR-074) |
| Animation layering (lower/upper body) | A4 (ADR-075) |
| Advanced locomotion polish (turns, hysteresis, speed blends) | A5 (ADR-076) |
| Animation LOD, validation, shared graphs, audit stabilization | A6 + A1 (ADR-077) |
| Fixed simulation tick orchestrator | ADR-065 in place |
| Gameplay presentation polish (DV3) | Move destination validation, billboards, terrain rings, shadow cascades — `docs/presentation-dv3.md` |
| Dev Mode (catalog, spawn, scenes, inspector) | Dev-gated; DV2 focus/cancel UX; locomotion profiles + weapons from Excel; A3–A5 profile fields via starters/code |
| Environment (time-of-day, water, lighting) | Dev/runtime presentation; singleton-safe |

## Explicitly deferred

- Overlay animation layer behavior (hit VFX on overlay slot — future per ADR-075)
- Corpse fade-out / lootable corpse presentation
- Economy, buildings, harvesting loops (see [ADR-072](ADRs/ADR-072-settlement-automation-and-production.md), DESIGN.md)
- Full creature AI template stack ([ADR-071](ADRs/ADR-071-creature-ai-architecture.md); ADR-062 scan AI only)
- Use-based skills and attribute-driven combat ([ADR-070](ADRs/ADR-070-progression-and-attributes.md))
- Grid inventory and equipment ([ADR-073](ADRs/ADR-073-inventory-and-equipment.md); I1 catalog foundation in [ADR-087](ADRs/ADR-087-item-definitions-and-inventory-profiles.md))
- Downed state, stagger, facing, weapon hit volumes ([ADR-069](ADRs/ADR-069-combat-design-philosophy.md))
- Full pathfinding optimizations (pooling, hierarchical)
- Multiplayer replication
- Production Excel pipeline outside `feature = "dev"`
- Combat polish (VFX, advanced AI)

## Recommendation

**Ready for feature development** with non-blocking caveats listed in `docs/reviews/REVIEW-CLOSE-feature-readiness.md`.

---

Implementation order exists to reduce architectural risk and avoid building systems on unstable foundations.

---

# Current Development Philosophy

The project should be built from the bottom up.

Priority order:

1. World foundations
2. World data
3. Terrain runtime
4. Doodad runtime
5. Query systems
6. Authoring support
7. Simulation foundations
8. Gameplay systems

Do not implement higher-level systems before lower-level dependencies exist.

---

# Phase 0 — Foundation

## Goal

Establish project structure and architectural boundaries.

## Deliverables

- Application structure
- Plugin structure
- Chunk Coordinate + Local Position world coordinate model
- Chunk identifiers
- World configuration
- Basic project layout

## Success Criteria

- Project compiles
- Plugin boundaries exist
- Core architecture is established

## Notes

No gameplay systems.

No terrain rendering.

No simulation systems.

Focus entirely on ownership and structure.

---

# Phase 1 — World Data Layer

## Goal

Create authoritative world data structures.

## Deliverables

- Chunk definitions
- Terrain data structures
- Heightfield loading
- Terrain metadata structures
- Terrain mask structures
- World configuration data

## Success Criteria

- Terrain data loads successfully
- World data can be queried
- Rendering is not required

## Notes

Heightfield data is authoritative.

Terrain meshes do not exist yet.

---

# Phase 2 — Terrain Runtime

## Goal

Render terrain from authoritative world data.

## Deliverables

- Terrain mesh generation
- Chunk streaming
- Terrain chunk lifecycle
- Terrain LOD system
- Basic far terrain representation

## Success Criteria

- Terrain is visible
- Chunk streaming works
- Terrain LOD works
- High-altitude viewing is functional

## Notes

Focus on correctness and scalability.

Avoid renderer-specific complexity.

---

# Phase 3 — Doodad Runtime

## Goal

Support environmental world objects.

## Deliverables

- Authored doodads
- Procedural doodads
- Exclusion zones
- Chunk ownership of doodads
- Doodad streaming
- Doodad instancing

## Success Criteria

- Procedural forests can exist
- Authored landmarks can exist
- Exclusion zones prevent overlap
- Doodads stream correctly

## Notes

Doodads are primarily visual.

Avoid premature gameplay integration.

---

# Phase 4 — World Query Layer

## Goal

Create stable interfaces for future systems.

## Deliverables

- Chunk lookup queries
- Terrain height queries
- Terrain slope queries
- Terrain normal queries
- Doodad queries
- Chunk loaded-state queries
- Occupancy query interfaces

## Success Criteria

Future systems can interact with the world without knowing implementation details.

## Notes

This phase is more important than it appears.

Future gameplay systems should depend on queries rather than world internals.

Initial public query API should stay small.

Avoid speculative queries that do not have clear consumers.

---

# Phase 5 — Minimal Authoring

## Goal

Support manual world creation.

## Deliverables

- Place authored doodads
- Move authored doodads
- Rotate authored doodads
- Scale authored doodads
- Save authored placements

## Success Criteria

A user can create:

- campsites
- ruins
- villages
- landmarks

without editing source terrain data.

## Notes

This is not a full editor.

Terrain authoring remains external.

---

# Phase 6 — Occupancy Layer

## Goal

Support dynamic world modification.

## Deliverables

- Occupancy data structures
- Building footprints
- Dynamic blockers
- Occupancy queries

## B1 — Building definitions (complete)

- `BuildingDefinition` / `BuildingCategoryDefinition` catalogs
- Excel sheets: `Buildings`, `Building Categories`
- Dev import + `assets/buildings/catalog.ron` export
- ADR-078

**Not in B1:** placement, rendering, construction runtime, occupancy baking,
navigation changes.

## B2 — Building runtime foundation (complete)

- `BuildingRecord` / `BuildingId` / `ChunkBuildingStore` on `WorldData`
- Authoring API: `create_building`, `move_building`, `remove_building`
- `BuildingsRuntimePlugin` — residency-gated render sync (cuboid placeholders superseded by BA1)
- Dev Mode: Buildings tab, batch spawn, inspector pick
- ADR-079

**Not in B2:** player placement, occupancy, construction simulation, navigation,
interiors, destruction.

## BA1 — Building runtime GLB assets (complete)

- `BuildingSceneAssets` shared glTF cache (`assets/buildings/{render_key}.glb`)
- `SceneRoot` spawn with async load gating; lifecycle tint on construction states
- Diagnostic magenta fallback for missing/failed assets (never neutral cubes)
- Build mode GLB ghost + footprint gizmo overlay
- Dev inspector asset diagnostics; optional `space:` / `roof:` scene tag discovery
- ADR-095

**Not in BA1:** per-lifecycle GLB variants, roof hide wiring, impostor LOD, new building mechanics.

## BP-CLEANUP — Placement transform and dev Complete spawn (complete)

- Fine 0.1 m anchor quantization (continuous feel, discrete occupancy)
- `BuildingPlacementPlan` shared by ghost, validation, and commit
- `building_model_render_transform` for ghost + runtime GLB alignment
- `model_local_offset` / `model_yaw_correction_degrees` on definitions
- `create_dev_complete_building` — Complete, full HP, progress 1.0
- ADR-096

**Not in BP-CLEANUP:** arbitrary rotation, terrain flattening, bake format migration.

## B3 — Generalized occupancy and baked footprints (complete)

- Shared `src/world/occupancy/` module: footprints, 2 m cells, chunk occupancy index
- `FootprintShape`: Circle, Rectangle, offline `BakedCellMask`
- Offline collision baker (`occupancy_collision` GLB node; `data-import` feature)
- Registration lifecycle on `WorldData::occupancy` (buildings + doodads)
- Composed [`query_passability_at`] (terrain → slope → static occupancy)
- Navigation and movement consume passability (no A\* rewrite)
- Doodad blocking migrated from parallel obstacle circles to footprint queries
- ADR-080

**Not in B3:** player build mode, ghosts, construction-state occupancy, spaces/portals,
doors/stairs, underground, runtime mesh rasterization, dynamic unit occupancy grid.

## B4 — Player build mode and ghost validation (complete)

- Client-local `BuildModeState` + build catalog HUD (`B` toggle)
- Terrain-snapped ghost with footprint gizmo + validity colors
- `validate_building_placement` pure world API
- `ClientIntent::PlaceBuilding` commit with revalidation
- `place_player_building` → `BuildingLifecycleState::Planned` + `OccupancyState::Reserved`
- ADR-081

**Not in B4:** construction workers, resource costs, spaces/portals, terrain flattening,
building relocation, demolition UI.

## B5 — Building construction lifecycle, vitals, and ruins (complete)

- `BuildingLifecycleState`: Planned, Foundation, InProgress, Complete, Destroyed, Ruins
- `BuildingVitals` + `ConstructionState.progress_0_1` on `BuildingRecord`
- `step_all_worker_tasks` applies construction labor from assigned workers (ADR-085 B8); `step_all_building_construction` auto-progress is dev-gated only
- `damage_building` / `heal_building` / `destroy_building` / `transition_to_ruins`
- `is_building_operational` gate for future production
- Occupancy-by-state policy (Planned/Ruins reserved; construction stages blocked)
- Runtime lifecycle tint sync; player building HUD panel; dev inspector actions
- Dev scene v2 building snapshot/restore with occupancy rebuild
- ADR-082

**Not in B5:** worker AI, resource costs, repair tasks, upgrades, demolition refunds, production.

## B6 — Navigable spaces, portals, stairs, and automatic interior visibility (complete)

- `SpaceId` / `PortalId` model with `SpaceRegistry` on `WorldData`
- Canonical `SpaceId::SURFACE` exterior; building spaces registered on hut completion
- `UnitRecord.current_space_id` authoritative; portal transition with hysteresis
- `NavigationWaypoint` with `space_id`; `find_path_with_spaces` cross-space grid A*
- `query_passability_in_space` and `ground_position_in_space` per-space grounding
- Client `ActiveViewedSpace` auto-follows primary selected unit; `ViewFollowLock` optional
- Dev inspector space/floor display; scene v3 `current_space_id` persistence
- ADR-083

**Not in B6:** room simulation, door state (B7), underground content, manual up/down UX, navmesh,
building stacking, roof mesh tagging (placeholder buildings only).

## B7 — Building interiors, doors, and interior object integration (complete)

- `InteriorProfileCatalog` with spaces, portals, doors, and child placements per building type
- `DoorRecord` / `DoorStore` on `WorldData`; portal `enabled` derived from `DoorState`
- Interior activation on **Complete**; deactivation on ruins/destruction/removal
- Child **Doodads** (scenery) and child **Buildings** (functional) with parent linkage
- `SpaceRecord.room_tag` metadata seam (no room simulation)
- Movement auto-opens authorized closed doors; pathfinding door-aware routes
- Scene v4 door/interior persistence; dev inspector door shortcuts (O/L)
- ADR-084

**Not in B7:** worker tasks, production, storage inventory, room bonuses, procedural furnishing,
destructible wall pieces, door opening time simulation, underground content.

## B8 — Building interactions, tasks, and construction labor (complete)

- `TaskStore` / `TaskRecord` on `WorldData` (ADR-085)
- `BuildingInteractionProfile` with construction and workstation points
- `step_all_worker_tasks` applies construction labor; auto-timed progress dev-gated only
- Player construct/operate orders via interaction dispatcher
- `UnitState::Working { task_id }` and reservation semantics

## B9 — Building persistence, performance baseline, and architecture freeze (complete)

- Scene format **v5**: tasks, runtime ID counters, `Working` unit state
- `validate_building_for_restore` + atomic scene apply with catalog validation
- `rebuild_building_world_indexes` canonical derived-index rebuild
- Dev scene load uses runtime `BuildingCatalog` / `FootprintCatalog` / `InteriorProfileCatalog`
- ADR-086; readiness report `docs/reviews/BUILDINGS-B9-READINESS.md`

**Not in B9:** production world save, economy, impostor LOD, underground content, advanced scheduler.

## Success Criteria

The world can represent structures that affect movement and placement.

## Notes

Occupancy must remain separate from terrain.

Occupancy is expected to change.

Terrain is mostly static.

---

# Phase 7 — Persistence Foundation

## Goal

Support permanent world changes.

## Deliverables

- Runtime overrides
- Persistent modifications
- World state saving
- World state loading

## Success Criteria

Changes survive reloads.

Examples:

- harvested resources
- constructed buildings
- destroyed objects
- authored modifications

## Notes

Procedural generation creates the baseline.

Persistent state becomes the authority.

---

# Phase 8 — Simulation Foundation

## Goal

Support world objects that exist independently of rendering.

## Deliverables

- Simulation object framework
- Promotion system
- Demotion system
- Abstract simulation objects
- Persistent simulation state

## Success Criteria

Objects can exist without:

- rendering
- physics
- local gameplay simulation

## Notes

This phase establishes the foundation required for future Kenshi-style systems.

No advanced simulation is required yet.

---

# Runtime Foundation Milestone

The runtime foundation is considered complete when the project supports:

- External terrain import
- Chunk streaming
- Terrain LOD
- Infinite-style world visibility
- Authored doodads
- Procedural doodads
- Exclusion zones
- World queries
- Occupancy
- Persistence
- Simulation object foundations

At this point the project has achieved its immediate architectural goal.

---

# Future Systems

The following systems are intentionally deferred until the runtime foundation exists.

These are future project goals.

They are not current implementation targets.

## Settlements

Design direction: [ADR-072](ADRs/ADR-072-settlement-automation-and-production.md), [DESIGN.md](DESIGN.md#settlement-automation).

- Professions with priority fall-through (not per-task micromanagement)
- Buildings generate **tasks**; workers claim by profession
- Production via building **requests** (Factorio-style logistics, individual workers)
- Direct player orders override automation temporarily

Potential systems:

- population
- needs
- assignments / professions
- production and storage
- ownership

---

## Resources

Design direction: [DESIGN.md](DESIGN.md#world-and-food) (staple crops and prepared foods).

- Alien biology, recognizable food economy (Brim Grain, Knot Tubers, Glass Pods, etc.)
- Harvesting, depletion, regrowth, extraction

---

## Units

Design direction: [ADR-070](ADRs/ADR-070-progression-and-attributes.md), [ADR-073](ADRs/ADR-073-inventory-and-equipment.md).

**Implemented foundations:** movement, orders, selection, combat engagement, death.

**I1 — item definitions and inventory profiles (ADR-087):** complete

- `ItemDefinition`, `ItemCategoryDefinition`, `InventoryProfileDefinition` catalogs
- Excel import (`Items`, `Item Categories`, `Inventory Profiles`) in dev builds
- Physical gold item definition; optional unit/building `inventory_profile_id`
- Dev Items browser (read-only)
- Runtime inventory deferred to I2

**I2 — authoritative inventory grid and item identity (ADR-088):** complete

- `InventoryStore`, `ItemInstanceStore`, `InventoryRecord` on `WorldData`
- Fixed-grid placement, stacks, unique instances, atomic ops, soft weight, auto-sort
- Profile migration with explicit leftovers
- Dev inventory harness (detached inventories only)

**I3 — unit inventories, corpse ownership, weight (ADR-089):** complete

- `UnitRecord.inventory_id`; `create_unit_with_inventory` attaches profile inventories
- Death transfers same `InventoryId` to `CorpseRecord` via `finalize_unit_removal`
- Fixed-tick corpse lifetime + expiration deletes inventory (no ground spill until I4 spill API)
- Soft weight query seam; `SimulationCatalogParams` for tick integration
- I3 unit/corpse/inventory tests in `world::unit::inventory::i3_tests`

**I4 — cross-inventory transfers, world piles, drop/pickup/loot (ADR-090):** complete

- Atomic `transfer_*` APIs with explicit full/one/half semantics and placement policies
- `WorldItemPileRecord` on `WorldData.item_pile_store`; deterministic merge/overflow
- `ItemInstanceLocation` index (`Inventory` | `WorldPile` | `Detached`)
- Drop, pickup, spill, corpse loot via shared transfer pipeline
- Runtime pile sync (`src/item_piles/`); dev pile harness on World Tools tab
- Tests in `world::item_pile::tests`

**I5 — building containers, access, destruction spill (ADR-091):** complete

- `BuildingRecord.inventory_id` + `InventoryOwnerRef::Building` at create time (lifecycle-gated access)
- `ContainerAccessPolicy`, `can_unit_access_inventory`, `InteractionType::Container`
- Destruction spill via I4 `spill_inventory_to_world_piles`; typed removal policies
- Starter `storage_chest` + interaction profile; dev building inspector container tools
- Tests in `world::building::inventory`

**I6 — player inventory UI, drag/drop, transfers (ADR-092):** complete

- `InventoryUiState` client presentation; `InventoryIntent` + `dispatch_inventory_intents`
- Modal inventory panel: grids, weight/gold, details, disabled equipment seam
- Open unit (**I**), interact container/corpse/pile; dual transfer + loot-all
- Right-click full, Ctrl one, Shift half, drag move/transfer, auto-sort button
- World input blocked while panel open; commit-time access + stale revision checks
- Tests in `client::inventory_dispatch`, `ui::gameplay::inventory`

**I7 — settlement treasuries, physical gold deposits (ADR-093):** complete

- `SettlementTreasuryRecord` abstract wealth on `WorldData::settlement_store`
- `deposit_gold` atomic physical removal + treasury credit; no withdrawals
- Settlement capability on `settlement_core` only (not chests)
- `InteractionType::Treasury`; treasury deposit UI (Physical Gold / Treasury Gold separate)
- Dev treasury harness; scene v6 persistence for settlements/treasuries
- Tests in `world::settlement`, `client::inventory_dispatch`

**I8 — inventory persistence, validation, audit (ADR-094):** complete

- Scene v7: inventories, instances, corpses, piles, allocators, unit `inventory_id`
- `rebuild_all_inventory_derived` on load; derived caches never serialized
- `validate_world_inventory_state` unified entry point
- Stress tests in `world::inventory::stress`
- Dev inspector inventory summaries; harness **V** → world validation
- Final audit: [docs/items-inventory-final-audit.md](docs/items-inventory-final-audit.md)

**Deferred:**

- use-based skills (no global runtime level; workbook `Level` is authoring metadata)
- attributes driving combat formulas (STR/DEX/CON/PER/AGI/CHR/INT — imported, not simulated)
- injuries and downed state
- equipment runtime, move-to-loot orders
- progression and equipment

---

## Reputation

Potential future systems:

- personal reputation
- faction reputation
- species reputation
- regional reputation
- event-based consequences
- delayed reporting
- local knowledge

---

## Factions

Potential future systems:

- diplomacy
- ownership
- influence
- territory

---

## Routes

Potential future systems:

- roads
- trade routes
- caravan routes

---

## Caravans

Potential future systems:

- trade
- transportation
- logistics

---

## Combat

Design direction: [ADR-069](ADRs/ADR-069-combat-design-philosophy.md), [DESIGN.md](DESIGN.md#combat).

**Implemented foundations:** weapons, orders, range/chase, strikes, projectiles, basic AI acquire, death pipeline.

**Target experience:** Warcraft III tactical engagements — not SC2 lethality.

**Deferred design items:**

- min/max weapon envelope and player-vs-AI reposition policy
- facing and weapon-origin hit volumes
- stagger, knockdown, enemy CC (player cannot cancel)
- downed state replacing instant death
- controlled randomness (misses, damage ranges, crits)
- tiered target selection for AI
- full attack-move pursue/resume semantics

---

## Multiplayer

Potential future systems:

- replication
- synchronization
- authority management

---

# Roadmap Rule

Do not skip foundational phases to implement future systems.

When evaluating a feature request:

Determine which roadmap phase owns the required functionality.

If the supporting phase is incomplete, prioritize the foundation before the feature.

The project should grow upward from stable foundations rather than outward through feature accumulation.