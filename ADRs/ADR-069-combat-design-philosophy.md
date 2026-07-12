# ADR-069: Combat Design Philosophy

## Status

Accepted (design direction — partial implementation)

## Context

Combat foundations (C1–C9) implement weapon data, orders, range, strikes, projectiles,
death, and basic auto-acquisition. The **intended player experience** was not previously
documented in ADRs. This ADR records combat design goals so future implementation does not
drift toward StarCraft II lethality or Kenshi simulation by accident.

Full design narrative: [DESIGN.md](../DESIGN.md#combat).

## Decision

### Genre target

**Warcraft III-style tactical combat:**

- Meaningful time-to-kill for repositioning and micro
- Veterans are valuable; preservation over disposable armies
- Positioning > APM
- Predictable, understandable rules

**Explicitly not:**

- StarCraft II burst lethality as the baseline
- Kenshi-style limb/injury simulation (see ADR-070 / DESIGN.md Injuries)

### Player responsiveness (core rule)

Players **may** interrupt their own units' actions. Players **may not** interrupt
enemy-imposed effects.

| Interruptible (own units) | Not interruptible (enemy-imposed) |
|---------------------------|-----------------------------------|
| Attack windup | Stagger |
| Movement | Knockdown |
| Current target / retreat | Enemy crowd control |

**Implemented today:** order cancellation clears attack cycles and combat state (ADR-056).
Stagger, knockdown, and CC are deferred.

### Collision and reach

- Physical unit collision; **no combat slot system**
- Collision radius target: smaller than Warcraft III
- Chokepoints and front lines emerge from collision + weapon reach
- If a unit can physically reach a target, it may attack

**Implemented today:** per-unit `collision_radius_meters`, doodad blocking (ADR-031),
edge-to-edge weapon range (ADR-057). Unit-unit collision and min range envelopes are deferred.

### Weapon envelope

Weapons have **min and max effective range** (design). Units auto-reposition when opponents
leave the envelope. Enemy AI may maintain preferred spacing; **player units prioritize
command responsiveness** over automatic repositioning.

**Implemented today:** single `range_meters` max; chase on leave-range with hysteresis
(ADR-057). Min range and player-vs-AI reposition policy are deferred.

### Target selection

Tier priority (design):

1. Active combatants
2. Idle enemy combatants
3. Non-combatants

Within tier: closest target; **no hidden weighting**.

**Implemented today:** closest valid + `UnitId` tie-break (ADR-062).

### Attack Move

Move → engage nearest valid → pursue → switch if closer target appears → resume movement.
Direct `Attack` overrides attack-move.

**Implemented today:** destination + optional acquired target + scan (ADR-056, ADR-057).
Full pursue/resume semantics incomplete.

### Facing and weapon origin

- Facing affects defense; surround danger from geometry, not flat bonuses
- Damage originates from weapon hit volume, not character center

**Deferred** — center-to-center range used today (ADR-057).

### Strike phases and stagger

Windup → **contact window (damage only)** → recovery. Heavy weapons may stagger; no
permanent stun-lock.

**Implemented today:** windup/strike/recovery/cooldown (ADR-058). Contact window equals
instant strike transition. Stagger not implemented.

### Downed vs death

**Design:** units downed by default; death requires additional circumstances; downed units
lootable/treatable.

**Implemented today:** 0 HP → dead → same-tick removal (ADR-059). **Presentation:** death animation plays after world removal without delaying simulation
(ADR-074 A3). Locomotion + attack layering is presentation-only (ADR-075 A4). Turn-in-place
and heading-aware locomotion polish are presentation-only (ADR-076 A5) — simulation facing
remains deferred per roadmap. Downed state replaces instant removal in a future ADR when implemented.

### Randomness

Controlled misses, evasion, crits, and damage ranges (WC3-style ranges considered).

**Implemented today:** flat catalog damage, no randomness (ADR-058).

## Consequences

- Future combat ADRs must cite this document when changing TTK, CC, or death models
- Player command pipeline (ADR-038, ADR-041) remains the authority for responsiveness
- AI additions (ADR-071) must respect predictability over opaque scoring

## References

- [DESIGN.md](../DESIGN.md)
- ADR-054, ADR-056, ADR-057, ADR-058, ADR-059, ADR-062
- ADR-038 (intent responsiveness), ADR-041 (commands)
