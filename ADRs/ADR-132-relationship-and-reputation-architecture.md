# ADR-132: Relationship and Reputation Architecture

## Status

Accepted (architecture foundation — **not** an implementation phase)

## Purpose

This ADR defines the **authoritative long-term architecture for directional relationships and
reputation**. Every future relationship, reputation, perception-desire, and social-consumer
implementation phase must conform to it.

It does **not** implement systems. It defines the conceptual model, identity ownership, the
authored/mutable layering, the numeric contract, the authoring contract, and the boundaries this
system deliberately does not own.

Where this document and an accepted implementation ADR disagree about *what exists today*, the
implementation ADR wins. Where they disagree about *intended structure*, this document wins and the
implementation should be reconciled toward it.

Design narrative: [DESIGN.md](../DESIGN.md#relationships-and-reputation).
Authoring contract: [docs/relationship-authoring.md](../docs/relationship-authoring.md).

---

## 1. What exists today (implemented)

**Phase 1–7 implemented (locked Map complete).** Autonomous desire consumes `effective_relationship()`
through `autonomous_wants_to_attack` with a primitive `-100` combat-AI threshold (not universal
relationship semantics). The Affiliation hostility matrix no longer owns proactive desire.
Perception acquisition uses per-unit Sight Range (Phase 4).

| Concern | Current implementation |
|---------|------------------------|
| Autonomous desire | `effective_relationship()` + `HOSTILE_RELATIONSHIP_THRESHOLD = -100` in `src/world/combat/autonomous_desire.rs` |
| Authored baseline | `wild -> player = -300` in `Rel Faction Faction` (workbook) |
| Mechanical alliance | `UnitRecord.team_id` equality veto |
| Weapon target permission | `WeaponDefinition.target_filters` — mechanical class only (Phase 5) |
| Self-defense | `UnitRecord.reactive_combat_target` — attacker-specific authorization (ADR-062) |
| AI candidate detection | `perceived_units` + authored Sight Range via bounded `query_units_in_radius` (Phase 4) |
| Design-time faction label | `UnitDefinition.faction_tag` — content metadata, gameplay-inert (ADR-051) |
| Relationship resolver + Standing | Implemented (Phases 2–3); gameplay consumption via autonomous desire (Phase 6); vertical validated (Phase 7) |

---

## 2. Conceptual model (accepted direction)

Relationships are **directional and conceptually N×N**:

```text
relationship(A -> B)
```

`A -> B` is independent of `B -> A`. Every individual unit may theoretically diverge toward every
other individual unit.

Each relationship-capable subject carries a set of **identity facets**. Relationship is defined as
the sum over the Cartesian product of the two facet sets:

```text
effective_relationship(A -> B)
  = Σ  s ∈ facets(A)
    Σ  t ∈ facets(B)
        edge_value(s -> t)          // absent edge contributes 0
```

**The N×N model is a function, not a table.** It is never materialized. Resolution cost is
`|facets(A)| × |facets(B)|` sparse lookups — independent of unit count, chunk count, and world size.
Inheritance, lazy materialization, and caching are therefore **not** required by the model and must
not be introduced without profiling evidence (ARCHITECTURE.md Performance Philosophy).

### Initial domains

| Domain | Facet source | Mutable at runtime |
|--------|--------------|--------------------|
| `Faction` | Authored default on the unit definition, **seeded into the runtime unit record** | Yes (planned) |
| `Species` | Authored unit identity from a real Species catalog | No |
| `Individual` | Derived from `UnitId` — never stored as a field | n/a |

"Race" and "Species" mean the same thing in Chasma. **Use Species.** Additional domains (culture,
religion, settlement, organization) may be added when genuinely needed; adding one must not change
the resolver, the store, or any consumer.

`SpeciesId` is a stable semantic key (`human`, `yaratan`, `cavecrawler`) from its own catalog.
It is **not** `UnitDefinitionId` — multiple unit definitions must be able to share a species, which
is what makes cross-domain Species relationships authorable.

### Composition authority

A **single** function assembles a unit's facets. Consumers never build facet sets themselves. This
is what keeps "add a domain" a one-place change, and it is where future disguise or
mistaken-identity behavior would live.

### Combination rule

All applicable directed contributions **stack additively**. There is:

- no precedence
- no "most specific source wins"
- no weights, multipliers, or nonlinear combination
- no implicit override of a general source by a specific source

A missing edge contributes 0. If a future combination semantic is ever justified, the architecture
must not make it impossible — but none is introduced now.

---

## 3. Numeric contract

- Relationship values are **signed integers** (`i32`). Integer arithmetic is required for
  determinism (ARCHITECTURE.md multiplayer compatibility), exact test equality, and drift-free
  accumulation of mutable deltas.
- **No hard clamp.** Values outside the ordinary useful range are legal and meaningful: a creature
  may sit at roughly −300 precisely so that small positive modifiers do not quickly overcome a
  deeply negative baseline.
- Additive stacking is therefore unbounded by construction.
- Relationship truth exposes **no semantic bands** (`Hostile`, `Friendly`, …). Banding is
  interpretation, and different consumers need different cutoffs. Consumers may saturate or
  threshold; truth does not.

---

## 4. Layers

| Layer | Contents | Mutable | Persisted |
|-------|----------|---------|-----------|
| **Disposition** | Authored baseline from Excel matrices | No — rebuilt from the workbook | No |
| **Standing** | Sparse mutable divergence, group- or individual-keyed | Yes | Yes — changed edges only |

```text
authored Disposition
  + persistent mutable Standing
  + future legitimate contribution sources
  = effective relationship
```

**Personal unit-to-unit divergence is not a parallel subsystem.** It is the Standing layer keyed by
`Individual` facets — the same machinery, the same keyspace.

Standing stores **deltas**, not absolute overrides. An override would be a precedence rule (§2) and
would destroy provenance. Curves, caps, diminishing returns, and asymmetric event effects belong to
the **single mutation seam**, not to storage and not to the resolver — that is where
"enslave −200 / free +100" style policy will eventually live.

### Regional and contextual contributions

The universal edge key stays `(source, target)`. It must **not** become
`(source, target, scope)` in anticipation of regional reputation. Regional, local, or contextual
systems may later contribute additional values *through* the resolver using whatever storage suits
those systems. The core N×N primitive remains source → target.

---

## 5. Identity is not ownership

Five concepts stay separate. None absorbs another.

| Concept | Authority | Answers |
|---------|-----------|---------|
| Control | `UnitRecord.owner_id` | Who commands this unit |
| Mechanical team | `UnitRecord.team_id` | Absolute ally/enemy rules — an **absolute veto**, never negotiated by a number |
| Runtime affiliation | `UnitRecord.affiliation` | Coarse controllability / UI classification |
| Design-time label | `UnitDefinition.faction_tag` | Content metadata (ADR-051) |
| Relationship identity | Relationship facets (runtime-authoritative) | Which groups this unit belongs to |
| Relationship value | Disposition + Standing | What A thinks of B |

Explicitly forbidden:

- Adding `HostileAnimal` / `NeutralAnimal` style `Affiliation` variants to express relationships.
  `Affiliation` equality is load-bearing for settlement building membership
  (`src/world/settlement/membership.rs`) and worker task eligibility
  (`src/world/task/eligibility.rs`); new variants would silently re-partition those systems.
- Deriving relationship identity from `faction_tag` as a display label.
- Deriving `TeamId` from relationship values.
- Overloading `OwnerId` with relationship meaning.

See ADR-051 for why a definition-supplied relationship *default* is not ownership derivation.

---

## 6. Authoring

Excel is authoritative for the Disposition layer. Authoring uses **directional matrices**: rows are
the source/observer, columns are the target, and each cell is the row's relationship toward the
column. Matrices are asymmetric by design. Runtime storage normalizes them into sparse edges.

Row-per-pair authoring is **rejected** as the authoring surface.

Identity keys are stable readable semantic slugs — `player`, `wild`, `trinity` for factions;
`human`, `yaratan`, `cavecrawler` for species. Legacy `F-000X` identifiers may remain as authoring
cross-reference but are **not** the runtime relationship identity, and must not appear as matrix
headers.

One authored faction identity authority: units reference the stable faction key, and presentation
resolves the display name from the Factions catalog rather than each unit carrying an independent
display name.

The existing one-dimensional `Factions.Disposition` column is **retired** rather than allowed to
coexist as a second relationship authority.

Full contract: [docs/relationship-authoring.md](../docs/relationship-authoring.md).

---

## 7. Resolution and observability

A single relationship authority exposes both a value and its provenance:

```text
effective_relationship(A, B)  -> i32
explain_relationship(A, B)    -> contributions + total
```

**Both must be produced by the same internal calculation path.** A separately written explanation
will eventually disagree with the value, and the diagnostic becomes the bug. Provenance identifies
source facet, target facet, contribution value, contributing layer, and the final total.

No cache and no pre-aggregation until profiling proves necessity. The single query function is the
seam through which a cache could later be added without consumer churn — but invalidation would
have to cover Standing mutation, faction change, membership change, and unit death, which is
precisely why it is not introduced now.

Diagnostics follow the existing `#[cfg(feature = "dev")]` `runtime_trace` idiom (zero-cost in
release, prefixed single lines). No dev window, overlay, hotkey, or inspector is approved.

---

## 8. Relationship is not behavior

Relationship answers *what does A think of B*. It never answers *should A attack B*.

Relationship code must never write `CombatState`, never set `attack_cycle`, and never touch
`reactive_combat_target`. `UnitOrder` remains the sole action boundary (ADR-071).

Intended flow:

```text
perceive candidate
  -> resolve A -> B relationship
  -> behavior / personality / state interprets it
  -> choose intent
  -> existing UnitOrder API
```

### Four separate questions

Current combat previously conflated these. **Phase 5 implemented** the split (ADR-056, ADR-062):

| Question | Basis |
|----------|-------|
| **Mechanical targetability** — can this target be attacked at all? | Alive, weapon exists, weapon target class, self, `TeamId` |
| **Explicit player intent** — may the player deliberately attack it? | Mechanical targetability only; relationship is **not** an invisible protection mechanism |
| **Default interaction intent** — what does a plain right-click mean? | Conservative UI affordance; uses autonomous desire, not explicit legality |
| **Autonomous desire** — does an AI unit *want* to attack? | `effective_relationship()` + primitive `-100` combat-AI threshold (Phase 6); future behavior templates may interpret differently (ADR-071) |

Same-team and own-unit attack semantics are **deferred**; existing behavior is preserved.

For the first primitive implementation, a sufficiently negative relationship may be interpreted by
the placeholder combat AI as Attack. That is a temporary behavior consumer, **not** a permanent
equation `negative relationship == attack`.

### Weapon target filters

`WeaponDefinition.target_filters` must describe **target class** — what a weapon can mechanically
engage — not social permission. `Enemies`, `Wildlife`, and `Neutral` currently encode social
hostility and are a second, hidden relationship authority; they are dissolved toward a target-class
vocabulary, with backward-compatible importer aliases during migration.

After migration, weapon targetability consumes **no** `Affiliation`, ownership hostility, or
relationship input.

### Retaliation

Immediate self-defense remains the existing attacker-specific combat authorization
(`reactive_combat_target`). It must **not** become faction reputation: a shared reputation delta
would make an entire faction instantly aware of an incident, which is the omniscience this
architecture forbids (§9).

Relationship changes alone do not invalidate an already-fired projectile. Mechanical target
invalidity remains a separate concern.

---

## 9. What this architecture does not own

Four stages, four owners. Relationship architecture stops at stage 4's write seam.

| Stage | Owner | Status |
|-------|-------|--------|
| 1. **Incident** — what factually happened | Future incident system | Deferred |
| 2. **Observation** — who knows it happened | Future knowledge system | Deferred |
| 3. **Attribution** — who is blamed | Future culpability policy | Deferred |
| 4. **Mutation** — resulting Standing deltas | Relationship mutation seam | Seam only |

Information propagates; reputation does not propagate magically. A faction must not know an incident
merely because it occurred. Player control does not imply culpability — responsibility attaches to
actual actors, and victim or self-defender roles attribute no blame.

When stage 2 is built it should follow ADR-115's precedent: a knowledge accessor that is initially a
pass-through, so uncertainty can be introduced behind it without rewriting consumers.

---

## 10. Persistence

The Disposition layer is **not** persisted — it is rebuilt from the workbook, like other catalogs.

The Standing layer is authoritative world state and must persist. There is no player save system
today; dev scenes are the only persistence mechanism (ADR-045). Standing therefore persists through
the dev-scene snapshot, following the established additive `#[serde(default)]` sub-struct pattern
used by `SceneSettlementStatePersistence` and `SceneConstructionPlanPersistence`.

Serialization stores **stable string/numeric identity keys**, never interned indices, so saves
survive catalog reordering.

---

## 11. Planned implementation (approved Map)

Phases 1–7 are implemented.

| Phase | Scope | Expected gameplay change |
|-------|-------|--------------------------|
| 1 | Faction / Species / Individual identity foundation and catalogs | None |
| 2 | Directional relationship matrix importer + authored store | None |
| 3 | Additive N×N resolver + provenance + sparse Standing + dev-scene persistence | None |
| 4 | Minimum perception seam — per-unit Sight Range + bounded spatial query | Acquisition range becomes unit-authored instead of two magic constants |
| 5 | Mechanical targetability / explicit intent / default interaction / autonomous desire separation, incl. `TargetFilter` cleanup | Explicit player-attack legality broadens; default right-click uses desire seam |
| 6 | Relationship-driven primitive autonomous desire | **Implemented** — wild units can proactively hate player units at authored `wild -> player = -300`; reverse direction may remain `0` |
| 7 | Vertical validation and documentation completion | **Implemented** — locked Map complete; no new gameplay systems |

Phase 6 replaced the temporary Affiliation matrix in `autonomous_wants_to_attack` with
`effective_relationship()` interpretation at a `-100` placeholder threshold.

Phase 7 validated the full vertical from identity through authored data, resolver, perception,
targetability/intent split, relationship-driven desire, and existing combat execution. No new
relationship features were added.

---

## 12. Deferred (must not be pulled forward)

Incidents, crimes, witnesses, reporting and information propagation, detailed culpability,
reputation event curves and diminishing returns, personality, full creature behavior templates,
regional standing, LOS and occlusion, `PER` influence, hearing and smell, perception memory and
alertness, trade and recruiting consumers, friendly-fire and own-unit attack design, relationship
UI, dev relationship inspector or overlay, caching and precomputed relationship aggregates.

---

## 13. Consequences

- Combat gains a coherent legality/desire split; `Affiliation` loses its role as the AI hostility
  authority without being replaced as ownership.
- Weapon catalog data stops encoding social relationships.
- Perception becomes the single candidate-enumeration authority, replacing two hard-coded radii and
  the global unit scan with the existing bounded chunk-local query.
- Adding a relationship domain, a faction, or a species becomes a data change.
- Relationship truth is observable by construction, so future diagnostics cannot disagree with the
  simulation.

## References

- ADR-051 (ownership and affiliation), ADR-056 (combat targeting), ADR-062 (combat AI and
  retaliation), ADR-069 (combat philosophy), ADR-071 (creature AI)
- ADR-115 (settlement AI architecture — knowledge seam precedent), ADR-045 (dev scenes)
- [ARCHITECTURE.md](../ARCHITECTURE.md), [DESIGN.md](../DESIGN.md), [ROADMAP.md](../ROADMAP.md)
- [docs/relationship-authoring.md](../docs/relationship-authoring.md)
