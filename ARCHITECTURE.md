# ARCHITECTURE.md

# Project Identity

This project is a **Bevy 0.18 large-world runtime and simulation foundation**.

The immediate goal is not to build a complete game.

The immediate goal is to build the foundational systems required for a future large-world
simulation game combining **Kenshi-inspired world survival and attachment** with
**Warcraft III-style tactical combat** — while remaining flexible enough to support future
mechanics that have not yet been designed.

Game design goals, combat philosophy, progression, AI, economy, and food systems are
documented in [DESIGN.md](DESIGN.md). Accepted design directions also appear as ADRs
069–073.

The project is first and foremost a large-world runtime:

> external terrain data in → streamed world out

The architecture should prioritize:

- long-term extensibility
- large-world scalability
- clean data ownership
- simulation independence from rendering
- performance-conscious design
- minimal future rewrites

Systems should be designed around generalized world concepts rather than individual game mechanics.

---

# Immediate Goal

Build a large-world runtime capable of:

- importing externally-authored terrain data
- streaming terrain by chunk
- rendering terrain using multiple LOD levels
- supporting effectively infinite view distance
- supporting authored doodads
- supporting procedural doodads
- supporting authored exclusion zones
- supporting future gameplay queries
- maintaining stable performance while traversing large worlds

This is the primary objective.

All future considerations exist only to prevent architectural dead ends.

They should not drive implementation unless required.

---

# Performance Philosophy

Performance is a primary architectural concern.

This project targets:

- large terrain datasets
- long view distances
- chunk streaming
- large numbers of doodads
- persistent world simulation
- future settlement systems
- future unit systems

As a result, systems should be designed around scalability from the beginning.

Performance considerations should influence architecture decisions, but should not prematurely drive low-level optimizations.

Preferred approach:

1. Design scalable data ownership.
2. Minimize unnecessary simulation.
3. Minimize unnecessary ECS entities.
4. Stream and materialize detail only when needed.
5. Profile before introducing complexity.

Avoid assuming that future optimization can solve poor architecture.

Prefer systems that naturally scale as world size increases.

---

# Scalability Rule

The existence of an object and the detailed simulation of an object are separate concerns.

The architecture should support:

- Existing but unloaded
- Existing but abstractly simulated
- Existing and fully simulated
- Existing and rendered

Not every object should occupy the most expensive state at all times.

This principle applies to:

- terrain chunks
- doodads
- units
- caravans
- resources
- settlements
- future simulation systems

---

# Core Architecture Principles

## Principle 1: Simulation Is The Authority

Simulation state is the source of truth.

Rendering is a representation of simulation state.

Gameplay entities are a representation of simulation state.

Nothing should exist solely because it is rendered.

---

## Principle 2: Chunks Are Geography

Chunks represent world geography.

Chunks do not own:

- factions
- settlements
- caravans
- units
- economies
- reputation systems

Chunks own:

- terrain data
- terrain metadata
- masks
- occupancy metadata
- authored world content
- procedural world content

Important gameplay systems may reference chunks but should not depend on chunks being visually loaded.

---

## Principle 3: Rendering Is Replaceable

World data must never depend on rendering implementation.

Future renderer upgrades should not require world rewrites.

Examples:

- StandardMaterial
- custom materials
- instancing
- impostors
- GPU-driven rendering

All rendering systems should consume world data.

World systems should not consume renderer data.

---

## Principle 4: Queries Are Stable

Future systems should interact with the world through query interfaces.

Initial query categories should include:

- chunk lookup
- terrain height lookup
- terrain slope lookup
- terrain normal lookup
- nearby doodad lookup
- chunk loaded-state lookup

Future query categories may include:

- occupancy lookup
- placement validation
- settlement lookup
- route lookup
- simulation-object lookup

Gameplay systems should never need direct access to terrain mesh entities.

---

## Principle 5: Promote Detail Near The Player

The world should exist at multiple levels of detail.

Far away:

- simulation records
- doodad instance data
- abstract world state

Near the player:

- ECS entities
- physics
- interactions
- animations

This principle applies to:

- doodads
- units
- caravans
- resources
- future simulation objects

---

## Principle 6: Data First

Systems should operate on persistent data structures whenever possible.

Entities are implementation details.

Persistent game concepts should have persistent data representations.

Examples:

- units
- settlements
- caravans
- resources
- factions

These concepts should not exist solely as ECS entities.

ECS entities are temporary manifestations of persistent world state.

---

# World Coordinate Model

The authoritative world position model is:

- Chunk Coordinate
- Local Position

World positions should not rely on large global floating-point coordinates as the source of truth.

The authoritative position representation should support:

- large worlds
- chunk streaming
- persistence
- future multiplayer
- future simulation systems

Rendering systems may use local floating-point transforms as needed.

Simulation and world data systems should use chunk-relative positions as the source of truth.

---

# Terrain Data Requirements

The terrain system is designed around high-precision terrain sources.

The preferred terrain representation is a floating-point heightfield.

Examples:

- OpenEXR (.exr)
- floating-point RAW heightfields
- future floating-point terrain formats

Low-precision formats should be avoided when they introduce visible terrain quantization artifacts.

Terrain source formats should be evaluated based on:

- precision
- import performance
- storage requirements
- compatibility with external authoring tools

The terrain pipeline should remain flexible enough to support future terrain source formats.

The current expected source terrain resolution is either:

- 1 meter per height sample
- 2 meters per height sample

This decision remains open.

Higher source precision is preferred when storage and build time allow it.

---

# External Terrain Pipeline

Terrain authoring occurs outside the runtime.

Preferred workflow:

```text
Gaea
  ↓
High-Precision EXR Heightfield
  ↓
Terrain Masks / Color Maps
  ↓
Runtime Import
  ↓
Chunk Generation
  ↓
Terrain Rendering

The runtime is responsible for:

importing terrain data
generating chunk representations
generating terrain meshes
exposing terrain queries
streaming terrain data
rendering terrain data

The runtime is not responsible for terrain creation.

In-engine terrain authoring tools are out of scope for the initial runtime.

Authoritative Terrain Data

Heightfield data is authoritative world data.

Terrain meshes are derived visual representations.

Terrain meshes are disposable.

All terrain queries should operate on terrain data rather than rendered meshes.

Examples:

terrain height lookup
slope lookup
terrain normal lookup
future pathfinding queries

Heightfield = truth.

Chunk mesh = visualization.

Terrain Mesh Exports

Mesh exports may be useful for:

landmarks
cliffs
caves
special terrain pieces
preview geometry
distant overview geometry

Mesh exports should not be used as the authoritative terrain source.

Authoritative terrain should come from heightfield and terrain metadata.

World Layers
World Data Layer

Persistent geographical data.

Contains:

chunk definitions
coordinates
terrain metadata
mask references
authored placements

Purpose:

Provides foundational world structure.

Terrain Layer

Contains:

terrain height data
terrain chunk data
terrain LOD data
terrain mesh generation

Purpose:

Provides terrain representation and terrain queries.

Doodad Layer

Contains:

authored doodads
procedural doodads
exclusion zones
instance metadata

Purpose:

Provides environmental world objects.

Doodads should initially be treated as environmental content rather than gameplay entities.

Building instances (ADR-079 B2) are authoritative records on `WorldData`, parallel to
units and doodads. Type definitions remain in `BuildingCatalog`; render entities are
derived in the Building Runtime Layer (`src/buildings/`).

Building production (ADR-107 EP2) is authoritative runtime state on `WorldData` via
`BuildingProductionStore`, keyed by `BuildingId`. Buildings own operation progress,
lifecycle, and future input/output consumption. Workers contribute labor through
`TaskType::OperateWorkstation` but do not own production state. Policy intent
(`BuildingOperationPolicy`) is separate from runtime state (`BuildingOperationState`).

Operation definitions (ADR-108 EP3) live in the immutable `OperationCatalog` Bevy resource.
`OperationDefinition` describes what an operation can produce (labor, workers, category,
future input/output seams). `BuildingOperationPolicy.selected_operation` stores only
`OperationDefinitionId`; definitions are resolved at runtime from the catalog and are never
serialized in saves. `BuildingDefinition.supported_operations` declares explicit building
compatibility; `default_operation_id` is optional and never inferred arbitrarily when
multiple operations are supported. Role-tagged building inventories (ADR-109 EP4) are
authored on `BuildingDefinition.inventory_bindings` and instantiated at building create into
`BuildingInventoryBindingStore` on `WorldData`. Each binding maps a stable
`BuildingInventoryBindingId` and `BuildingInventoryRole` to a runtime `InventoryId`.
`OperationDefinition` future I/O references binding IDs, not inventory instances.
`BuildingRecord.inventory_id` remains a compatibility accessor for the explicit default
binding. Generic production execution (ADR-110 EP5) runs when operation progress completes
a cycle: `execute_production_cycle` resolves authored `OperationDefinition` inputs/outputs
through inventory bindings, validates availability and output capacity, then atomically
consumes inputs and produces outputs via inventory runtime APIs. Failures block production
through `OperationalLimitingFactor` (`MissingInput`, `OutputBlocked`, `MissingInventory`,
`InvalidInventoryBinding`) without partial mutations. Mine Iron, Smelt Iron, and Bake Bread
are starter catalog content exercising the same runtime.

Building production extraction (ADR-111 EP6) integrates permanent Terrain Fields into
operational efficiency without changing execution. `OperationDefinition.terrain_requirements`
declares which fields influence a selected operation. `terrain_efficiency_for_operation`
filters the cached `BuildingTerrainAssessment` to those requirements; production steps never
resample terrain. Terrain Fields store environmental potential only — nothing is depleted.
Mine Iron, Mine Stone, and Pump Water produce `iron_ore`, `stone`, and `water` through the
same `execute_production_cycle` path as crafting operations. Poor terrain reduces efficiency
to zero via existing `OperationalLimitingFactor` values; buildings are never destroyed for
invalid placement.

Generic hauling and logistics (ADR-112 EP7) move items between building inventories through
authoritative `HaulingRequest` records on `WorldData`. Buildings declare `logistics_routes` on
`BuildingDefinition`; production assessment and completion hooks generate requests for output
surplus and input deficits. Remote endpoints resolve via `LogisticsEndpointIndex` — workers never
scan all inventories. `InventoryReservationStore` reserves source items and destination capacity;
`pickup_haul_cargo` and `deposit_haul_cargo` move items through worker inventories atomically.
Workers execute `TaskType::Haul` via `assign_hauling_task` and `step_haul_worker_tasks`.

Multi-building production chains (ADR-113 EP8) prove complete vertical slices on the same
generic runtime. `smelt_iron` consumes Iron Ore and produces Iron Bar and Slag; `bake_bread`
consumes Flour and Water and produces Bread. Fuel is deferred (Option A): `fuel_input` bindings
exist as seams but operations do not require fuel items. Production validates inputs using
unreserved quantities from the shared `InventoryReservationStore`; `InputReserved` blocks cycles
when haul reservations hold required items. Logistics routes connect mines, processors, and
`storage_chest` without production searching remote inventories. Input bindings accept deliveries;
output bindings advertise supply for surplus hauling.

Settlement production planning (ADR-114 EP9) decides what buildings should produce without
executing production or moving items. Each settlement owns a `SettlementProductionPlanner` on
`WorldData` with authored `StockGoal` targets. The planner aggregates storage inventory,
propagates demand through a catalog-derived production graph, detects circular recipes, and
updates `BuildingOperationPolicy` (enable, selected operation, priority, repeat). Runtime
diagnostics and derived graphs are not persisted. `step_settlement_production_planners` runs
before worker tasks when operation catalogs are available.

Settlement AI architecture (ADR-115) defines the authoritative long-term structure for all future
Settlement AI (SA) phases that build on EP1–EP9. Authority flows strictly downward — World →
(Faction) → Settlement → Building → Task → Worker — with each layer commanding only the layer
beneath it. Settlements own strategy as a single **weighted-need arbiter**: need pressures are
computed, one generic loop services the most pressing unmet need by applying that need's
**data-defined Response**, and the settlement self-corrects as needs re-weigh. There is no
separate Goals planner and no stack of domain decision engines — player, faction, and any future
top-level AI all write the same **directive** nudges (need weights/targets). Buildings own
operations and report capabilities/status; workers (units holding a task) execute only.
Emergencies and interrupts are priority reweighting, not separate systems. The
`BuildingOperationPolicy` (intent) vs `BuildingOperationState` (truth) boundary, data-first
authority on `WorldData`, event-driven (dirty-flag) reevaluation, and one shared simulation model
for player and AI settlements are enforced invariants.

Settlement runtime (ADR-116 SA1) introduces authoritative **`SettlementState`** on `WorldData`
(`SettlementStateStore`), parallel to treasury identity (`SettlementRecord`). It stores persistent
memory only: kind/archetype, policies, need targets, modifiers, emergency flags, planner lifecycle
scheduling, and extension seams. It does **not** evaluate needs, plan, assign workers, or generate
tasks. Derived analysis (pressures, priorities, response graphs, planner caches, temporary
diagnostics) is never authoritative and must be rebuildable entirely from SettlementState after
load (rebuild principle). Player and AI/wildlife/pack settlements use the identical runtime type;
differences are authored data. Scene format v13 persists SettlementState.

Need evaluation (ADR-117 SA2) computes transient **`NeedSnapshot`** pressures from SettlementState
plus read-only world sensors (inventories, buildings, policies). Authored **`NeedDefinition`**
entries live in `NeedCatalog`; runtime results live in `NeedEvaluationStore` on `WorldData` and are
never persisted. Evaluation is dirty/cadence driven (not every frame), independent per need, and
produces normalized pressure `0..=100` as the only output future systems should consume. It does
not plan, generate tasks, or mutate production/buildings/workers.

Response Engine (ADR-118 SA3) converts pressures into scored **`CandidateResponse`** options via an
authored **`ResponseCatalog`**. Discovery is NeedSnapshot → catalog `supported_need_ids` →
capability/policy validation → score. Results live in transient `ResponseCandidateStore` (never
persisted). Needs never know responses; responses never know workers; the engine never executes —
no tasks, policy writes, construction, or inventory changes. Future selection/planning consumes
candidates only.

Response Arbiter (ADR-119 SA4) evaluates CandidateResponses and produces transient
**`SettlementIntent`** / `SettlementIntentPlan` on `SettlementIntentStore`. It ranks and selects
multiple intents under budgets (pressure, scores, policies, workload, emergencies) without modifying
buildings, tasks, workers, or inventories. Intent is never persisted and rebuilds after load.
Execution remains a later phase.

Building Intent Propagation (ADR-120 SA5) converts SettlementIntent into
**`BuildingOperationPolicy`** changes via capability-based building discovery (`supported_operations`).
It is the first SA phase that influences the world. It never touches `BuildingOperationState`, tasks,
logistics, or construction. EP9 skips SA5-assigned buildings so settlement strategy remains
authoritative for those policies. Workers remain unaware of settlement strategy.

Emergency Pressure & Priority Reweighting (ADR-123 SA8) evaluates authored `EmergencyDefinition`s
into persistent `ActiveEmergencyInstance` records (severity, hysteresis, manual overrides). Emergencies
reweight need pressure (SA2) and response scores/availability (SA3), optionally bump strategic task
priority one tier, and relax SA7 preemption when interruption policy allows. They never command
workers or buildings. Evaluation reports are transient; active instances persist with SettlementState.

Strategic Construction Planning (ADR-124 SA9) converts construction `SettlementIntent` into persistent
**`ConstructionPlan`** records. Authored response→capability→building mappings and capacity-gap checks
decide whether to build; bounded placement search (hard validity vs soft preference) selects sites
inside settlement reach; committing a plan places a `Planned` building as spatial reservation.
`SettlementIntent` stays transient; committed plans survive pressure dips. Player and AI share one
model (approval/autonomy policies). The planner never spawns Complete buildings, assigns workers, or
moves materials — SA6 tasks and existing construct/haul runtimes execute. Planning reports are
transient; plans and reservations persist (scene v14+).

Strategic Task Generation (ADR-121 SA6) converts SettlementIntent into executable **Tasks** in the
shared `TaskStore` via authored `StrategicTaskTemplate` mappings (ResponseId/Type → task kind).
Tasks remain owned by `WorldData`; Settlement AI contributes, merges, and cancels Available strategic
tasks but never assigns workers. Production/haul/crafting/mining tasks stay owned by their runtimes.
**Runtime order places SA9 before SA6** so construct tasks attach to reserved ConstructionPlan sites.
Intent → need pressure → response priority → task priority. Reports are transient; authoritative
tasks (optional `StrategicTaskOrigin`) persist and regenerate safely after load.

Worker Assignment (ADR-122 SA7) treats `TaskStore` (+ open hauling requests) as a **marketplace**.
Idle workers evaluate priority, distance, capability, and reservations, then claim work through
existing reservation APIs. Settlement never selects workers. Higher-priority work may preempt with
hysteresis; interrupted tasks return to Available. PlayerAssigned work is immune to autonomous
preemption. Assignment reports are transient; authoritative assignments/reservations persist.

Planning Scheduler (ADR-125 SA10) determines **when** settlement planner stages run (event-driven dirty
flags, fallback cadence, budgets, stagger, priority). Stages keep their own evaluation logic; the
scheduler never plans. Invalidation is strictly downstream — later stages must not dirty earlier ones;
Worker Assignment never invalidates planning. Implementation consolidates per-stage poll loops; it
does not change Need/Response/Intent/Construction decisions.

**Authoritative SA runtime pipeline:** SA8 → SA2 → SA3 → SA4 → SA5 → (EP9) → SA9 → SA6 → SA7.

Occupancy Layer

Contains:

buildings
walls
blockers
dynamic obstacles

Purpose:

Supports future pathfinding and gameplay.

Occupancy data should remain separate from terrain data.

Terrain is mostly static.

Occupancy is expected to change.

Rendering Layer

Contains:

terrain rendering
doodad rendering
far-world rendering
visibility systems

Purpose:

Visual representation only.

Rendering owns no authoritative game state.

Gameplay Layer

Contains:

interactions
combat
local simulation
player-facing systems

Purpose:

Provides detailed local behavior.

Gameplay systems should consume queries rather than raw world structures whenever possible.

Future Simulation Layer

Not part of the immediate implementation.

Architecture must remain compatible with it.

Contains:

settlements
units
caravans
routes
schedules
resource production
reputation systems
faction systems
population systems
assignment systems

Purpose:

Allows the world to remain active independent of player location.

Settlements

Settlements are future simulation objects. Automation philosophy (professions, task
generation, building production requests) is defined in
[DESIGN.md](DESIGN.md#settlement-automation) and [ADR-072](ADRs/ADR-072-settlement-automation-and-production.md).

Examples:

towns
villages
camps
mines
farms
player bases

Settlements are not terrain.

Settlements are not doodads.

Settlements are not chunks.

Settlements may own or reference:

population
buildings
storage
role assignments
needs
faction ownership
event history

Settlements should be treated as first-class world entities.

Resources

Architecture must support future resource systems.

Examples:

trees
ore
crops
wildlife

Resources should be represented separately from their visual appearance.

Example:

Resource Node:

type
state
metadata

Visual Representation:

mesh
material
LOD

This allows harvesting, depletion, and regrowth without redesigning the rendering system.

Persistence Rule

Procedural generation creates the initial world state.

After world creation, gameplay changes are represented as overrides.

Examples:

harvested trees
constructed buildings
destroyed buildings
dead units
settlement ownership changes
reputation changes

Procedural generation should never overwrite persistent world state.

The generated world is the baseline.

The simulation state is the truth.

Pathfinding Considerations

Pathfinding is not part of the immediate goal.

However:

Future pathfinding must not depend on rendered terrain.

Future pathfinding should primarily consume:

occupancy data
terrain queries
navigation metadata
route graphs

Long-distance movement should eventually support graph-based travel.

Examples:

roads
trade routes
caravan routes

Local movement may use detailed pathfinding later.

Reputation Considerations

Not part of the immediate goal.

Architecture should remain compatible with:

personal relationships
faction reputation
regional reputation
species reputation
event-based consequences
delayed reporting
local knowledge

Future reputation systems should support evaluating relationships between units without requiring every possible relationship pair to be eagerly stored.

Personal relationship history should be stored sparsely when events justify it.

Consequences should not assume instantaneous global knowledge.

Building Considerations

Not part of the immediate goal.

Architecture should support:

player-built structures
settlement expansion
occupancy modification
future pathfinding updates

Buildings should primarily affect occupancy rather than terrain.

Multiplayer Considerations

Not part of the immediate goal.

Architecture should remain compatible with future multiplayer.

Requirements:

deterministic procedural generation
serializable simulation state
serializable world state
rendering independence

Networking should be able to replicate world state without redesigning core systems.

Avoid Premature Renderer Complexity

Custom shaders, GPU-driven systems, custom render pipelines, and renderer-specific optimizations should not be introduced unless profiling demonstrates a need.

The initial architecture should remain compatible with future renderer upgrades without requiring changes to world data structures.

World architecture takes priority over renderer complexity.

Explicit Non-Goals

The following are intentionally out of scope for the initial runtime:

in-engine terrain authoring tools
in-engine terrain sculpting tools
in-engine texture painting tools
road editors
river editors
full world editor
advanced AI
economy simulation
faction simulation
settlement simulation
multiplayer implementation
navmesh generation
large-scale combat systems
custom GPU rendering pipelines

**Note:** Unit combat **foundations** (orders, range, strikes, projectiles, basic AI) are
implemented per ADRs 054–062. [ADR-069](ADRs/ADR-069-combat-design-philosophy.md) documents
the target Warcraft III-style experience; downed state, stagger, facing, and attribute
scaling remain future work. Settlement, economy, and full creature AI remain deferred per
ADR-072 and ADR-071.

These may be added later.

The architecture should permit them.

The initial implementation should not attempt to solve them.

## Items and Inventory Authority (I1–I8)

Physical items, grid inventories, corpses, world piles, building containers, and settlement
treasury balances are authoritative on **`WorldData`** (ADRs 087–094). ECS entities and UI state
are never inventory truth.

- **Physical gold** is a normal item stack everywhere except treasury records (abstract balance only).
- **Derived caches** (`cell_owner`, `total_mass_grams`, spatial indexes) rebuild after load.
- **`validate_world_inventory_state`** is the single validation entry point for dev and scene apply.
- **Scene v7** persists full inventory world state for dev snapshots; see ADR-094.

Final audit: [docs/items-inventory-final-audit.md](docs/items-inventory-final-audit.md).

Development Rule

When making design decisions:

Prefer generic world concepts over game-specific implementations.

Build systems that solve categories of problems.

Avoid implementing features solely to satisfy a single future mechanic.

If a future mechanic can be supported through existing abstractions, prefer the abstraction.

The architecture should evolve toward a simulation-driven world rather than a collection of isolated gameplay systems.