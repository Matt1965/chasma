# ADR-056: Combat Targeting and Attack Orders (C3)

# Status

Accepted (C3 — targeting and order state only)

# Context

ADR-054 established weapon catalog data. ADR-055 added `UnitVitals` and
`CombatState` on `UnitRecord`. C3 adds attack-related **orders and combat
state** without damage, range checks, chase, death, or AI target acquisition.

# Decision

## Scope (C3)

| In scope | Out of scope (deferred) |
|----------|-------------------------|
| `UnitOrder::Attack`, `UnitOrder::AttackMove` | Damage application |
| `CombatState::Attacking`, `CombatState::AttackMoving` | Range validation (C4) |
| Target validity (`is_valid_attack_target`) | Chase / attack-move scanning |
| Ownership + weapon filter rules | Death / removal |
| Command + interaction routing | Projectiles, animations, combat AI |

## State ownership

Locomotion remains on `UnitState` (`Idle`, `Moving`). Combat intent lives on
`CombatState` (separate from locomotion):

- `Attacking { target: UnitId }` — direct attack order
- `AttackMoving { destination, target: Option<UnitId> }` — destination stored;
  `target` reserved for future scan (C4+)

`WorldData` is authoritative. ECS entities are not combat truth.

## Orders (`issue_unit_order`)

- `Attack` — validates target, sets `CombatState::Attacking`; no movement
- `AttackMove` — sets `CombatState::AttackMoving`; no movement or scan yet
- `MoveTo` — unchanged (command buffer)
- `Idle` — clears combat state to `Peaceful`

Signature includes `WeaponCatalog` and `AttackTargetingPolicy`.

## Target validity (`src/world/combat/targeting.rs`)

`validate_attack_target` / `is_valid_attack_target` enforce:

1. Attacker and target exist
2. Attacker ≠ target
3. Both alive (`current_hp > 0`)
4. Attacker has enabled default weapon
5. Runtime ownership hostility (not catalog `faction_tag`)
6. Weapon `target_filters`

**No range check in C3.**

### Ownership rules (runtime)

| Attacker affiliation | May attack |
|---------------------|------------|
| Player | Hostile, Wildlife |
| Hostile | Player |
| Same `team_id` | Never |
| Neutral | Only if weapon filter allows `Neutral` or dev override |
| Dev | All (when `dev_allow_all_targets`) |

### Weapon filters

Respect `WeaponDefinition.target_filters`: `Enemies`, `Wildlife`, `Neutral`,
`Structures`, `All`. `Enemies` requires ownership hostility pass.

Typed errors: `AttackerNotFound`, `TargetNotFound`, `SelfTarget`,
`AttackerDead`, `TargetDead`, `MissingWeapon`, `InvalidOwnershipTarget`,
`WeaponCannotTarget`.

## Command pipeline

- Right-click terrain → `CommandType::Move`
- Right-click valid hostile unit → `CommandType::Attack`
- Palette `AttackMove` → `CombatState::AttackMoving` (intent only)

`BuiltCommandPlan` adds `Attack` and `AttackMove`. Dispatcher issues via
`issue_attack_orders_to_selection` / `issue_attack_move_orders_to_selection`.

## Interaction classification

`InteractionType` extensions:

- `AttackableUnit` — valid attack target
- `FriendlyUnit` — same team / non-hostile
- `NeutralUnit` — neutral affiliation

Unit clicks use `classify_unit_target` (not terrain query).

## Trace

Command trace records attack order accept/reject per unit with `UnitOrderError`
when rejected.

## Attack-cycle lifetime (REVIEW-A2)

An [`AttackCycle`] is valid only while **all** hold:

- attacker is alive
- target exists and is alive
- `CombatState` still references the same target
- target remains valid under ownership / weapon filters
- unit is in an attack-capable engagement state (`Attacking`, `Chasing`, or
  `AttackMoving` with `target: Some`)

If any condition fails, clear `attack_cycle` immediately; no strike may occur
from that cycle.

### Retargeting (`UnitOrder::Attack`)

When a new `Attack { target }` is accepted and `target` differs from the
current cycle target:

- clear the existing `attack_cycle` (no preserved windup / recovery progress)
- set combat state to the new target via `initial_attack_combat_state`
- emit `AttackCycleResetRetarget` when trace is available

Re-issuing `Attack` against the **same** target does **not** reset cycle
progress.

### Order cancellation

Orders that cancel or replace engagement clear `attack_cycle` and stale combat
targets:

| Order | Combat state | Cycle |
|-------|--------------|-------|
| `Idle` | `Peaceful` | cleared |
| `MoveTo` | `Peaceful` (on issue) | cleared |
| `AttackMove` | `AttackMoving { target: None }` | cleared |
| `Attack` (new target) | updated | cleared when target differs |

Emit `AttackCycleClearedOrderCancelled` when trace is available.

### Invalid engagement

When engagement detects target removed, dead, non-hostile, weapon missing, or
filter-invalid:

- clear `attack_cycle`
- transition to `Peaceful` for direct `Attack`, or resume stored `AttackMove`
  destination (`target: None`) when applicable
- emit `AttackCycleClearedInvalidTarget`

## Future

- **C4:** range validation, chase, attack-move target acquisition
- **C5:** damage timing, cooldowns, death

## Design direction (not yet fully implemented)

See [ADR-069](ADR-069-combat-design-philosophy.md) and [DESIGN.md](../DESIGN.md#combat).

- **Attack Move:** pursue fleeing targets, switch on closer valid target, resume movement
  after combat; direct `Attack` overrides
- **Target tiers:** active combatants → idle enemies → non-combatants; closest within tier
- **Responsiveness:** player can cancel own windup/movement/target; cannot cancel enemy CC
- **Retargeting** (implemented): new `Attack` target clears attack cycle (REVIEW-A2)

# Consequences

- `issue_unit_order` signature change propagates to dispatch and tests
- `CombatState` is no longer `Copy` (contains `WorldPosition`)
- Interaction query requires `WeaponCatalog` for unit-under-cursor classification
