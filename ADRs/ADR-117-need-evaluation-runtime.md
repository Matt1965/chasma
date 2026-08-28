# ADR-117: Need Evaluation Runtime (SA2)

## Status

Accepted

## Context

ADR-115 defined Settlement AI around a weighted-need arbiter. ADR-116 (SA1) introduced persistent
`SettlementState` (targets, modifiers, policy) without computing anything.

SA2 teaches a settlement how to **evaluate itself**: compute current values, desired values, and
normalized pressure for each need. It does not decide actions, generate tasks, or mutate production.

## Decision

### Needs are computed, never authoritative persistent objects

`NeedDefinition` entries are authored catalog content (`NeedCatalog`). Runtime results are
`NeedSnapshot` values held in a transient `NeedEvaluationStore` on `WorldData`.

Nothing produced by Need Evaluation is persisted. After save/load the store is cleared and snapshots
rebuild on the next evaluation.

### NeedSnapshot is the evaluation output

Each snapshot carries:

- NeedId
- Current / desired values
- Deficit / surplus
- Normalized pressure `0..=100`
- Optional blocking reason
- Trend seam (unused in SA2)
- Evaluation tick + source diagnostic string

Snapshots are rebuilt whenever evaluation runs.

### Pressure is the universal output

```
pressure = clamp(round((max(0, desired - current) / desired) * 100), 0, 100)
```

When `desired <= 0`, pressure is `0`. Settlement modifiers (matching need id or `"all"`) and matching
emergency flags may adjust pressure within `0..=100`. Future systems consume pressure only — never
raw inventory counts.

### Independent evaluation

Each need computes Current → Desired → Pressure independently. No need inspects another need. No need
generates actions or mutates SettlementState / inventories / buildings / workers.

### Evaluation cadence

`step_settlement_need_evaluation` runs during the simulation tick (before EP9 production planners)
when:

- the settlement's need-store dirty flag is set, or
- no prior snapshot exists, or
- `NEED_EVAL_CADENCE_TICKS` (30) have elapsed since the last evaluation

Dirty hints come from `mark_settlement_state_dirty` (inventory/building/policy invalidation seams).
Need dirty lives on `NeedEvaluationStore` so evaluation never clears EP9 `planner.dirty`.

### First needs (architecture exercise only)

> **AMENDED 2026-08-28** — see [Amendment: CategoryStock, member demand, stone](#amendment-2026-08-28--categorystock-member-demand-and-stone).
> Food is no longer a stub. Construction remains **building-backlog** pressure and is not material
> stock. Population / `UnitCount` remains deferred as a Need.

Food, Construction, Housing, Defense, Research, Expansion, Luxury — measurement stubs sufficient to
exercise the catalog/snapshot/pressure path. No Response behaviors.

### Validation

Catalog construction rejects duplicate NeedIds and unknown evaluators. Snapshot validation rejects
pressure outside `0..=100`, non-finite values, negative desired, and broken deficit accounting.

## Rejected designs

- **Persistent Need objects** — pressures are derived; persisting them violates the rebuild principle.
- **Needs generating actions** — SA2 reports state only; response selection is SA3+.
- **Cross-dependent Need calculations** — each need is independent; coupling belongs in a later arbiter
  that reads pressures, not inside evaluators.
- **Clearing SettlementState/EP9 dirty from need eval** — need dirty is a separate transient flag.

## Consequences

- Dev inspector shows need current/target/pressure/modifiers/source/diagnostics.
- SA3 Response Engine (ADR-118) reads pressures from `NeedEvaluationStore` and produces
  `CandidateResponse` options — it does not invent parallel sensors.
- Scene restore clears `NeedEvaluationStore`.

## Amendment (2026-08-28) — CategoryStock, member demand, and stone

### Why

Food stock was measured correctly via `ItemCategoryId`, but **nothing consumed food**, and food
desired was an authored constant. Housing, defense, and luxury measured world state by **building or
item id substring**. The existing `Construction` need counted incomplete buildings (backlog), which
is a valid need and must not be overloaded as "we are short on stone."

A 2026-08-28 design pass also locked individual hunger (ADR-134) and explicit membership (ADR-133).
SA2 must observe those, not invent a second population system.

### CategoryStock is the generic category-stock evaluator

Replace per-need inventory special cases (`FoodStock`, `LuxuryStock`, and any item-id matching) with
one evaluation method:

```text
NeedEvaluationMethod::CategoryStock { category: ItemCategoryId }
```

Food and the milestone's second competing need (construction-material / stone stock) are the **same
evaluator with different authored data**. Luxury can migrate onto the same method when it has a real
category; it must not keep `id.contains("luxury") || id == "iron_bar"`.

SA2 still does not generate actions. CategoryStock only reports current stock vs desired.

### Food desired uses member demand — not a Population need

Food **current** remains settlement-scoped food-category stock (ADR-133 membership for which
inventories count as settlement stock).

Food **desired** uses **live member** consumption:

- count units whose `settlement_id` matches (dead units have `None` and must not count)
- apply authored per-unit consumption characteristics (ADR-134)
- optionally compose with an authored buffer / `NeedTarget` stockpile — exact composition is
  implementation, but member demand **must** be an input

**Member count exists ≠ Population is an active Need.**

This ADR may document an authoritative member-count query because food demand (and future systems)
need it. The Population / `UnitCount` need evaluator remains **deferred**. Do not restore it because
the count is now available. Food pressure is the food need; there is no Population pressure in this
milestone.

Non-members may physically take food later; they are **never** legitimate food demand.

### Stone / construction-material stock is the second competing need

The milestone's second need is **material stock**, not construction backlog.

- Author a `NeedDefinition` whose evaluation method is `CategoryStock` for a construction-material
  category.
- **Do not** reuse `NeedCategory::Construction` or `NeedEvaluationMethod::ConstructionSites` for this.
  Construction remains incomplete-building backlog pressure and stays available but is not the
  competing need.
- Stone desired is **authored** (`NeedTarget` / definition default), not member-driven. Combined with
  dynamic food desired, food-vs-stone worker reallocation can emerge from scoring when one pressure
  overtakes the other — not from scenario scripting.

### Item category for stone (content prerequisite)

Starter `stone` is currently `raw_material` alongside iron ore and coal. Measuring `raw_material`
would mix unrelated stocks and is **not** acceptable.

Confirm or author a dedicated item category (e.g. `construction_material`) and assign `stone` to it
so CategoryStock stays generic. Do **not** special-case item id `stone` in the evaluator.

If `NeedCategory` has no suitable variant for this need, add one for materials / construction-stock.
Do not overload Construction, Population, or Economy.

### Housing, defense, and remaining substring sensors

Housing (`hut`/`house`/…) and defense (`wall`/`tower`/…) still match definition ids by substring, and
defense mixes **policy** (`aggression`) into measured current. Those evaluators are **not** the
milestone's competing pair. They are recorded as deferred defects in ADR-115: buildings should
declare what they provide, and SA2 must measure world state only. Do not drive milestone acceptance
from them.

### Pressure vs weight

SA2 pressure remains the unweighted `0..=100` shortage signal defined above. Authored `NeedTarget.weight`
does **not** belong in SA2. Weight shapes SA4 urgency (ADR-119 amendment).

### Preserved

- Independent per-need evaluation; no actions; nothing persisted.
- ConstructionSites backlog measurement is unchanged in meaning.
- Emergency modifiers may still adjust pressure within `0..=100` (ADR-123).

## References

- ADR-115, ADR-116, ADR-114, ADR-133, ADR-134, ADR-087
- ADR-118, ADR-119
- ARCHITECTURE.md Settlement AI section
