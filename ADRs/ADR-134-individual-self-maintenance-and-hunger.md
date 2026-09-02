# ADR-134: Individual Self-Maintenance & Hunger

## Status

Accepted (design direction — implementation pending)

## Context

ADR-115 gave settlements strategic needs, and ADR-117 (SA2) computes a food need from settlement
stock against an authored target. A 2026-08-28 audit found that **nothing in the simulation consumes
food**. Food stock therefore only ever rises, food pressure decays to zero, and the entire SA2–SA7
pipeline has no reason to act. The settlement AI is structurally complete and behaviourally inert.

The missing piece is a consumer. The design question was whether that consumer is the settlement
(an abstract upkeep drain) or the individual unit.

ADR-071 already lists hunger as a "current state" input to the layered creature decision model but
never implemented it, and left the individual decision layer unspecified.

## Decision

### Hunger is individual unit state

Hunger belongs to the **unit**, not to the settlement. Units get hungry, seek food, travel to it, and
eat it. The settlement never holds an abstract food-consumption pool.

Hunger lives on `UnitRecord` as its own field. It is **not** part of `UnitVitals`, which ADR-055 owns
as combat HP and which `normalize_restored_unit` deliberately overwrites from the catalog on restore —
placing hunger there would silently reset it on every load.

### No generic upkeep framework

Hunger is **not** generalized into a `RecurringConsumer` / upkeep abstraction covering fuel,
maintenance, wages, or other recurring costs.

Solve hunger as hunger. The Groundwork Rule (AGENTS.md) forbids building the general system before
consumers exist, and one consumer is not a category. If several recurring costs later appear with
genuinely shared semantics, generalize then, with evidence.

### Food is a category, not an item

Edibility is determined by item **category** (`ItemCategoryId` `"food"`). How much eating restores is
authored **nutrition** on the item (ADR-087 / ADR-117), not “one stack = one meal.” No unit logic
references a hardcoded item id such as `bread`.

Eating restores hunger by that item’s nutrition, clamped to the unit’s authored hunger maximum.

### Two eating behaviors, both permanent

| Situation | Behavior |
|---|---|
| Unit already carries food | Eat from own inventory |
| Unit does not carry food | Travel to accessible food storage/source and consume it there |

**Eat-at-source is a legitimate end-state behavior, not technical debt.** There is no requirement that
a unit first transfer food into its own inventory before consuming it. Neither form is scheduled for
removal.

This also avoids an inventory-full failure path that a mandatory transfer step would create.

### Two hunger stages with different interruption authority

| Stage | Milestone placeholder | Behavior |
|---|---|---|
| **Normal hunger** | ~50% remaining | Seek food **between** ordinary tasks. Do **not** interrupt current work merely for being somewhat hungry. |
| **Critical hunger** | ~10% remaining | **Interrupt** ordinary work and other non-combat activity to obtain food. |

**Combat outranks hunger at every stage.** Hunger never interrupts combat.

Percentages, decay rate, and hunger maximum are **tunable authored data** on the unit definition, not
architectural constants. Milestone values (~50% / ~10%, test-friendly decay) are placeholders.

Hunger remaining and item nutrition must use **compatible units** so eating a Prispod (nutrition 25)
and aggregating settlement food value are the same currency (ADR-117).

The architectural consequence is that the self-maintenance layer must express **urgency**, not a
boolean `is_hungry`. It distinguishes "I should eat when convenient" from "I need to stop what I am
doing and eat", and those two answers have different authority over an in-progress activity.

### Authority boundaries

Self-maintenance is the first concrete piece of ADR-071's individual decision layer
(`current state → decision`). It obeys the same boundaries relationship code obeys (ADR-132):

- It **never** writes `CombatState` and never touches combat targeting.
- It acts through `UnitOrder`, which remains the action boundary.
- It is **not** a second worker-assignment authority. It may release an in-progress task through the
  existing marketplace release path (ADR-122), but it never claims tasks, never assigns units, and
  never moves workers directly.

### First come, first served — no reservations

Food is not reserved or claimed. If several units travel toward the same remaining food and one
consumes it first, the others arrive and find nothing; they simply re-evaluate and seek food again.

This is acceptable and arguably desirable world behavior. No claim state is introduced, and therefore
none has to be persisted, validated, or released.

### No starvation consequences in this milestone

This milestone delivers hunger state, food seeking and eating, and the resulting settlement demand.
It delivers **no** consequence for being hungry.

Long-term intent, **recorded and not scheduled**: progressive hunger causes broad stat reduction, then
reduced maximum health, and eventually death — with actual starvation death intentionally slow and
difficult to reach.

The existing `starvation` emergency (ADR-123) remains what it already is: authored pressure
reweighting. It is informational, and it is not a damage source.

### Membership and demand

Hunger is individual, but **demand is settlement-scoped**. Settlement food demand aggregates only
**live members** — units whose `settlement_id` matches the settlement (ADR-133). Dead units have
already cleared membership and must not contribute.

This uses live members’ authored consumption over a horizon **plus** an authored reserve, in
**nutrition units** (ADR-117). It does **not** create or activate a Population need.

Non-member and other-settlement units may physically take food under existing inventory access rules,
but they are never counted as legitimate settlement demand. Unauthorized taking is future theft/crime
behavior and is out of scope.

```text
physical ability to take food
    ≠
settlement considers that unit part of its food demand
```

### Persistence

Hunger persists per unit. Restore must **not** normalize it from the catalog the way HP is normalized.

## Rejected designs

- **Settlement-level abstract food consumption** — hides the behavior that makes a settlement legible,
  and gives units nothing to do.
- **Generic `RecurringConsumer` / upkeep framework** — one consumer is not a category; violates the
  Groundwork Rule.
- **A single hardcoded food item** — food is a category, as SA2 already treats it.
- **Boolean `is_hungry` with one threshold** — cannot express the difference between opportunistic and
  interrupting hunger.
- **Hunger interrupting combat** — combat priority is a design invariant (ADR-069).
- **Food reservations / claims** — unnecessary state; contention resolves acceptably by re-seeking.
- **Hunger inside `UnitVitals`** — restore normalization would erase it.
- **Starvation damage now** — consequences are deliberately deferred.
- **Self-maintenance claiming tasks directly** — would create a second worker-assignment authority
  competing with ADR-122.

## Consequences

- SA2 food pressure becomes genuinely dynamic: consumption drains stock, pressure rises, the
  settlement responds, production refills, pressure falls.
- Food demand can be derived from member count and authored per-unit consumption (ADR-117 amendment)
  **without** activating a Population need.
- `UnitState` / order handling must accommodate an eating activity and a hunger-driven release of
  ordinary work, without granting hunger authority over combat.
- Unit definitions gain authored hunger characteristics (maximum in nutrition units, decay rate,
  normal/critical **fractions** of that maximum). Thresholds and decay are data, not code constants.
- Item definitions gain authored `nutrition` for food-category items (ADR-087).
- Dev tooling should surface per-unit hunger and stage so the milestone scenario is diagnosable.

## References

- ADR-071 (creature AI — individual decision layer this begins to fill)
- ADR-133 (settlement identity and membership — scopes demand)
- ADR-117 (need evaluation), ADR-123 (emergencies), ADR-122 (worker assignment / task release)
- ADR-087/ADR-088 (item categories, inventory), ADR-090/ADR-091 (transfers, building containers)
- ADR-055 (unit vitals — explicitly not hunger's home), ADR-069 (combat priority)
- DESIGN.md (world and food)
