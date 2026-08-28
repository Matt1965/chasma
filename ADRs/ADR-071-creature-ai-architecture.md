# ADR-071: Creature AI Architecture

## Status

Accepted (design direction — minimal scan AI implemented)

## Context

ADR-062 implemented closest-target auto-acquisition for eligible units. The broader creature
AI model — species templates, behavior templates, personality, and dynamic state — was not
documented.

Full design narrative: [DESIGN.md](../DESIGN.md#creature-ai).

## Decision

### Layered decision model (target)

```text
Species Template → Behavior Template → Personality → Current State → Decision
```

| Layer | Role |
|-------|------|
| **Species template** | Baseline capabilities, senses, body, default weapons |
| **Behavior template** | Tactical doctrine (swarm, ambusher, skirmisher, grazer, pack hunter) |
| **Personality** | Slow-changing biases (aggression, bravery, curiosity, territoriality, sociality, persistence, protectiveness) |
| **Current state** | Fast-changing modifiers (hunger, injured, alert, tired, recently attacked) |
| **Decision** | Concrete action selection respecting ADR-069 predictability rules |

**Personality biases; it does not script actions.** **Dynamic state is not personality.**

### Behavior templates (examples)

- **Swarm** — cohesion, regroup, shared focus fire
- **Ambusher** — wait, concealment, strike when prey enters range
- **Skirmisher** — maintain distance, retreat when pressed
- **Grazer** — herd, flee when threatened
- **Pack hunter** — ally coordination, flanking opportunities

### Relationship as decision input (2026-08-24, accepted direction — not implemented)

[ADR-132](ADR-132-relationship-and-reputation-architecture.md) supplies the social input this layered
model previously left unspecified:

```text
perceive candidate
  -> resolve A -> B relationship
  -> behavior / personality / state interprets it
  -> choose intent
  -> existing UnitOrder API
```

Relationship is an **input** to the interpretation step, not a behavior system. It answers *what
does A think of B*, never *should A attack B*. Relationship code must never write `CombatState`,
set `attack_cycle`, or touch `reactive_combat_target`; `UnitOrder` remains the action boundary.

Different behavior templates may interpret the same relationship value differently — a territorial
predator attacks, a grazer flees, a merchant refuses service. The first primitive implementation
lets the placeholder AI interpret a sufficiently negative relationship as Attack. That threshold is
**tuning**, and `negative relationship == attack` must not become a permanent equation.

### Confidence (optional future layer)

Derived from allies nearby, enemy strength, health, species, experience. Informs attack vs
retreat without replacing tiered target rules (ADR-069).

### Relationship to ADR-062

Current `step_combat_ai_acquisition` is **not** this architecture — it is a deterministic
placeholder:

- Round-robin scan, closest valid target, `UnitId` tie-break
- Issues `UnitOrder::Attack` only; no behavior trees or personality

Future work replaces or wraps acquisition with template-driven decisions while keeping
**UnitOrder API** as the action boundary (ADR-062).

### Design constraints (from ADR-069)

- Predictability over hidden target weighting
- Closest-in-tier targeting unless a behavior template explicitly documents an exception
- Player units: optional auto-acquire seam exists (`CombatAiSettings`); default off

## Non-goals (until settlement/wildlife phases)

- Economy AI, job selection, inventory management AI
- Squad-level coordinated maneuvers beyond pack-hunter template
- Morale rout animations and audio

## Consequences

- New AI modules should extend `src/world/combat/ai/` or a sibling `src/world/ai/` without
  writing `CombatState` directly
- Species/behavior data belongs in catalogs, not hard-coded per unit type
- ADR-062 remains valid for current implementation scope

## References

- [DESIGN.md](../DESIGN.md)
- ADR-062, ADR-069, ADR-056
- ADR-132 (relationship architecture — supplies the social decision input)
