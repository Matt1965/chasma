# ADR-038: Intent-Based Client Architecture (U-UI2)

# Status

Accepted (U-UI2 — client intent pipeline foundation)

# Context

U8–U12 established authoritative unit control on [`WorldData`], client-local
selection ([`SelectedUnits`]), and presentation feedback in the Player layer.
Input systems in [`handle_unit_selection_input`] directly updated selection and
issued move orders, coupling device reads to command dispatch.

Future replay, AI control, multiplayer sync, and debug tooling require a stable
boundary between **input observation** and **command emission**. SC2-style RTS
control benefits from a deterministic, order-preserving action pipeline.

# Decision

## Pipeline

```text
Input (devices) → ClientIntentQueue → Intent dispatch → Command APIs → WorldData
                                              ↓
                                    Presentation (read-only mirror)
```

## Client layer (`src/client/`)

| Module | Responsibility |
|--------|----------------|
| [`intent.rs`](../src/client/intent.rs) | Pure-data [`ClientIntent`], [`ClientIntentQueue`], [`ClientInputModifiers`] |
| [`dispatcher.rs`](../src/client/dispatcher.rs) | Route intents to selection + [`issue_move_orders_to_selection`] |
| [`pipeline.rs`](../src/client/pipeline.rs) | [`collect_unit_input_intents`], plugin resources |

## Intent model

Intents are **pure data** — no behavior inside enum variants:

- `SelectUnit`, `ToggleUnitSelection`, `BoxSelect`, `BoxSelectAdd`
- `ClearSelection`, `MoveCommand`, `ShiftModifier`

Input systems **push** intents only. They use read-only world access for ray
pick resolution. They never call [`issue_unit_order`] or mutate [`SelectedUnits`].

## Dispatch rules

- [`dispatch_client_intents`] drains the queue each frame (FIFO, deterministic).
- Selection updates go through [`SelectedUnits`] only.
- Simulation changes go through existing command APIs only.
- Empty selection causes `MoveCommand` to be **ignored** (unchanged from U9).

## Schedule (Player control chain)

```text
tick_unit_movement → collect_unit_input_intents → dispatch_client_intents → presentation sync
```

Movement resolves the command buffer before new intents are collected.

## Debug

[`PlayerInteractionSettings::debug_intents`] logs each intent and dispatch result.

**REVIEW-A6:** Debug overlay *visualization* is dev-feature-gated and defaults off in
production. Gameplay presentation (selection rings, move feedback) is separate and always
available. See ADR-039.

## Migration (U-UI2 scope)

- Removed direct [`handle_unit_selection_input`] side effects.
- Selection and move behavior preserved via intent bridge.
- [`units/input/`](../src/units/input/) retains picking, selection helpers, commands.

Future U-UI phases migrate presentation under `client/presentation/` and devtools
under `client/devtools/` without changing this pipeline contract.

# Consequences

**Benefits:**

- Input decoupled from simulation; replay/AI can enqueue intents later
- Deterministic, order-preserving client actions
- SC2-style separation: observe → intent → command

**Costs:**

- Extra indirection per frame (negligible)
- Two systems replace one monolithic input handler

# References

- ADR-033 (player control)
- ADR-034 (multi-unit selection)
- ADR-037 (movement feel / command buffer)

[`WorldData`]: ../src/world/data.rs
[`SelectedUnits`]: ../src/units/input/selection.rs
[`handle_unit_selection_input`]: ../src/units/input/mod.rs
[`issue_unit_order`]: ../src/world/unit/orders.rs
[`issue_move_orders_to_selection`]: ../src/units/input/commands.rs
[`ClientIntent`]: ../src/client/intent.rs
[`ClientIntentQueue`]: ../src/client/intent.rs
[`ClientInputModifiers`]: ../src/client/intent.rs
[`collect_unit_input_intents`]: ../src/client/pipeline.rs
[`dispatch_client_intents`]: ../src/client/dispatcher.rs
[`PlayerInteractionSettings`]: ../src/units/input/settings.rs
