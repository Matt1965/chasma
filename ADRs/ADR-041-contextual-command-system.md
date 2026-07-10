# ADR-041: Contextual Command System (U-UI5)

# Status

Accepted (U-UI5 — RTS command layer expansion; REVIEW-B3 placeholder cleanup)

# Context

U-UI2 established the intent pipeline (`ClientIntent` → dispatch → command APIs).
U-UI3 added trace/debug observability. U-UI4 added gameplay UI that **reflects**
client state without owning command logic.

Right-click previously always emitted `MoveCommand`. SC2-style RTS control requires
contextual command semantics (move, stop, hold, future attack/interact) while
keeping simulation rules unchanged.

REVIEW-B3 audited the command layer and removed placeholder behavior that implied
functionality that did not exist (e.g. Hold routing to Stop, Interact aliasing Move).

# Decision

## Pipeline extension

```text
Input → ClientIntent::ContextualCommand { target }
     → resolve_contextual_command (classification only)
     → command_availability (validation)
     → build_command_plan
     → issue_*_orders_to_selection
     → issue_unit_order → WorldData
     → ResolvedCommandFeedback (UI hook)
```

Legacy `ClientIntent::MoveCommand` remains and routes through the same path.

Palette commands (`ClientIntent::PaletteCommand`) use the same pipeline; only
Stop is immediate-dispatch from the HUD — Move, Attack, and Attack Move arm for
right-click targeting.

## New module: `src/client/commands/`

| Module | Responsibility |
|--------|----------------|
| `command_types.rs` | `CommandType`, `CommandTarget`, `ContextualCommandIntent` |
| `command_availability.rs` | Availability rules and structured unavailability reasons (REVIEW-B3) |
| `context_resolver.rs` | Terrain/unit click → contextual intent (no gameplay logic) |
| `command_builder.rs` | Contextual intent → `BuiltCommandPlan` |
| `command_palette.rs` | Available commands per selection (SC2 palette foundation) |

## Implemented command set (REVIEW-B3)

| Type | Status | Player exposure |
|------|--------|-----------------|
| Move | Implemented | Enabled; arms for right-click |
| Stop | Implemented | Enabled; immediate palette dispatch |
| Attack | Implemented | Enabled; arms for right-click on valid target |
| AttackMove | Implemented | Enabled; arms for right-click on terrain |
| HoldPosition | Reserved | Visible, **disabled** — `FeatureNotImplemented` |
| Interact | Reserved | Visible, **disabled** — `FeatureNotImplemented` |

Commands not in this table (Patrol, Gather, Build, Repair, Guard, Follow,
Ability) are not exposed.

## Placeholder removal philosophy (REVIEW-B3)

1. **No pretend implementations** — removed `HoldAll` plan variant and
   Hold→Idle aliasing.
2. **No silent success** — unimplemented commands return
   `CommandBuildError::FeatureUnavailable` and dispatch returns
   `IntentDispatchStatus::Rejected`.
3. **Explicit UI state** — disabled buttons include reason in tooltip via
   `command_tooltip`.
4. **Reserved variants stay internal** — `HoldPosition` and `Interact` remain in
   `CommandType` for future work but are not routable to simulation.

## Context resolution rules

- Right-click terrain → `CommandType::Move` (or armed command)
- Right-click hostile unit → `CommandType::Attack` when valid
- Right-click friendly unit → `CommandType::Move`
- Empty selection → intent ignored
- Invalid target → no intent emitted (input collection)
- Armed Attack / AttackMove override default terrain classification

## UI availability rules

- [`CommandPaletteEntry`](../src/client/commands/command_palette.rs) carries
  `CommandAvailability` instead of a bare `enabled` flag.
- HUD command panel reads availability; it never writes to `WorldData`.
- Only implemented commands may be clicked to arm or dispatch.

## Boundaries (unchanged)

- No changes to pathfinding, steering, formation, or movement simulation
- No command queue (future U-series)
- Selection system unchanged
- Dev Mode uses the same intent dispatch path — no direct simulation mutation

## Future extensibility

- Palette hotkeys via `resolve_palette_command`
- Unit capability matrix in `command_palette.rs`
- HoldPosition, Interact, harvest, and building placement as new implementations
- Spell and ability commands as new `CommandType` variants
- Command queue as a layer above `BuiltCommandPlan`

# Consequences

**Benefits:**

- SC2-style command semantics without simulation rewrites
- Single extension point for abilities and worker commands
- UI accurately reflects what the game can do today
- Structured rejection aids debug trace and future player feedback

**Costs:**

- Extra indirection on right-click dispatch (negligible)
- Reserved command types remain in the enum until implemented

# References

- ADR-038 (intent pipeline)
- ADR-039 (trace layer, REVIEW-B3 rejection)
- ADR-040 (gameplay UI)
- ADR-030 (unit orders)
- ADR-056 / ADR-057 (combat commands)

[`ResolvedCommandFeedback`]: ../src/client/commands/mod.rs
