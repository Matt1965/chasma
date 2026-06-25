# ADR-055: Unit Vitals and Combat Runtime State (C2)

# Status

Accepted (C2 — combat state foundation only)

# Context

ADR-054 established weapon-centric catalog data. C2 adds authoritative **runtime**
combat state on unit instances without targeting, damage resolution, death, or
attack orders.

# Decision

## Ownership

| Concern | Owner |
|---------|-------|
| Design-time max HP, body stats | `UnitDefinition.max_hp` (Excel `Max HP`) |
| Runtime current/max HP | `UnitRecord.vitals: UnitVitals` |
| Combat posture | `UnitRecord.combat_state: CombatState` |
| HP mutation API | `WorldData::{get_unit_vitals, set_unit_hp, damage_unit, heal_unit}` |
| Rendering | ECS — no vitals components |
| HUD / inspector | Read-only consumers of `WorldData` |

`base_hp` remains a separate design stat from Excel `Base HP`. `max_hp` is the
value copied to instances at spawn.

## Types (`src/world/unit/`)

- `UnitVitals { current_hp, max_hp }`
- `CombatState` — `Peaceful` (default), `Alert`, `Engaged` (placeholders)

Reserved on `UnitDefinition` with no C2 behavior:

- `stamina_max: Option<u32>`
- `energy_max: Option<u32>`

## Spawn

`create_unit` / `create_unit_with_ownership` initialize:

```text
vitals.current_hp = definition.max_hp
vitals.max_hp     = definition.max_hp
combat_state      = Peaceful
```

## WorldData HP rules

- `set_unit_hp` clamps to `[0, max_hp]`
- `damage_unit` uses `saturating_sub` — no underflow, no death side effects
- `heal_unit` cannot exceed `max_hp`
- No entity removal on zero HP (deferred)

## Excel

`Units` sheet requires `Max HP` column (`> 0`). Imported via existing unit catalog
pipeline.

## UI / dev exposure

Player HUD (`selected_unit_panel`) and dev inspector show `current/max HP` and
combat state label. No combat UI widgets in C2.

# Why death is deferred

Death implies removal policy, corpses, loot, AI reactions, and command cleanup.
C2 only stores HP so later phases can apply damage and death consistently.

# Future

- C3+: attack orders, targeting
- C4+: edge-to-edge range, damage pipeline
- Death/removal, stamina/energy consumption, `UnitRecord.active_weapon_id`

# Non-goals (C2)

No targeting, attack orders, attack-move, damage resolution, death, corpses,
projectiles, combat AI, movement changes, or combat UI.

# References

- ADR-054 Weapon-Centric Combat Data Model
- ADR-027 Unit Data Ownership
- ADR-050 Player HUD Foundation
- Implementation: `src/world/unit/vitals.rs`, `combat_state.rs`, `WorldData` HP APIs
