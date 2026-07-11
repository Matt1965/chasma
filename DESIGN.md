# DESIGN.md

# Purpose

This document captures **game design direction** for Chasma — goals, philosophy, and
planned mechanics that are not yet fully implemented.

It complements:

- **ARCHITECTURE.md** — how systems are built (authority, layers, scalability)
- **ROADMAP.md** — implementation order and current status
- **ADRs/** — accepted technical decisions for implemented foundations

When design intent conflicts with an accepted ADR, the ADR describes **what exists today**.
This document describes **where the game is headed**. Implementation ADRs should gain
"Design Direction" sections that point here rather than silently diverging.

Design content here is intentionally **draft** unless cross-referenced by an accepted ADR.
Prefer predictability and player agency over hidden simulation cleverness.

---

# Overall Design Philosophy

Recurring principles across combat, economy, progression, and AI:

| Principle | Meaning |
|-----------|---------|
| **Player responsiveness is sacred** | Player-issued commands resolve immediately; players can interrupt their own units' actions, not enemy-imposed control |
| **Predictability over smart AI** | Transparent rules beat opaque target weighting; players should understand why units act |
| **Veterans are legendary** | Long-lived units are extremely valuable; design favors preservation over disposable armies |
| **Time is the balancer** | Very high mastery is theoretically attainable but increasingly time-consuming (RuneScape-style, not Kenshi grind-penalty) |
| **Simulation creates decisions, not chores** | Automation reduces busywork; strategy and attachment remain |
| **Physical believability** | Positioning, collision, weapon reach, and facing matter more than hidden combat slots |
| **Emergent stories** | Systems should reinforce memorable narratives rather than abstract mechanic stacks |

**Genre touchstones:**

- **World simulation:** Kenshi-inspired large-world survival, logistics, and attachment
- **Combat feel:** Warcraft III tactical engagements — not StarCraft II lethality, not Kenshi limb simulation
- **Progression:** RuneScape-style use-based skills — not traditional character levels

---

# Combat

See [ADR-069](ADRs/ADR-069-combat-design-philosophy.md) for the accepted design direction.

## Combat philosophy

Combat is designed around **Warcraft III-style tactical engagements**:

- Units survive long enough for meaningful repositioning and micro
- Veterans are extremely valuable and should be preserved
- Positioning matters more than APM
- The player should always feel in control of their own units
- Combat should be predictable and understandable

**Not:** StarCraft II time-to-kill, Kenshi-style permanent injury simulation (see Injuries).

## Combat responsiveness

**Rule:** Players may interrupt actions performed by their own units, but cannot interrupt
actions imposed by enemies.

| Player **can** cancel | Player **cannot** cancel |
|----------------------|--------------------------|
| Attack windup | Stagger |
| Movement | Knockdown |
| Current target | Enemy crowd control |
| Retreat immediately | |

This is a **core combat philosophy**, not a polish detail. Current implementation supports
order cancellation and attack-cycle clearing (ADR-056); stagger, knockdown, and enemy CC are
deferred.

## Collision and positioning

- Physical unit collision (no abstract combat slot system)
- Collision radius **smaller than Warcraft III** — tighter formations, more precise choke play
- Natural front lines emerge from collision and reach
- Chokepoints matter
- **If a unit can physically reach a target, it may attack** — no hidden slot reservation

Current foundation: collision radii on `UnitDefinition`, doodad obstacles (ADR-031), grid
navigation (ADR-032). Unit-unit collision blocking is a future enhancement.

## Weapon combat envelope

Weapons have **minimum and maximum effective ranges** (design target; current catalog uses
a single `range_meters` — see ADR-054, ADR-057).

| Weapon (example) | Envelope (design units) |
|------------------|-------------------------|
| Axe | 0–100 |
| Spear | 100–200 |
| Bow | 200–1000 |

**Auto-reposition rules:**

- Units reposition automatically only when an opponent leaves the weapon envelope
- Enemy AI may proactively maintain preferred spacing within the envelope
- **Player-controlled units prioritize responsiveness** over automatic repositioning

## Target selection

Priority tiers (highest first):

1. Active combatants
2. Enemy combatants not currently fighting
3. Non-combatants (pack animals, workers, etc.)

Within a tier: **attack closest target**. No hidden target weighting.

Current C9 AI uses closest-valid + `UnitId` tie-break (ADR-062) — tiered priority is deferred.

## Attack Move

Design behavior:

1. Move toward destination
2. Engage nearest valid target along the route
3. Pursue fleeing targets
4. Switch targets if another valid target becomes closer
5. Resume movement toward destination after combat

**Direct attack orders override** attack-move behavior.

Foundation: `CombatState::AttackMoving` and scan radius (ADR-056, ADR-057). Full pursue /
resume semantics are not complete.

## Facing

- Facing matters for defense — units primarily defend against their current opponent
- Being surrounded should become dangerous **naturally** (multiple attackers, reach, collision)
  without artificial "surrounded" damage bonuses
- Not yet implemented in simulation

## Weapon collision (strike origin)

Attacks originate from the **weapon**, not the character center:

- Sword blade, spear tip, tail, horn, fists, etc.

Strike validation and damage application should eventually use weapon attach points / hit
volumes. Current implementation uses center-to-center edge distance (ADR-057).

## Attack animation phases

General cycle (aligns with ADR-058 timing model):

| Phase | Damage |
|-------|--------|
| Windup | No |
| Contact window | Yes — damage only here |
| Recovery | No |

Heavy attacks may add **stagger** (hammers, large creatures, heavy weapons). Light weapons
generally do not. Stagger must never enable permanent stun-lock.

## Downed state (future)

Design replaces instant death as the default outcome:

- Units are normally **downed** instead of instantly killed
- Downed units: lootable immediately, treatable, may survive
- Death requires additional circumstances (execution, bleed-out, abandonment, etc.)

**Current implementation:** 0 HP → `UnitState::Dead` → removal same tick (ADR-059). Downed
state is a deliberate future replacement, not current behavior.

## Injuries (under design)

Goals:

- Meaningful consequences without copying Kenshi's limb system
- Avoid permanent frustrating debuffs
- Encourage recovery, treatment, and long-term attachment to units

## Randomness

Combat includes **controlled randomness** (deferred in current flat-damage implementation):

- Misses, evasion, critical hits, damage ranges
- Warcraft III-style damage ranges under consideration
- Randomness must remain understandable — not opaque RNG stacks

---

# Progression and Attributes

See [ADR-070](ADRs/ADR-070-progression-and-attributes.md).

## Progression model

**Decision:** Use-based progression. No traditional global character level.

- Skills improve through performing related activities
- No overall learning penalty — skills do not slow each other
- A legendary warrior can eventually master craftsmanship given enough time
- Very high skills become extremely time-consuming but remain theoretically attainable
- "Demigod" characters are acceptable — **time** is the limiting resource

The workbook `Level` column on the Units sheet is **authoring metadata today**, not
authoritative runtime progression. Future runtime skills replace global level as the
progression truth.

## Attributes (draft)

Attributes are **inputs** to formulas — not directly exposed raw numbers to players in
combat tooltips.

| Attribute | Planned influence |
|-----------|-------------------|
| **STR** | Melee damage, carry weight, block power |
| **DEX** | Reload speed, crit chance |
| **CON** | Health, regeneration |
| **PER** | Accuracy, enemy spotting distance; likely crit damage / hit quality |
| **AGI** | Move speed, attack speed |
| **CHR** | Shop prices; future leadership / social systems |
| **INT** | Research speed; likely expanded later |

Catalog stats on `UnitDefinition` are imported and preserved (ADR-027); combat formulas do
not consume them yet (ADR-058).

## Critical hits (draft direction)

Crit chance primarily from:

- Weapon base crit
- Dexterity
- Weapon skill (use-based)

Example concept (not final):

```text
crit_chance = (weapon_base + 15 * log(DEX)) * (weapon_skill / 100)
```

Perception likely influences critical **damage** or weak-point exploitation rather than
base crit rate.

---

# Creature AI

See [ADR-071](ADRs/ADR-071-creature-ai-architecture.md).

## Architecture (proposed)

```text
Species Template
      ↓
Behavior Template
      ↓
Personality
      ↓
Current State
      ↓
Decision
```

**Current implementation:** scan → validate → `issue_unit_order(Attack)` only (ADR-062).
Full template stack is future work.

## Behavior templates (examples)

| Template | Tactics |
|----------|---------|
| Swarm | Stay together, regroup, focus shared targets |
| Ambusher | Wait for prey, attack from concealment |
| Skirmisher | Maintain distance, retreat when pressed |
| Grazer | Herd behavior, flee when threatened |
| Pack Hunter | Coordinate with nearby allies |

Templates define **tactics**, not species identity alone.

## Personality values (bias, not scripts)

Possible axes: Aggression, Bravery, Curiosity, Territoriality, Sociality, Persistence,
Protectiveness. These **bias** decisions rather than directly selecting actions.

## Dynamic state (temporary modifiers)

Not personality. Examples: Hunger, Injured, Alert, Tired, Recently attacked. These
temporarily modify behavior until conditions clear.

## Confidence (possible future value)

Derived from nearby allies, enemy strength, current health, species, prior experiences.
Used to decide attack vs retreat. Distinct from personality (slow-changing) and dynamic
state (fast-changing).

---

# Inventory and Equipment

See [ADR-073](ADRs/ADR-073-inventory-and-equipment.md).

## Decision

**Kenshi-style grid inventory** for carried items. **Traditional equipment slots** for
worn/wielded gear.

Rationale:

- Physical item size matters for logistics
- Fits survival / caravan focus
- Equipment slots keep combat loadouts readable

## Quality-of-life (planned)

- Auto-sort, smart placement, automatic stack merging
- AI-friendly inventory organization
- Minimize busywork; preserve strategic packing decisions

`active_weapon_id` on `UnitRecord` is the first equipment seam (ADR-054).

---

# Settlement Automation

See [ADR-072](ADRs/ADR-072-settlement-automation-and-production.md).

## Philosophy

Assign **professions** rather than micromanaging individual tasks.

Example worker profile:

| Role | Assignment |
|------|------------|
| Primary | Farmer |
| Secondary | Hauler |
| Emergency | Defender |

## Jobs vs tasks

| Concept | Persistence | Examples |
|---------|-------------|----------|
| **Jobs** | Persistent professions | Farmer, Builder, Hunter, Smith |
| **Tasks** | Temporary work units | Harvest Wheat, Bake Bread, Repair Wall, Haul Stone |

Buildings **generate tasks**. Workers perform tasks based on profession and priority.

## Production model

Buildings **request** production inputs/outputs (Factorio-style logistics with individual
workers, not abstract city-wide pools).

Example — Bakery requests flour; produces bread. Workers with matching professions and
priorities satisfy requests automatically.

## Worker priorities

Ordered list (example): Farming → Construction → Hauling → Medicine.

When primary work is absent, workers naturally fall through to secondary priorities.
**Direct player orders temporarily override** automation.

---

# World and Food

## Staple crops (design)

| Crop | Notes |
|------|-------|
| Brim Grain | Staple grain |
| Knot Tubers | Root crop |
| Glass Pods | Prispods remain an existing crop concept |
| Thread Moss | |
| Ember Bulbs | |

## Prepared foods (design)

Brim Bread, Stone Stew, Traveler's Cakes, Smiler Cheese, Hunter's Skewers.

## Philosophy

Alien biology with a **recognizable food economy** — players should intuit supply chains
even when ingredients are exotic.

Resource nodes and doodad `ResourceNode` kinds are architectural seams (ARCHITECTURE.md);
crop simulation and recipes are not implemented.

---

# Document Maintenance

When implementing a design item:

1. Update the relevant ADR from "design direction" to "accepted decision" with scope
2. Update ROADMAP.md phase status
3. Trim or mark implemented sections here to avoid stale duplication

When rejecting a design item, note the rejection in the ADR and remove or revise here.
