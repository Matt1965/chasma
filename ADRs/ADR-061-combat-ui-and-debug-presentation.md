# ADR-061: Combat UI and Debug Presentation

## Status

Accepted

## Context

C5–C7 established authoritative combat simulation: engagement, strikes, projectiles,
and death handling on [`WorldData`]. C8 requires making that state readable without
changing combat rules. Players need HUD and command feedback; developers need overlays,
trace logs, and inspector fields.

Prior UI layers (ADR-040, ADR-048) already treat ECS and HUD as presentation. Combat
must follow the same boundary: simulation truth stays on [`UnitRecord`], [`ProjectileRecord`],
[`WeaponCatalog`], and [`UnitCatalog`].

## Decision

### Read-only presentation model

All C8 systems **read** combat state and **emit intents** through the existing client
command pipeline. No HUD, overlay, or inspector code may mutate [`WorldData`] combat
fields directly.

Presentation helpers live in:

- `src/ui/gameplay/combat_display.rs` — HUD formatting
- `src/debug/overlay/combat_overlay.rs` — gizmo overlays
- `src/debug/combat_log.rs` — trace formatting
- `src/dev/inspector/` — dev snapshot extensions

### HUD and command panel combat fields

The selected-unit panel shows vitals, weapon stats (from [`WeaponCatalog`]), combat
state, target, and attack phase. Multi-selection shows count, average HP %, and a
primary-unit weapon summary.

Command panel buttons (Attack, Attack Move, Stop, Hold) arm [`CommandType`] values or
emit palette intents only. Enabled state reflects selection + controllability policy,
not direct combat mutation.

### Overlay scoping rules

Combat debug overlays are gated by [`DebugOverlayConfig::combat`] (default off). When
enabled, overlays draw for **selected units only**, capped by `max_draw_units`:

- Edge-to-edge weapon range circles
- Target lines from [`UnitRecord::combat_state`]
- Projectile lines/markers from [`ProjectileRecord`] when source is selected
- Optional hit/dead markers from recent combat trace entries affecting selection

Overlays never iterate all world units by default.

### Combat trace / log model

[`CommandTraceBuffer`] records combat outcomes including `AttackOrderAccepted`,
`AttackOrderRejected`, `AttackEnteredRange`, strike/projectile/death events. Dev
inspector surfaces recent lines via [`recent_combat_log_lines`], filtered by inspected
unit when applicable. Trace order matches simulation flush order (deterministic).

### Projectile debug presentation

Projectiles are excluded from gameplay selection ([`SelectedUnits`]). Dev inspector
lists in-flight projectiles sourced from the inspected unit and supports standalone
[`ProjectileInspectorSnapshot`] capture from [`ProjectileRecord`] only. ECS projectile
entities remain render mirrors (ADR-060).

### Cursor feedback

When Attack is armed, cursor mode switches to Attack only on hover targets that pass
[`is_valid_attack_target`]. Friendly/neutral hovers do not show attack cursor unless
targeting rules allow it.

### Why UI must not own combat truth

Duplicating HP, targets, or weapon stats in ECS or UI resources would desync from
simulation, break replay/multiplayer seams, and invite accidental rule changes in
presentation code. UI reads catalogs and records each frame; simulation remains sole
writer.

## Consequences

- Weapon stat display tracks catalog changes automatically.
- Combat overlays are cheap when disabled and bounded when enabled.
- Future combat features extend trace outcomes and snapshot fields without HUD owning
  new rules.

## Non-goals (C8)

No new damage types, armor UI, inventory, combat AI, animation polish, or simulation
changes.
