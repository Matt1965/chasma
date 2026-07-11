# ADR-070: Progression and Attributes

## Status

Accepted (design direction — not implemented in simulation)

## Context

The Units workbook sheet imports Strength, Dexterity, Constitution, Agility, Charisma,
Intelligence, and a `Level` column (ADR-027). Combat (ADR-058) does not yet consume
attributes. Progression philosophy was undocumented.

Full design narrative: [DESIGN.md](../DESIGN.md#progression-and-attributes).

## Decision

### Use-based progression

- **No traditional global character level** as runtime progression truth
- Skills improve by performing related activities (RuneScape-style)
- No global learning penalty; skills do not slow each other
- Extreme mastery is time-gated, not hard-capped — demigod characters are acceptable
- Workbook `Level` is **authoring metadata** until a skills system supersedes it

### Attribute roles (draft)

Attributes are formula **inputs**, not necessarily shown as raw numbers to players.

| Attribute | Planned role |
|-----------|--------------|
| STR | Melee damage, carry weight, block power |
| DEX | Reload speed, crit chance |
| CON | Health, regeneration |
| PER | Accuracy, spotting range; likely crit damage / hit quality |
| AGI | Move speed, attack speed |
| CHR | Shop prices; future leadership / social |
| INT | Research speed; expanded later |

`UnitDefinition` preserves imported stats; runtime skill levels live on instance/simulation
state when implemented (data-first, ADR-027).

### Critical hits (draft)

Crit chance from weapon base + DEX + weapon skill. Example concept:

```text
(weapon_base + 15 * log(DEX)) * (weapon_skill / 100)
```

Perception biases crit damage or weak-point hits, not necessarily base crit rate.

### Injuries (under design)

Meaningful consequences without Kenshi limb simulation; avoid permanent frustrating
debuffs; encourage treatment and attachment. No decision until downed-state combat (ADR-069)
exists.

## Non-goals (current phase)

- Skill XP curves, UI, respec, or per-activity tracking
- Removing workbook columns before skills system exists
- Stat scaling on weapons (`stat_scaling` field remains reserved, ADR-054)

## Consequences

- ADR-058 flat damage remains until attributes and skills ADR implementation
- New progression features require instance state on `WorldData`, not ECS-only
- Excel import continues preserving all stat columns

## References

- [DESIGN.md](../DESIGN.md)
- ADR-027, ADR-054, ADR-058, ADR-069
