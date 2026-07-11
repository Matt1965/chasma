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
| Fixed simulation tick orchestrator | ADR-065 in place |
| Dev Mode (catalog, spawn, scenes, inspector) | Dev-gated; data-driven from Excel |
| Environment (time-of-day, water, lighting) | Dev/runtime presentation; singleton-safe |

## Explicitly deferred

- Animation / locomotion blending
- Economy, buildings, harvesting loops (see [ADR-072](ADRs/ADR-072-settlement-automation-and-production.md), DESIGN.md)
- Full creature AI template stack ([ADR-071](ADRs/ADR-071-creature-ai-architecture.md); ADR-062 scan AI only)
- Use-based skills and attribute-driven combat ([ADR-070](ADRs/ADR-070-progression-and-attributes.md))
- Grid inventory and equipment ([ADR-073](ADRs/ADR-073-inventory-and-equipment.md))
- Downed state, stagger, facing, weapon hit volumes ([ADR-069](ADRs/ADR-069-combat-design-philosophy.md))
- Full pathfinding optimizations (pooling, hierarchical)
- Multiplayer replication
- Production Excel pipeline outside `feature = "dev"`
- Combat polish (death animations, VFX, advanced AI)

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

**Deferred:**

- use-based skills (no global runtime level; workbook `Level` is authoring metadata)
- attributes driving combat formulas (STR/DEX/CON/PER/AGI/CHR/INT — imported, not simulated)
- injuries and downed state
- grid inventory + equipment slots
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