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

> **AMENDED 2026-08-28; SUPERSEDED IN PART the same day** — see both amendments below.
> Food is no longer a stub. Construction remains **building-backlog** pressure. Materials is a
> **separate** stock need. Population / `UnitCount` remains deferred as a Need. Food current is
> **nutrition**, not item count.

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

### Category-driven sensing, not one numeric meaning

> **SUPERSEDED 2026-08-28 (later the same day)** — forcing food and stone through one count-based
> `CategoryStock` evaluator. See [Amendment: nutrition vs count](#amendment-2026-08-28--food-nutrition-vs-material-count).
> Category as the *discovery* key remains.

Replace per-need inventory special cases (`FoodStock`, `LuxuryStock`, and any item-id matching) with
**category-driven** aggregation: which items count is `ItemCategoryId`, never a hardcoded item id.

Food and materials are **not** required to share one value function. Stone is quantity. Food is
nutritional value (amendment below).

SA2 still does not generate actions.

### Food desired uses member demand plus a reserve — not a Population need

Food **current** is settlement-scoped **available food value** (sum of authored nutrition × quantity
for food-category items in member inventories). 100 raw items and 100 meals are not equivalent.

Food **desired** is, in the **same nutrition units**:

```text
desired = expected member consumption over an authored horizon
        + desired reserve / stockpile buffer
```

- Live members only (`settlement_id` match; dead units do not count)
- Authored per-unit consumption characteristics (ADR-134), compatible with item nutrition
- Reserve/buffer is authored (`NeedTarget` and/or need definition) — exact numbers are tuning

**Satisfying immediate consumption does not stop food production.** If the reserve is short, food
pressure is low but **nonzero**. SA4 arbitrates that against other needs. If stone (or anything else)
is more urgent, workers do that. If other needs are satisfied, producing surplus food is valid.

**Do not** add a special rule “once minimum food is met, never produce food.” Reserve-building is
normal need pressure.

**Member count exists ≠ Population is an active Need.** The Population / `UnitCount` evaluator remains
**deferred**.

### Materials stock is the second competing need (not Construction)

The milestone's second need is **material stock**, not construction backlog.

| Need | Meaning |
|---|---|
| `construction` | Unfinished buildings / construction backlog (`ConstructionSites`) |
| `materials` | Insufficient construction-material **count** (`NeedCategory::Materials`) |

- **Do not** reuse `NeedCategory::Construction` or `NeedEvaluationMethod::ConstructionSites`.
- Item category: dedicated `construction_material` (or equivalent); assign `stone` to it. Do **not**
  measure `raw_material` and do **not** special-case item id `stone`.
- Materials **current** = quantity of that category. Materials **desired** is authored.
- Rebind `increase_construction_materials` from NeedId `construction` to NeedId `materials`.

### Amendment (2026-08-28) — food nutrition vs material count

Forcing food and stone through one count-based `CategoryStock` was too much generalization. Categories
still decide *which items participate*. **What the number means is need-specific.**

Simplest architecture (do **not** invent a generic item-value framework):

1. **Shared helper:** aggregate matching-category items from settlement-accessible inventories
   (membership-scoped, as today).
2. **`CategoryCount { category }`:** `current = sum(quantity)` — materials/stone.
3. **`CategoryNutrition { category }`:** `current = sum(quantity × item.nutrition)` — food.
   `nutrition` is an authored field on `ItemDefinition` (ADR-087). Non-food items are 0 and must not
   be treated as meals.

Luxury may later use `CategoryCount` once it has a real category. Do not keep substring matching.

Current and desired for a given need **must use the same unit**. Mixing item-count desired with
nutrition current is invalid.

Long-term, an entire production chain may satisfy food (farm → flour → bread). EP9's production graph
is the existing place for that reasoning (ADR-114). **This milestone** uses a short chain:
Prispod farm → directly edible Prispod with nutrition. That is a **validation simplification**, not a
rule that farms always output ready-to-eat food.

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
