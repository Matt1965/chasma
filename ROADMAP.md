# ROADMAP.md

# Purpose

This document defines implementation order.

It does not define final project scope.

The final project vision is described in ARCHITECTURE.md.

Future systems may appear in ARCHITECTURE.md long before they appear in this roadmap.

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

Potential future systems:

- population
- needs
- assignments
- production
- storage
- ownership

---

## Resources

Potential future systems:

- harvesting
- depletion
- regrowth
- extraction

---

## Units

Potential future systems:

- movement
- equipment
- injuries
- progression

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

Potential future systems:

- local combat
- squad combat
- large encounters

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