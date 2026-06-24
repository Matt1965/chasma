# ADR-050: Player HUD Foundation (P-UI1)

# Status

Accepted (P-UI1 — Player HUD foundation)

# Context

ADR-040 established a minimal gameplay UI layer with a small bottom-left HUD
(selection count, command label, portrait placeholders). ADR-041 added the
contextual command palette (Move, Stop, Hold). The project needs a **real**
player-facing HUD that combines:

- **StarCraft II** command clarity (bottom command bar, explicit orders)
- **Kenshi** inspection depth (detailed unit stats, squad roster)

Dev tools remain on **egui** (ADR-043/047). Player HUD must use **Bevy UI**.

# Decision

## Philosophy

Hybrid SC2/Kenshi layout:

| Zone | Purpose |
|------|---------|
| Bottom-left | Selected unit stats / inspection |
| Bottom-center | Squad / available units roster |
| Bottom-right | Command panel (Move, Stop, Hold + future placeholders) |

**No minimap** in P-UI1 — space is not reserved.

## Technology split

- **Bevy UI** — all player HUD (`src/ui/gameplay/`)
- **egui** — dev panel, inspector, tools only

## Module layout

| Module | Responsibility |
|--------|----------------|
| [`layout.rs`](../src/ui/gameplay/layout.rs) | Root HUD + bottom bar spawn |
| [`selected_unit_panel.rs`](../src/ui/gameplay/selected_unit_panel.rs) | Stats from selection + WorldData + UnitCatalog |
| [`squad_panel.rs`](../src/ui/gameplay/squad_panel.rs) | Roster + click-to-select |
| [`command_panel.rs`](../src/ui/gameplay/command_panel.rs) | Command buttons + intent emission |
| [`player_hud_state.rs`](../src/ui/gameplay/player_hud_state.rs) | `PlayerHudState` (UI-only) |
| [`input_gate.rs`](../src/ui/gameplay/input_gate.rs) | Pointer capture blocking world intents |
| [`styles.rs`](../src/ui/gameplay/styles.rs) | Shared HUD styling |

## Data flow

```text
WorldData (read) ──┐
UnitCatalog (read)├──► HUD panels (Bevy UI)
SelectedUnits (read)┘
         ▲
         │ ClientIntent (SelectUnit, Toggle, PaletteCommand)
         │
    Squad / Command panels
```

Gameplay UI **never** mutates `WorldData`. Selection changes go through
[`ClientIntentQueue`](../src/client/intent.rs). Stop/Hold emit
`ClientIntent::PaletteCommand`.

## Primary selection rule

When multiple units are selected, the **lowest `UnitId` by raw value** is
the primary inspect target (deterministic; matches legacy leader unit rule).

## Input capture

When the cursor interacts with HUD widgets (`PlayerHudHoverState`), the client
intent collector skips mouse-based world selection/commands — mirroring dev panel
gating.

## Command panel (P-UI1 scope)

- **Enabled:** Move (arms UI mode), Stop, Hold Position
- **Disabled placeholders:** Attack, Harvest, Interact
- Only enabled commands emit gameplay intents (Stop/Hold via `PaletteCommand`)

## Future expansion

- Command card abilities
- Inventory / equipment panels
- Squad persistence and Kenshi-style management
- Minimap (dedicated ADR when added)
- Unit portraits / icons

# Consequences

- Player HUD is a distinct layer from dev UI
- Bottom bar replaces minimal corner HUD
- Command palette is now player-visible, not just logical
- Tests cover HUD state, stats formatting, command enablement, input gating

# Related

- ADR-040 Gameplay UI Layer
- ADR-041 Contextual Command System
- ADR-038 Client Intent Pipeline
