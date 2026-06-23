# ADR-041: Contextual Command System (U-UI5)

# Status

Accepted (U-UI5 — RTS command layer expansion)

# Context

U-UI2 established the intent pipeline (`ClientIntent` → dispatch → command APIs).
U-UI3 added trace/debug observability. U-UI4 added gameplay UI that **reflects**
client state without owning command logic.

Right-click previously always emitted `MoveCommand`. SC2-style RTS control requires
contextual command semantics (move, stop, hold, future attack/interact) while
keeping simulation rules unchanged.

# Decision

## Pipeline extension

```text
Input → ClientIntent::ContextualCommand { target }
     → resolve_contextual_command (classification only)
     → build_command_plan
     → issue_move_orders_to_selection / issue_idle_orders_to_selection
     → issue_unit_order → WorldData
     → ResolvedCommandFeedback (UI hook)
```

Legacy `ClientIntent::MoveCommand` remains and routes through the same path.

## New module: `src/client/commands/`

| Module | Responsibility |
|--------|----------------|
| `command_types.rs` | `CommandType`, `CommandTarget`, `ContextualCommandIntent` |
| `context_resolver.rs` | Terrain/unit click → contextual intent (no gameplay logic) |
| `command_builder.rs` | Contextual intent → `BuiltCommandPlan` |
| `command_palette.rs` | Available commands per selection (SC2 palette foundation) |

## Command types (U-UI5 scope)

| Type | U-UI5 behavior |
|------|----------------|
| Move | Fully functional via formation move dispatch |
| Stop | Routes to `UnitOrder::Idle` |
| HoldPosition | Placeholder — idle routing until hold mechanics |
| AttackMove | Placeholder — resolves to MoveTo |
| Interact | Placeholder — resolves to MoveTo |

## Context resolution rules

- Right-click terrain → `CommandType::Move`
- Right-click unit → `CommandType::Move` (fallback until combat)
- Empty selection → intent ignored
- Invalid target → no intent emitted (input collection)
- Mixed-capability selection → palette intersection (static subset for now)

## UI integration (U-UI4 hook only)

[`ResolvedCommandFeedback`](../src/client/commands/mod.rs) stores the last resolved
`CommandType` and tooltip. Gameplay UI reads this resource — it never decides commands.

## Boundaries (unchanged)

- No changes to pathfinding, steering, formation, or movement simulation
- No combat, damage, ability execution, or AI
- No command queue (future U-series)
- Selection system unchanged

## Future extensibility

- Palette hotkeys via `resolve_palette_command`
- Unit capability matrix in `command_palette.rs`
- AttackMove / Interact classification in `context_resolver.rs`
- Spell, harvest, and building placement commands as new `CommandType` variants
- Command queue as a layer above `BuiltCommandPlan`

# Consequences

**Benefits:**

- SC2-style command semantics without simulation rewrites
- Single extension point for abilities and worker commands
- UI can show command type/tooltip from read-only feedback

**Costs:**

- Extra indirection on right-click dispatch (negligible)
- Placeholder command types must be clearly documented to avoid misuse

# References

- ADR-038 (intent pipeline)
- ADR-039 (trace layer)
- ADR-040 (gameplay UI)
- ADR-030 (unit orders)

[`ResolvedCommandFeedback`]: ../src/client/commands/mod.rs
