# Building Navigation & Universal Movement Contract

**Status:** Normative architecture contract (documentation).  
**Period:** August 2026 — authority consolidation + simplified universal model (IN-11gB).  
**Audience:** Contributors, AI agents, future navigation implementation slices.

This document defines **required architecture** for unit movement and pathfinding, with building navigation blueprints as one blocker source among several. It also defines debugging and escalation methodology.

**Important:** Statements here describe **required architecture**. They are not claims that the current codebase fully conforms. Where implementation may diverge, verify on the live path before changing behavior.

**Related:**

- [navigation-blueprint.md](navigation-blueprint.md) — schema, editor, persistence mechanics
- [occupancy-authoring.md](occupancy-authoring.md) — footprint catalog (placement/diagnostics; not movement authority)
- [ADR-080](../ADRs/ADR-080-generalized-occupancy-and-baked-footprints.md) — occupancy foundation (partially superseded for building **unit movement**)
- [ADR-083](../ADRs/ADR-083-navigable-spaces-portals-and-interior-visibility.md) — spaces, portals, cross-space routing
- [AGENTS.md](../AGENTS.md) — concise invariants and escalation rules

---

## Core design philosophy

Navigation should have **one** underlying movement-legality system.

The Navigation Editor is **not** a separate navigation system. It is an **authoring tool** for geometry and metadata consumed by the same universal runtime navigation used everywhere else.

```
Navigation Editor
    ↓
Navigation Blueprint (persisted)
    ↓
Runtime navigation geometry + connection metadata
    ↓
Universal movement legality query
    ↓
Planner · Simplifier · Movement executor · Blocked Area overlay
```

Planner, simplifier, and movement executor must **not** invent separate definitions of legal movement.

---

## Target conceptual model

```
WORLD BLOCKER SOURCES
    |
    +-- Terrain
    +-- Doodads
    +-- Units
    +-- Explicit world obstacles
    +-- Building Navigation Blueprints
              |
              +-- Region polygons (literal boundaries)
              +-- Boundary blockers (closed edges)
              +-- Entrance openings (intentional gaps)
              +-- Connection metadata (derived portals)
    |
    v
UNIVERSAL MOVEMENT LEGALITY  ("Can agent move from A to B?")
    |
    +-- Planner (find legal route)
    +-- Simplifier (shorten without illegalizing)
    +-- Movement executor (enforce legality)
    +-- Blocked Area visualization (truthful diagnostic)

Portal     = derived connection metadata through an authored opening
SpaceId    = semantic region/space identity (membership, routing, presentation)
Entrance   = intentional boundary opening + authoring metadata
```

---

## Authoritative design vs implementation status

| Kind | Meaning |
|------|---------|
| **Required architecture** | What Chasma is intended to do. Future work should converge here. |
| **Current implementation** | What has been verified in code or manual testing. May lag or diverge. |

Use phrasing such as:

- “Required architecture: …”
- “Current implementation may not yet fully conform; verify before modifying.”

Do not convert known bugs into design requirements. Do not write “Chasma does X” without verification.

---

## Universal movement legality

At the lowest conceptual level, runtime navigation answers one question:

**Can this agent move from A to B?**

The exact mechanical implementation is **not** prescribed here. It may use segment intersection, swept-circle/capsule tests, spatial indexes, navigation grids, polygon geometry, cached masks, or other appropriate structures.

**Required architecture:** All consumers use the **same** authoritative movement-legality semantics:

- A* / route planning
- Direct-path and line-of-sight tests
- Path simplification
- Movement execution and arrival validation
- Portal/entrance approach
- Blocked Area debug visualization

No consumer may independently reinterpret blockers.

### Pathfinding consumes blockers; it does not define them

| Component | Job |
|-----------|-----|
| **Planner** | Find a route through movement that is **legal** |
| **Simplifier** | Reduce the route **without making it illegal** |
| **Movement executor** | Execute the legal route and **prevent invalid movement** |

This eliminates scenario-specific rules such as: cardinal-only boundary exceptions, diagonal-only boundary checks, special simplifier LOS legality, or different executor boundary semantics.

### Avoid scenario-specific navigation architecture

Core runtime architecture should use reusable primitives:

- Segment legality · Blocker query · Region containment · Boundary crossing · Opening · Connection · Agent clearance

Do not accumulate special cases for particular huts, corners, entrances, footprints, or concave shapes. Those should resolve through general geometry.

Tests may use descriptive scenario names; production architecture should not be named around incidents.

---

## Sources of navigation blocking

The universal system composes **independent blocker domains**:

| Source | Role |
|--------|------|
| **Terrain** | Slope, grounding, availability |
| **Doodads** | Independent obstacle authority |
| **Units** | Independent collision authority |
| **Explicit world obstacles** | Authored/static blockers outside buildings |
| **Building Navigation Blueprints** | Building-specific geometry and connections |

Doodad or unit collision is **not** a fallback when a building lacks a blueprint.

---

## 1. Building navigation blueprint authority

### Hard invariant (unit movement and pathfinding)

For **runtime unit movement and pathfinding**, each building has **exactly one** building-specific navigation authority.

### Navigation blueprint present

**Required architecture:** If a valid authored or generated navigation blueprint exists for a building:

- The blueprint is that building’s **sole** navigation authority.
- **Region geometry** owns navigable building space.
- **Region boundaries** own building navigation barriers.
- **Entrance openings** own legal boundary crossings.
- **Connections** own legal region↔region and floor↔floor crossings.
- **Explicit interior obstacles** (when intentionally authored) may add additional blocking.

The following must **not** independently block movement for that building:

- Render mesh · Visual geometry · Physics/render collider
- Legacy analytic building footprint · Legacy baked footprint
- Old building occupancy representation
- Rectangle/circle footprint used before blueprint navigation

Those may exist for **non-movement** purposes: placement, authoring, selection, diagnostics, catalog sizing. They are **not** fallback runtime movement authorities.

### No navigation blueprint

**Required architecture:** If a building has **no** valid navigation blueprint:

- The building is a **navigation ghost**.
- It contributes **no** building-specific movement blockers.
- No runtime fallback to analytic footprint, baked footprint, render mesh, collider, doodad collision, or legacy occupancy.

If a building should block movement, it must receive a navigation blueprint (authored or generated).

### Generated navigation blueprint

A valid generated/default blueprint counts **exactly** the same as an authored blueprint once active.

| State | Movement authority |
|-------|-------------------|
| Authored blueprint active | Blueprint |
| Generated/default blueprint active | Blueprint |
| No blueprint | Ghost (no building movement blocking) |

There is no fourth “legacy movement footprint” fallback.

**Authored vs generated at runtime:** Once resolved, runtime navigation does not care whether the blueprint was manual, automatic, variant, or another legitimate source. Source may remain metadata; it is not a different passability implementation.

### Buildings without interiors

Simple building-like objects (smelters, chests, machines, small structures) may receive a **generated** navigation blueprint: typically a closed polygonal boundary with no usable entrance. They behave as solid obstacles without manual authoring.

No legacy runtime footprint fallback is required.

### Generation purpose

Navigation generation has **two** primary purposes:

1. **Automatic coverage** — Avoid manual authoring for every building-like object that should block movement.
2. **Authoring baseline** — For buildings that will be manually edited, generation provides a starting polygon so authors do not begin from an empty editor.

Typical workflow: building asset → generate baseline polygon → Navigation Editor → adjust vertices/regions/entrances → save.

Generated navigation is an authoring convenience and default coverage mechanism. It is **not** a separate runtime architecture.

**Implementation note (August 2026):** Code may still use `BuildingNavigationMovementAuthority::LegacyFootprint` when runtime navigation is absent. That path **does not** conform to the ghost contract. Treat ghost behavior as normative; align in a dedicated slice — not by stacking exemptions.

---

## 2. Doodads and units are separate

This building rule does **not** remove other navigation blockers.

Doodads, units, explicit obstacles, reservations, terrain slope/unavailability, and other systems retain their own authorities.

**Required architecture:** Doodad/unit collision is **not** the fallback navigation system for buildings.

Do not describe doodad occupancy as “what buildings use when they lack a blueprint.”

---

## 3. Render geometry is never navigation authority

**Required architecture:** The visible building mesh answers **what the building looks like**. It does **not** answer **where a unit can move**.

Consequences:

- A deliberately oversized navigation region may include the visible hut. In that debugging configuration, a unit must be able to walk through the visible hut because the blueprint says that space is navigable. This is **expected**.
- The visual building must not secretly block the path.
- A visible wall with no corresponding navigation boundary is **not** a movement wall.
- A navigation boundary with no visible wall **still** blocks movement.

Use this separation during debugging.

---

## 4. Navigation region contract

A **navigation region** is primarily a polygonal authored shape.

**Required architecture:** Each region provides:

1. A **polygon boundary** (literal geometry).
2. A **semantic navigable space** inside that boundary.
3. Optional **intentional openings/connections** in its boundary.

The polygon is literal:

- Every vertex matters · Every edge matters · Concavity matters · Closing edge matters

Do **not** substitute rectangle, AABB, convex hull, or bounding circle for movement decisions unless used strictly as an acceleration prefilter that does not alter final legality.

### Region edges are blockers

By default, a region boundary is **closed**. Movement cannot cross that boundary except through an intentional opening.

Inside-region and outside-region movement may occur normally; crossing the boundary requires an opening.

The visible building mesh has **no** influence. A navigation region intentionally larger than the visible building means a unit may walk through the visible mesh. That is correct.

Region boundaries are navigation barriers. Agent clearance must be respected inside regions.

**Diagnostic probe:** Oversized concave temporary polygons are valid tools to detect hidden footprint or exemption authority.

---

## 5. Entrance contract

### Conceptual model

An **Entrance** is fundamentally an **intentional opening in a region boundary**, plus useful metadata.

Movement may cross through the opening. Movement may **not** cross the rest of the edge.

The Entrance object exists because an opening carries semantic information:

- Which region boundary owns the opening
- Opening width · Threshold position · Direction/orientation
- Connected region/space · Optional door association · Interior/exterior semantics

An entrance is **not**:

- A magic passable circle
- A broad footprint exemption
- A region where normal blockers temporarily stop applying

The underlying blocker geometry should simply contain an opening.

### Threshold, landing, staging

| Element | Role |
|---------|------|
| **Threshold** | Anchored to the **owning region boundary edge** — the actual crossing. Must not float off the edge. |
| **Interior landing** | Inside the target region; offset inward; must provide agent clearance. |
| **Exterior staging** | Outside the boundary for surface approach; derived from entrance geometry. |

Surface↔interior and interior↔surface use the same authored relationship in reverse when bidirectional.

### Continuous entrance crossing

For a normal same-height building entrance, movement remains **continuous**:

outside → approach opening → cross boundary opening → inside (and reverse on exit)

There should not be an artificial teleport solely because an Entrance is represented as a Portal. Staging/threshold/landing points may exist internally as **derived guidance** for route planning; they do not replace the actual boundary opening.

Vertical transitions (stairs, ramps, elevators, different floors) may need additional mechanics but should use the same connection concept.

### Editor glyph (preserved)

- Long bar parallel to the owning region edge.
- Short end bars perpendicular to the edge.
- Visually resembles a capital **I** aligned along the boundary.

**Implementation note:** Runtime may use disc triggers for traversal detection. Required architecture treats the **edge opening** as authoritative. Portal discs are derived diagnostics, not substitute “holes in the whole wall” or passability exemptions.

---

## 6. Portal contract

A **Portal** is **derived connection metadata** created from an authored Entrance or another explicit transition.

It answers: **What navigable spaces are connected through this opening?**

Example:

```
Surface  ↔  Entrance opening  ↔  Interior Region A
```

A Portal may supply metadata for:

- Cross-region route planning · Surface/interior membership changes
- Door state · Roof/interior visibility · Multi-floor routing · AI routing

A Portal is **not** the physical movement blocker. It should **not** make nearby terrain or footprint cells magically passable. Physical legality comes from navigation geometry; Portal semantics describe the connection.

**Supersedes (partial):** Historical ADR and code patterns treating portal-radius discs as independent surface passability authority are **not** normative under this model. See [ADR-083](../ADRs/ADR-083-navigable-spaces-portals-and-interior-visibility.md) — connection graph remains valid; passability exemption via portal circles is superseded for movement legality.

---

## 7. Region membership and SpaceId

**Region membership** is semantic state:

- Is the unit on Surface? Inside Interior Region A?
- Which floor owns navigation height?
- Should this building roof become transparent?
- Which connections are available next?

Membership should **not** create a separate collision universe. The universal movement-legality system remains authoritative.

Crossing an intentional region connection updates semantic membership. For a normal entrance: Surface → cross opening → Interior Region (and reverse on exit).

### SpaceId

Keep `SpaceId` when useful. Its conceptual purpose:

**Which navigable space does this position, waypoint, or unit belong to?**

It may support region membership, floor height, connection-graph routing, interior visibility, and waypoint semantics.

Planner, simplifier, and executor must **not** use contradictory movement rules because of `SpaceId`. They must agree on which space is evaluated and use the same legality contract within each space.

See [ADR-083](../ADRs/ADR-083-navigable-spaces-portals-and-interior-visibility.md) for the established space registry and portal graph model.

---

## 8. Persistence / runtime contract

**Authoring/persistence** and **runtime activation** are distinct layers.

A persisted blueprint visible in the Navigation Editor does **not** prove runtime navigation is active.

**Required architecture:**

- Cold load of a persisted blueprint must produce **equivalent runtime topology** to an unchanged Save/Apply refresh.
- Save/Apply must not be a magic button required to make navigation work.
- Runtime state is derived from persisted authoritative data.
- Derived activation state must be rebuilt when loading.
- Stale runtime activation flags must not permanently suppress rehydration.

### Invariant

For the same building and unchanged blueprint data:

**Cold Load Runtime Topology == No-Op Save/Apply Runtime Topology**

Topology equivalence includes: regions, spaces, entrances/portals, connections, geometry, enabled states derived from identical data. Runtime numeric ids need not match if allocation is non-deterministic.

**Diagnostic:** If a no-op Save/Apply changes behavior, suspect persistence vs runtime rehydration divergence.

---

## 9. Navigation Editor responsibility

The Navigation Editor is conceptually simple. It **edits**:

- Region geometry · Vertices · Entrances/openings · Connections · Floors (where applicable) · Relevant navigation metadata

It **saves** Navigation Blueprints.

It does **not**:

- Define another pathfinding system
- Directly manage runtime footprint exemptions
- Expose internal planner implementation details to authors

The Navigation Editor authors geometry; the universal runtime system consumes it.

### Preserve current editor presentation

The current Navigation Editor visual experience is **desirable** and should **not** be redesigned as part of runtime simplification work.

Preserve conceptually:

- Current region, vertex, and entrance visualization
- Current editor layout · Transparency slider · Save behavior
- Existing variant-save functionality (not redesigned due to lower testing priority)

The problem being simplified is **runtime navigation architecture**, not authoring presentation.

---

## 10. Editor visuals vs debug overlays

Clearly distinguish two layers:

| Layer | Purpose |
|-------|---------|
| **Navigation Editor visuals** | Edit vertices, edges, entrances, regions — part of the editor as currently designed |
| **Blocked Area** | Inspect **actual runtime movement authority** |

Do **not** merge these. An authored edge may exist correctly while runtime activation is broken. That distinction is diagnostically useful.

### Blocked Area (primary current diagnostic)

For the current “get navigation working correctly” phase, **one** general navigation overlay is required: **Blocked Area**.

**Purpose:** Show where the authoritative runtime movement system currently allows and rejects movement.

Must **not** require: selected unit · moving unit · active path · special movement command.

Must work while editing Navigation Blueprints.

Exact visualization is implementation judgment (polygon shading, blocker lines + clearance, sampled grid/mask, etc.). **Critical requirement: truthfulness** — do not show one approximation while movement uses another authority.

### Runtime Entrances (not core authoring)

**Runtime Entrances** is **not** required as a normal initial authoring overlay. The editor already visualizes authored entrances.

Runtime entrance/portal diagnostics may remain as **advanced forensic** tools. Normal authoring workflow should primarily need: Navigation Editor geometry + Blocked Area.

### Selected Unit Path (not core editor tooling)

**Selected Unit Path** is **not** part of the core Navigation Editor workflow. It should not be presented as a normal editor overlay/control.

Underlying diagnostic code may remain for advanced debugging. Future work may move it to advanced dev diagnostics, hide it, or remove it if unused.

### Authored Blueprint overlay

**Answers:** “What did I author?”

Shows region boundaries, vertices, entrances, relevant authored topology. Persistent toggle; truthful about working copy vs persisted data.

---

## 11. Manual game behavior is acceptance authority

Automated tests are required. When a test passes but the real game visibly fails the same behavior, **the real-game observation wins**.

The correct response is **not**:

- Another speculative patch
- Assuming the manual test is wrong
- Increasing test count without checking the real path

The correct response is: determine why the test is not exercising the same runtime path.

A helper test such as `query_passability_in_space returns Passable` does **not** prove the player-commanded unit uses that function.

End-to-end regressions should exercise, where relevant:

```
player command → goal resolution → space resolution → planner
→ path simplification → portal handling → movement execution → arrival
```

---

## 12. Debugging escalation rule

Repository-level working rule for navigation defects.

### Level 1 — First occurrence / clear local bug

A narrow quick fix is acceptable when:

- Cause is directly proven
- Ownership is unambiguous
- Scope is local
- Manual verification is straightforward

### Level 2 — First attempt failed

If an issue remains after **one** implementation attempt: **STOP** normal patching.

Before another behavior change:

1. Reproduce the actual live-game failure
2. Trace the real runtime call chain
3. Identify the exact first incorrect decision/rejection
4. Determine which authority supplied it
5. Compare intended vs actual ownership

Do not issue a larger speculative implementation prompt.

### Level 3 — Regression / “we already fixed this”

**Mandatory forensic phase.** No behavior edits until the cause is proven.

Investigation may include: runtime `SpaceId`, building navigation authority, resolved blueprint authority, runtime region/portal counts, actual passability function, blocker source, raw vs simplified path, movement executor, persisted vs runtime topology, cold load vs Save/Apply state.

### Level 4 — Tests disagree with real game

Mandatory vertical-path investigation. Find where the test diverges from player-controlled behavior. Do not accept the green test as proof.

---

## 13. “Already attempted” rule

When a user reports a problem persists after a prior fix attempt:

Do **not** immediately generate another implementation plan.

First:

1. Review what the prior fix claimed to change
2. Identify which claim real behavior disproves
3. Trace whether the changed code runs in the live scenario
4. Determine whether the prior fix targeted wrong layer, dead code, synthetic-only path, wrong authority, or correct layer but incomplete call chain
5. Only then implement

“Already attempted” means **increase rigor**, not increase prompt size.

---

## 14. Design / implementation phasing

Preferred workflow for complex navigation bugs:

| Phase | Activity |
|-------|----------|
| **A. Observation** | Capture actual behavior |
| **B. Discriminating test** | Smallest experiment that distinguishes causes (oversize region, concave polygon, no-op Save/Apply, authored vs runtime portal counts) |
| **C. Read-only forensic review** | No behavior edits; exact live authority, first divergence, call chain |
| **D. Review findings** | Confirm finding explains observation before coding |
| **E. Narrow implementation** | Fix only the proven defect |
| **F. Automated regression** | Low-level + vertical-path coverage where relevant |
| **G. Manual acceptance** | Verify original reproduction in running game |

Do not automatically proceed from forensic result to implementation without confirmation.

---

## 15. Prompt-scoping guidance

Avoid implementation slices that simultaneously redesign runtime activation, persistence, editor UI, overlays, pathfinding, passability, portals, and save system unless architecture genuinely requires them all.

Prefer: diagnosis slice → one ownership correction → manual acceptance → next defect.

When multiple systems appear broken, identify the **first failure** in the chain before fixing downstream behavior.

Large prompts are not inherently better.

---

## 16. Ownership-first principle

For every navigation datum or decision, ask: **who owns this?**

| Datum / decision | Owner |
|------------------|-------|
| Region polygon | Navigation blueprint |
| Entrance threshold | Navigation blueprint |
| Runtime `SpaceId` | Derived runtime navigation |
| Runtime `PortalId` | Derived runtime navigation |
| Visual mesh | Presentation |
| Terrain elevation | Surface world data |
| Building-local interior elevation | Navigation / building transform |
| Doodad collision | Doodad obstacle authority |
| Unit collision radius | Unit / navigation agent definition |
| Building footprint (catalog) | Placement, sizing validation, diagnostics — **not** movement when blueprint active; **not** movement at all when no blueprint (ghost) |

Avoid two systems independently owning the same movement-blocking decision. Do not solve ownership conflicts by stacking exemptions.

---

## 17. Known lessons (debugging examples)

Concrete lessons from recent development — **diagnostic patterns**, not permanent bug descriptions:

- Saved Navigation Editor overlay can show correct persisted geometry while runtime navigation is inactive.
- Synthetic test fixtures can supply dependencies the real asset lacks.
- Portal/unit can have correct authoritative coordinates but incorrect render coordinates.
- Large/concave temporary polygons are useful diagnostic probes.
- No-op Save/Apply changing behavior indicates runtime rehydration/state divergence.
- Visual building blocking despite no nearby navigation edge proves another authority is still active.
- Endpoint-only movement validation can miss a boundary crossing.
- Passing helper-level tests does not prove player-command behavior.

---

## 18. Current implementation status (August 2026)

Recent work has focused on:

- Blueprint-driven building navigation
- Multi-region interiors
- Portal/entrance traversal
- Navigation Editor authoring
- Persistence/runtime parity (cold load vs Save/Apply)
- Removing contradictory building blocking authorities

**Verified in automated tests (partial list):** blueprint buildings skipping legacy footprint in static occupancy queries; legacy buildings still blocking via footprint when no runtime navigation; interior segment/boundary tests; vertical player-command path for concave regions and interior footprint crossing; real-hut activation/reconcile tests.

**Not verified / may remain open:** Full manual acceptance on irregular Survival Hut polygon; all entrance traversal angles; complete ghost behavior for no-blueprint buildings; full overlay fidelity for Blocked Area under all authority modes.

**Known implementation gaps (may not conform to this contract):**

- `LegacyFootprint` authority when runtime navigation is absent (contradicts ghost rule)
- Planner/simplifier/executor may use different segment legality (e.g. cardinal A* steps, simplifier LOS without boundary checks)
- Portal-radius surface passability exemptions may stack as second authority
- Blocked Area overlay may not yet fully reflect universal legality

**Normative statement:** Architectural contracts in this document are **normative** even where implementation has not been verified to conform.

Do not claim unresolved entrance/pathing behavior is fixed without live-game confirmation.

---

## 19. Documentation conflicts and supersession

Historical docs and ADRs may describe overlapping or superseded concepts. **Do not rewrite historical ADR bodies** as though they never existed. Add supersession notes where this contract supersedes prior movement semantics.

### Conflicts identified (August 2026)

| Topic | Historical / code implication | Normative model (this document) |
|-------|------------------------------|----------------------------------|
| **Portal circles** | Portal-radius discs exempt surface passability | Portal = connection metadata; physical legality from geometry with opening |
| **Legacy footprint fallback** | Footprint blocks when no runtime blueprint | No blueprint → ghost; generated blueprint for objects that should block |
| **Separate Surface/Interior universes** | Different passability paths per space | One universal legality contract; `SpaceId` is semantic membership |
| **Navigation Editor owns runtime** | Editor overlays imply runtime authority | Editor authors blueprint; runtime derives geometry independently |
| **Planner-specific blockers** | A*, simplifier, executor use different rules | All consume same movement-legality query |

### Cross-reference updates

- [navigation-blueprint.md](navigation-blueprint.md) — universal legality and portal supersession notes
- [ADR-080](../ADRs/ADR-080-generalized-occupancy-and-baked-footprints.md) — partial supersession for building **unit movement**
- [ADR-083](../ADRs/ADR-083-navigable-spaces-portals-and-interior-visibility.md) — connection graph retained; portal passability exemption superseded
- [occupancy-authoring.md](occupancy-authoring.md) — footprint vs movement scope
- [dev-mode.md](dev-mode.md) — Blocked Area as primary navigation diagnostic

Historical ADR text is preserved; supersession notes clarify movement authority only.
