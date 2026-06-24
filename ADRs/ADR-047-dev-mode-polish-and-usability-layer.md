# ADR-047: Dev Mode Polish and Usability Layer

# Status

Accepted (dev tooling only — U-DEV1)

# Context

Dev mode (ADR-043/044) provides runtime authoring: catalog browse, spawn tools,
scene snapshots, and debug toggles. As catalogs grow and workflows deepen, the
panel suffered from:

- Lost UI state on tab switches
- Per-frame full catalog filtering
- No favorites or quick-spawn paths
- Split debug toggle representations
- Fragile mouse/keyboard input ordering vs gameplay

This ADR covers **presentation and tooling** only. It must not alter simulation
rules, intent/command architecture, or debug overlay rendering logic.

# Decision

## DevModeState as session tool memory

[`DevModeState`](../src/dev/dev_mode.rs) owns all dev UI persistence while F12
mode is active:

| Field group | Persisted |
|-------------|-----------|
| Navigation | `active_tab`, `search_query`, `enabled_only` |
| Spawn | `selected_definition`, `spawn_mode`, `brush`, placement flags |
| Favorites | `favorites`, `favorite_slots[9]` |
| Audit | `spawn_history`, `last_spawn` |
| Debug | `debug_config` ([`DebugOverlayConfig`](../src/debug/settings.rs)) |

Reset only on:

- Full app reload
- Explicit **Reset dev state** (Debug tab)

Tab switches **do not** clear selection or search.

## Catalog performance (in-memory)

[`CatalogBrowseIndex`](../src/dev/catalog_cache.rs) pre-builds rows when catalog
length changes. [`CatalogFilterCache`](../src/dev/catalog_cache.rs) stores filtered
results keyed by tab, query, enabled-only, and favorites hash.

[`DevSearchDebounce`](../src/dev/catalog_cache.rs) delays filter application by
4 frames so typing does not recompute every frame.

Index fields per row: name, render key, category, definition id (biome tags when
present on definitions).

## Favorites

- Toggle: **F** with a row selected, or UI list star prefix
- Pin: matching favorites sort to list top via `pin_favorites`
- Slots: **Ctrl+1–9** assign, **1–9** recall selection

Stored in `DevModeState` only (no disk persistence yet).

## Quick spawn shortcuts

| Input | Behavior |
|-------|----------|
| 1–9 | Select favorite slot definition |
| Ctrl+1–9 | Assign slot from current selection |
| Ctrl+click terrain | Repeat last spawned definition |
| Shift+click terrain | Batch spawn (count ≥ 5) |

All spawns go through [`execute_batch_spawn`](../src/dev/tools/batch_spawn.rs) →
`create_unit` / `create_doodad`.

## Spawn history

[`DevSpawnHistory`](../src/dev/history.rs) records definition, position, spawn
type, and simulation tick. Session-local audit trail for debugging and future
undo/replay — **not** simulation truth.

## DebugOverlayConfig consolidation

Single source of truth: [`DebugOverlayConfig`](../src/debug/settings.rs).

Dev panel toggles edit `DevModeState.debug_config`; [`sync_dev_debug_controls`](../src/dev/debug_controls.rs)
copies into the live overlay resource each frame. Overlay systems unchanged.

Scene snapshots map to/from [`SceneDebugFlagsSnapshot`](../src/dev/scenes/snapshot.rs)
for on-disk compatibility.

## Input safety

[`DevModeInputGate`](../src/dev/dev_mode.rs):

- `block_gameplay_mouse` when panel hovered or dev spawn handled
- `spawn_handled_this_frame` prevents duplicate spawn on one click

Dev spawn runs **before** [`collect_unit_input_intents`](../src/client/pipeline.rs).

Panel hover uses UI `Interaction` on [`DevPanelUi`](../src/dev/input.rs) nodes.

## UI layout

Sim status text (variable width) precedes fixed-width Pause/Step buttons so
button positions remain stable when tick/state labels change.

# Consequences

## Positive

- Faster catalog browse at scale
- Stable authoring workflow across tabs
- One debug config path from UI → overlay
- Spawn audit trail without gameplay coupling

## Negative

- Debounced search lags raw input by ~4 frames (cosmetic)
- Favorites lost on explicit reset or reload (disk persistence deferred)

# Non-goals

- Simulation rule changes
- Disk persistence for favorites/history
- Undo/redo (history only)
- Intent system changes
- Custom debug overlay rendering

# References

- ADR-043 Dev Mode Architecture
- ADR-044 Dev Spawn Tools
- ADR-039 Debug Overlay (U-UI3)
- ADR-046 Simulation Pause
