# ADR-043: Dev Mode Runtime Authoring System

# Status

Accepted (DEV MODE v1 — runtime authoring + debug control layer)

# Context

The project uses Excel-driven [`UnitCatalog`](../src/world/unit/catalog/registry.rs) and
[`DoodadCatalog`](../src/world/doodad/catalog/registry.rs) definitions loaded under the
Cargo `dev` feature. Designers need a lightweight in-engine way to browse those catalogs,
place instances at cursor positions, and toggle U-UI3 debug overlays without modifying
simulation rules or bypassing [`WorldData`](../src/world/data.rs) authority.

Prior tooling was compile-time only (`TerrainPreviewPlugin`, `spawn_dev_preview_units`).
There was no runtime F12 panel, no unified catalog browser, and no input-safe spawn path.

# Decision

## Dev mode is an authoring layer, not gameplay

```text
Input → Dev UI → spawn helpers → create_unit / create_doodad → WorldData → render sync
```

Dev mode **must not**:

- mutate movement, pathfinding, steering, or formation logic
- spawn ECS entities directly
- bypass catalog validation
- alter the client intent pipeline contract

## Module layout (`src/dev/`)

| Module | Responsibility |
|--------|----------------|
| `dev_mode.rs` | [`DevModeState`](../src/dev/dev_mode.rs), tabs, spawn mode, debug flags |
| `catalog_browser.rs` | In-memory filter/search over unit + doodad catalogs |
| `spawn_tools.rs` | Terrain-grounded spawn via [`create_unit`](../src/world/unit/authoring.rs) / [`create_doodad`](../src/world/doodad/authoring.rs) |
| `debug_controls.rs` | Maps dev flags → [`DebugOverlaySettings`](../src/debug/settings.rs) |
| `panel.rs` | Bevy UI panel (tabs, list, toggles) |
| `input.rs` | F12 toggle, search keys, spawn click, [`DevModeInputGate`](../src/dev/dev_mode.rs) |
| `mod.rs` | [`DevModePlugin`](../src/dev/mod.rs) registration |

The entire module is behind `#[cfg(feature = "dev")]` and registered from
[`PlayerPlugin`](../src/player/plugin.rs).

## Runtime state

[`DevModeState`](../src/dev/dev_mode.rs) is a **client-local resource** (not simulation
truth). Key fields:

- `enabled` — F12 toggle
- `active_tab` — Units | Doodads | Debug | World Tools
- `search_query`, `enabled_only`, `selected_definition`
- `debug_flags` — mirrors U-UI3 overlay categories

## Source tagging

New variants [`UnitSource::Dev`](../src/world/unit/source.rs) and
[`DoodadSource::Dev`](../src/world/doodad/source.rs) distinguish runtime dev placements
from authored and procedural content. Procedural key derivation treats `Dev` like
`Authored` (no procedural identity).

## Spawn pipeline

1. Player selects a catalog row in the dev panel.
2. Left-click on terrain (when panel is not hovered) raycasts via existing
   [`terrain_click_to_world_position`](../src/units/input/terrain_click.rs).
3. X/Z from render pick; Y from [`ground_world_position`](../src/world/terrain/query.rs).
4. [`spawn_selected_at_position`](../src/dev/spawn_tools.rs) calls authoritative APIs only.

Units/doodads at the click location are **not** used for placement raycasts (spawn ignores
pick targets).

## Input priority

| Condition | Behavior |
|-----------|----------|
| Dev mode off | Normal gameplay input |
| Panel hovered | Mouse blocked via [`DevModeInputGate`](../src/dev/dev_mode.rs) |
| Definition selected + terrain click | Spawn; block gameplay mouse for that frame |
| Otherwise | Normal selection / move commands |

The gate is checked in [`collect_unit_input_intents`](../src/client/pipeline.rs) under
`feature = "dev"` only — minimal coupling, no intent type changes.

## Debug overlay integration

Dev Debug tab toggles map directly to [`DebugOverlaySettings`](../src/debug/settings.rs):

| Dev flag | Overlay field |
|----------|---------------|
| `show_paths` | `path` |
| `show_steering_vectors` | `steering` |
| `show_formations` | `formation` |
| `show_selection_circles` | `selection` |
| `show_interaction_hits` | `interaction` |
| `show_command_trace` | `intent` |
| `show_grid_overlay` | Reserved (no grid overlay yet) |

Overlays remain read-only gizmo systems (ADR-039).

## UI approach

Bevy UI (`Node`, `Button`, `Text`) — same pattern as gameplay HUD (ADR-040). Panel is
right-docked, hidden until F12. Search uses keyboard capture when the panel is not hovered.

# Consequences

## Positive

- Safe runtime placement aligned with simulation authority
- Catalog-driven browsing without external indexing
- Debug visualization controllable from one panel
- Clear extension point for scenario editor / brushes / save states (World Tools tab)

## Negative

- Requires `--features dev` build for the panel
- Spawn click consumes left-click when a definition is selected (intentional tradeoff)
- Grid overlay toggle is a no-op until a grid debug system exists

# Future work

- World Tools tab: scenario snapshots, exclusion brushes, chunk reload
- Text input widget for search (replace keyboard capture)
- Optional middle-click spawn to avoid selection conflict
- Grid navigation debug overlay wired to `show_grid_overlay`

# Verification

- `cargo check --features dev`
- `cargo test --lib --features dev` (dev module tests + existing suite)
- Spawn tests assert `UnitSource::Dev` / `DoodadSource::Dev` in chunk stores
