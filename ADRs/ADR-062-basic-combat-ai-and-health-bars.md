# ADR-062: Basic Combat AI and Health Bars

## Status

Accepted

## Context

C3–C8 established combat simulation, player commands, UI, and debug presentation.
Hostile units could not fight back without player-issued orders. Unit HP was only
visible in the selected-unit HUD panel.

C9 adds minimal combat auto-acquisition for non-player units and overhead health
bars as read-only presentation.

## Decision

### Auto-acquisition is not full AI

`src/world/combat/ai/` implements **scan → validate → issue order** only. No behavior
trees, squad tactics, kiting, morale, or threat scoring.

### Combat AI uses UnitOrder API

Eligible units call `issue_unit_order(UnitOrder::Attack { target })`. AI never writes
[`CombatState`] or vitals directly. Orders flow through the existing C3–C8 pipeline.

### Deterministic scan and target priority

[`CombatAiSettings`] controls enable flag, scan radius, interval, per-tick budget,
and optional player auto-acquire (default off).

Scan uses round-robin cursor over `sorted_unit_ids()` with accumulated interval
timing. Target selection: closest valid target, then lowest [`UnitId`] tie-break.
Validation uses [`is_valid_attack_target`] (C3 ownership + weapon rules).

### Ownership-based hostility

Eligibility uses runtime [`Affiliation`] / [`is_player_controllable`], not catalog
`faction_tag`. Hostility matrix remains in C3 targeting.

### Scan budget model

`max_units_scanned_per_tick` caps work per interval window. `scan_interval_seconds`
prevents full-world scans every simulation tick.

Combat AI acquisition runs in stage 6 of [`run_simulation_tick`]
([ADR-065](ADR-065-authoritative-simulation-tick-orchestrator.md)), after death cleanup
and before movement.

### Attack-move interaction

Units in [`CombatState::AttackMoving`] without a valid target remain eligible for
AI acquisition. Attack-move resume after target death is handled by existing C4
engagement (`invalid_target_state` restores attack-move). No attack-move redesign.

### Health bars as read-only presentation

`src/units/health_bars/` reads [`UnitRecord::vitals`] from [`WorldData`] each frame.
ECS bar entities are disposable children of unit render entities.

Visibility (default):

- Selected units
- Damaged units (`current_hp < max_hp`)
- Hovered unit
- All units when dev **Health bars (all)** debug toggle is on

Full-health unselected units are hidden by default.

### Trace events

Combat AI emits `AiTargetAcquired`, `AiScanNoTarget` via [`CommandTraceBuffer`].
Health bar show/hide traces emit only when dev health debug is enabled.

## Future hooks

Advanced AI, aggro memory, threat scoring, squad tactics, and patrol behavior remain
out of scope. [`CombatAiSettings::player_units_auto_acquire`] is the seam for
optional player auto-targeting.

Full creature AI architecture (species → behavior template → personality → state →
decision) is documented in [ADR-071](ADR-071-creature-ai-architecture.md). C9
implementation is a deterministic placeholder only.

**Design alignment ([ADR-069](ADR-069-combat-design-philosophy.md)):** future AI must use
tiered target priority (active combatants first) and closest-in-tier selection — no hidden
weighting.

## Non-goals (C9)

No behavior trees, kiting, retreats, abilities, armor, economy AI, sound/VFX, or
minimap combat markers.
