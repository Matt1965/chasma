# ADR-133: Settlement Identity, Membership, Anchor & Boundary

## Status

Accepted (design direction — implementation pending)

## Context

ADR-116 through ADR-125 implemented the SA1–SA9 pipeline: persistent state, need evaluation,
response scoring, arbitration, policy propagation, strategic tasks, worker assignment, emergencies,
and construction planning. The pipeline runs, but a 2026-08-28 audit found it cannot produce
observable behavior, and part of the cause is that **settlement identity itself is under-specified**.

Three concrete defects:

- `SettlementRecord.anchor_building_id: BuildingId` — a settlement's identity is parasitic on a
  building. A settlement has no independent place in the world.
- `UnitRecord` has no settlement reference at all. A settlement therefore has no member population,
  so no need can be scaled to "how many mouths does this settlement have".
- Building membership is *derived* by `reconcile_settlement_building_membership`, which links
  buildings that share a settlement's affiliation. With one settlement this is invisible; with two
  settlements of the same affiliation it assigns buildings to whichever settlement is found first.

ADR-093 gave settlements a treasury anchored to a building, which was adequate for gold but not for
identity. ADR-115 §20 describes membership as "affiliation-scoped building membership", which this
ADR supersedes.

This ADR defines what a settlement *is* as a world object, who belongs to it, and how one is created.
It does not change the decision pipeline.

## Decision

### A settlement is a placed world object with its own identity

A settlement is anchored by a dedicated **`SettlementAnchor`** world object. It is **not** a
`BuildingRecord`, and **not** a `BuildingDefinition` marked special.

Rationale: `BuildingRecord` carries construction lifecycle, vitals and ruins (ADR-082), occupancy
footprint (ADR-080), operation policy and state (ADR-107), interiors and doors (ADR-084), inventory
bindings (ADR-109), and navigation blueprint authority (`docs/building-navigation-authority.md`). A
settlement's origin point needs none of these. Routing it through `BuildingRecord` would force every
existing building consumer to special-case one record that is not really a building — the
"stacking exemptions" failure the project rules forbid.

Because the anchor is not a building, it cannot be constructed, damaged, ruined, or operated.
Settlement removal is out of scope for this milestone.

### Explicit center and mutable radius

`SettlementRecord` carries the settlement's world **center** and a **mutable**
`boundary_radius_meters`.

The settlement kind (or anchor definition) supplies only the *initial* radius **at creation time**;
it is never read again as authority. Radius is runtime settlement data precisely so it can later
change, grow, or be modified by progression and content without a schema change. Growth mechanics
themselves are out of scope; a fixed initial radius is correct for the milestone.

### Membership is explicit; derived rosters are caches only

`UnitRecord.settlement_id: Option<SettlementId>` and `BuildingRecord.settlement_id:
Option<SettlementId>` are the **sole authority** for membership.

`SettlementStore` rosters and lookup indices are **caches**. They are rebuilt from the authoritative
record fields, are never authoritative, and may be discarded and regenerated at any time — the
rebuild principle from ADR-116.

The following are **not** membership authority, and must never be used to infer it:

- faction
- `Affiliation` (ADR-051)
- `TeamId`
- ownership
- proximity to the anchor
- containment within the boundary

This supersedes ADR-115 §20 and retires `reconcile_settlement_building_membership`.

### Boundary seeds default membership at placement; it does not maintain it

The boundary defines a spatial area. It does **not** define membership.

- **Buildings:** boundary containment is evaluated **once, at creation/placement**, to seed the
  default `settlement_id`. A building placed outside every settlement receives `None`.
- **Buildings do not change membership afterwards.** Moving a building, or a boundary changing around
  it, does not reassign it.
- **Units:** proximity never assigns membership. Existing units are never auto-enrolled. Membership
  requires explicit assignment.

Continuously re-deriving membership from position would make membership a function of location —
which contradicts the explicit-authority rule above and would let strategic ownership silently flip
when a boundary grows or an object moves.

### Settlements may not overlap

Placement of a new settlement anchor is **rejected** when it would overlap an existing settlement.

The exclusion distance is **derived from both settlements' spatial data**, not from a constant tuned
to today's radius:

```text
reject when  distance(new.center, existing.center) < new.radius + existing.radius + margin
```

This stays correct when radii differ and when radius later changes or grows.

Non-overlap is not merely tidiness — it is what makes boundary-seeded default membership
unambiguous. If boundaries could overlap, a building placed in the intersection would have no
defensible default settlement, and the seeding rule above would become arbitrary.

Overlap validation is required even though the milestone scenario uses a single settlement, because
it is a known placement invariant rather than an open design question.

### One creation function; every surface is a caller

A single world-layer `create_settlement(...)` is the only way a settlement comes into existence. It:

1. validates placement against the non-overlap invariant and returns a typed rejection on failure
2. creates the `SettlementAnchor`
3. creates the `SettlementRecord` (center, initial radius, anchor reference)
4. ensures `SettlementState` (ADR-116)
5. ensures the treasury (ADR-093)
6. ensures the EP9 planner entry (ADR-114)

Dev placement and future player Build Mode placement are both **callers** of this function. There is
no dev-only settlement-creation architecture.

Player Build Mode placement is **staged until after the Settlement AI milestone**. The shared
creation function, validation, and anchor type must exist now so that staging adds only placement
intent, ghost validation, and UI — not a second creation path.

### Dev authoring must be able to construct the complete milestone scenario

Because player placement is staged, Dev mode must be sufficient to build **and save** the whole
scenario. Required practical flow:

```text
place SettlementAnchor
  → SettlementRecord + SettlementState created
  → place buildings inside the boundary
  → building receives explicit settlement_id (seeded at placement)
  → place / select units
  → assign explicit settlement_id via dev tooling
  → save / load preserving all membership
```

Required Dev tooling for this milestone:

- Place a `SettlementAnchor` (invokes the shared `create_settlement` function, including overlap
  validation)
- Visualize the settlement boundary in Dev / Build-Mode context
- Place buildings such that a building **inside** the boundary is seeded with that settlement's id,
  and a building **outside every settlement** receives `None`
- Explicitly assign `settlement_id` on **selected units** (proximity must not auto-enroll them)
- Persist and restore the anchor, center, radius, and both membership fields

Explicit unit assignment for selected units is a required Dev mechanism for this milestone, not a
convenience. Building membership is seeded at placement; a later Dev reassignment tool is not
required now. Settlement **removal** is out of scope.

### Anchor visibility and selection

Target behavior:

- anchor is **selectable** in Build Mode / appropriate dev tooling
- settlement **boundary is visualized** in Build Mode / dev context
- anchor is **not visible** in ordinary gameplay

Eventual selection exists so the player can inspect and manage a settlement. Settlement removal is
out of scope.

**TEMPORARY MILESTONE BEHAVIOR:** if conditional visibility adds unnecessary milestone work, the
anchor may remain always visible and dev-selectable. This is **explicitly temporary and is not final
UX**. It is replaced when player Build Mode placement is generalized.

### Membership lifecycle

- **Unit death clears `settlement_id`** in the death pipeline (ADR-059). A dead unit is not part of
  the live population and contributes to no roster and no demand. Future corpse, history, or
  accounting needs must use separate data rather than keeping dead units in the active roster.
- Building membership persists for the lifetime of the building record; cache rebuild always
  reflects the record.

### Physical access is not demand legitimacy

Two questions are separate and must not be conflated:

| Question | Answer |
|---|---|
| Can this unit physically take that food? | Existing inventory access rules (ADR-090/ADR-091) |
| Does this unit count as settlement demand? | Only if `settlement_id` matches |

Non-member and other-settlement units may physically take items. They are **never** counted as
legitimate settlement demand. Taking without permission is future theft/crime behavior and is out of
scope; no theft mechanics are introduced here.

### Member counting is infrastructure, not a Need

Authoritative live member count (units whose `settlement_id` matches, excluding the dead) is required
so food demand and future systems can scale to population.

**Member count exists ≠ Population is an active settlement Need.**

ADR-117 may document member counting as a query. The Population / `UnitCount` need evaluator remains
deferred. Food desired may consume member count without creating Population pressure.

### Persistence

Persisted: the anchor, center, radius, and the explicit `settlement_id` on units and buildings.
Rosters and indices are rebuilt on load.

`SCENE_VERSION` increments. Legacy scenes load as follows: units and buildings receive `None`, and
each existing `SettlementRecord` synthesizes an anchor at its legacy anchor building's position with
the default initial radius.

## Rejected designs

- **Settlement as a `BuildingRecord` or special `BuildingDefinition`** — imports construction
  lifecycle, vitals, occupancy, interiors, and navigation authority that a settlement origin does not
  have, and forces special cases across every building consumer.
- **Membership derived from affiliation / faction / team / ownership** — the current defect. Breaks as
  soon as two settlements share an affiliation.
- **Membership continuously derived from boundary containment** — makes strategic ownership a
  function of position and lets it flip silently.
- **Hardcoded exclusion radius constant** — breaks when radius differs between settlements or changes
  later.
- **A dev-only settlement creation path** — guarantees a second, divergent player path later.
- **Proximity auto-enrollment of units** — implicit membership with no author intent.
- **Keeping dead units in the roster** — inflates member counts and food demand.
- **Radius as an immutable definition constant** — forecloses later growth for no benefit.

## Consequences

- `reconcile_settlement_building_membership` is retired.
- SA2 food demand can finally be scaled to member population (ADR-117 amendment).
- ADR-093 treasury semantics shift: the treasury belongs to the settlement, and the anchor object
  replaces `anchor_building_id` as identity.
- `SettlementOwnership::from_building_ownership` should derive from the placing actor at creation
  rather than from a building.
- `SettlementKind::Hive`, `Pack`, and `Herd` (ADR-116) become questionable, because a nomadic group
  has no placed anchor and no non-overlapping boundary. Recorded as a deferred design question in
  ADR-115; not scheduled and not solved here.
- ADR-093 currently hosts settlement treasury interaction on the `settlement_core` **building**. After
  this ADR, treasury identity belongs to the settlement (via the anchor), not to a building id. How
  players deposit gold without a `settlement_core` identity-building is **deferred** — do not force the
  anchor through `BuildingRecord` to preserve that interaction.
- Ordinary-work eligibility (`UnitWorkCapabilities` boolean gates) is **not** this ADR; see the
  ADR-115 / ADR-122 amendments.
- Dev mode gains anchor placement, boundary visualization, and explicit unit assignment
  (`docs/dev-mode.md`, `docs/dev-ui.md`).

## References

- ADR-115 (settlement AI architecture — §4/§20 corrected), ADR-116 (SettlementState), ADR-093
  (treasuries), ADR-114 (EP9)
- ADR-051 (ownership and affiliation — explicitly not membership authority)
- ADR-059 (unit death pipeline), ADR-045 (scene snapshots), ADR-081 (placement and ghost validation)
- ADR-134 (individual hunger — consumer of member population)
- ARCHITECTURE.md Settlement AI section
