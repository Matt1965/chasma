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
- bypass catalog validation (including scene restore — see ADR-045)
- alter the client intent pipeline contract

## Module layout (`src/dev/`)

| Module | Responsibility |
|--------|----------------|
| `dev_mode.rs` | [`DevModeState`](../src/dev/dev_mode.rs), tabs, spawn mode, debug flags |
| `catalog_browser.rs` | In-memory filter/search over unit + doodad catalogs |
| `spawn_tools.rs` | Terrain-grounded spawn via [`create_unit`](../src/world/unit/authoring.rs) / [`create_doodad`](../src/world/doodad/authoring.rs) |
| `debug_controls.rs` | Maps dev flags → [`DebugOverlaySettings`](../src/debug/settings.rs) |
| `panel.rs` | Bevy UI panel content (tabs, list, toggles) |
| `catalog/` | Catalog window — tab routing, Advanced Mode, contextual placement (Slice 4) |
| `window/` | Draggable dev-window framework (Slice 3) |
| `input.rs` | F12 toggle, search keys, spawn click, [`DevModeInputGate`](../src/dev/dev_mode.rs) |
| `mod.rs` | [`DevModePlugin`](../src/dev/mod.rs) registration |

The entire module is behind `#[cfg(feature = "dev")]` and registered from
[`PlayerPlugin`](../src/player/plugin.rs).

## Runtime state

[`DevModeState`](../src/dev/dev_mode.rs) is a **client-local resource** (not simulation
truth). Key fields:

- `enabled` — F12 toggle
- `active_tab` — Units | Doodads | Buildings | Items | Scenes | Inspect (transitional) | World | Fields | Debug (advanced)
- `search_query`, `enabled_only`, `selected_definition`
- `catalog` — [`CatalogSessionState`](../src/dev/catalog/state.rs): Advanced Mode, tab memory, compact status (Slice 4; client-local, not scene-persisted)
- `debug_config` — mirrors U-UI3 overlay categories

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
| `show_grid_overlay` | Walkable navigation cells (`grid`) |
| `show_nav_blockers` | Blocked nav cells by passability reason |
| `show_nav_footprints` | Building footprint outlines |
| `show_nav_entrances` | Portal / entrance markers |
| `show_nav_reservations` | Construction-reserved occupancy |
| `show_nav_occupancy` | Static blocked occupancy cells |

Overlays remain read-only gizmo systems (ADR-039). **REVIEW-A6:** overlay draw systems
compile and register only with `feature = "dev"`; production builds use
[`DebugOverlayConfig::production()`](../src/debug/settings.rs) (all categories off).
Toggle changes persist for the session via [`DevModeState::debug_config`](../src/dev/dev_mode.rs).

## UI approach

Bevy UI (`Node`, `Button`, `Text`) — same pattern as gameplay HUD (ADR-040). As of Dev UI
Slice 3, the legacy panel lives inside a draggable **Catalog** window
([`src/dev/window/`](../src/dev/window/mod.rs)); session layout is client-local only.
A **Windows** launcher (top-left) reopens hidden windows. Search uses keyboard capture
when a text field is focused (DV2).

## Catalog workspace (Slice 4)

The **Catalog** window is the primary dev surface for asset discovery and placement.

**Standard tabs:** Units, Doodads, Buildings, Items, Scenes.

**Advanced Mode** (session-only toggle, default off): reveals World, Fields, Editor, and Debug tabs as **launchers** for dedicated windows (Slice 8).
Underlying systems keep running when launchers are hidden; open windows stay open when Advanced Mode turns off; enabled debug overlays stay on.

**Dedicated windows (Slice 8):** `DevWindowId::Debug`, `World`, `Fields` — draggable shells in [`src/dev/window/`](../src/dev/window/), content in `debug_window/`, `world_window/`, `fields_window/`. Window visibility is session-only; overlay flags, lighting, and field state are not reset on hide.

**Transitional:** Pile/treasury harness keyboards remain while World window is open (Slice 12 migration).

**Placement:** The standalone Placement tab is removed. Select a definition on Units,
Doodads, or Buildings; contextual placement controls appear below the catalog list.
Active placement persists across tab switches; a banner shows when viewing other tabs.
Cancel via **Cancel placement** or right-click (centralized policy in [`handle_dev_right_click_input`](../src/dev/input.rs)).

**Dev overlap:** [`PlacementRules::avoid_doodads`](../src/dev/tools/placement_rules.rs) defaults
to `false` so dev placement allows overlap (unrelated validity checks remain). Transform commit
overlap is unconditional ([`dev_gizmo_*_commit_options`](../src/dev/gizmo/commit.rs)).

Deferred to later slices: lighting sliders and project-default persistence (11), full building-action and harness hotkey migration (12).

## Shared dev widgets and tooltips (Slice 9)

- Widget library: [`src/dev/widgets/`](../src/dev/widgets/) — toggles, steppers, collapsible sections, badges, status lines, confirmation bars, numeric draft helpers, search styling
- Tooltip foundation extended in [`src/dev/tooltip/`](../src/dev/tooltip/) — `DevTooltipContent`, hover delay, viewport clamp, `DevTooltipHoverZone` for disabled controls
- Presentation-only: widgets emit existing domain actions; no second authoritative value except ephemeral numeric drafts while focused
- Retrofit priority: Navigation Editor, Debug, World, Fields, Selected Object, Catalog placement
- Not in scope: lighting sliders/persistence (11), full building-action migration (12), final styling (13)

## Dev hotkey infrastructure (Slice 6)

- Code registry: [`src/dev/hotkeys.rs`](../src/dev/hotkeys.rs) (`DEV_HOTKEY_REGISTRY`, `dev_shortcuts_suppressed()`)
- **Esc** removed from dev handlers (reserved for pause menu)
- **L** coordinate-space toggle removed; gizmos world-aligned (`DEV_GIZMO_COORDINATE_SPACE`)
- Hold **O** / **G** transform modifiers removed; initial placement terrain snap unchanged
- **/** retained as Scale only; suppressed during text focus and modals
- Right-click precedence centralized in [`handle_dev_right_click_input`](../src/dev/input.rs)
- Blueprint Esc replacements: Selected Object buttons (exit inspection/edit, cancel pending, cancel variant draft)

## Selected Object window (Slice 5)

World-object inspection lives in a dedicated **Selected Object** draggable window
([`DevWindowId::SelectedObject`](../src/dev/window/id.rs)), driven entirely by
[`WorldSelectionState`](../src/client/selection/mod.rs). The legacy Catalog **Inspect** tab is removed.

- Summary view by default; diagnostics collapsed behind a toggle
- Building navigation strip + temporary authoring section (Slice 7 migrates to Navigation Editor)
- Transform buttons share [`activate_dev_transform_tool`](../src/dev/gizmo/input.rs) with `,` `.` `/` hotkeys
- Catalog definition selection remains separate from world-object selection

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

- World Tools tab: exclusion brushes, chunk reload (scenario snapshots via ADR-045 validated restore)
- Text input widget for search (replace keyboard capture)
- Optional middle-click spawn to avoid selection conflict
- Grid navigation debug overlay wired to `show_grid_overlay`

# Verification

- `cargo check --features dev`
- `cargo test --lib --features dev` (dev module tests + existing suite)
- Spawn tests assert `UnitSource::Dev` / `DoodadSource::Dev` in chunk stores
