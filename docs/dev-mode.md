# Dev Mode (F12)

Runtime authoring UI for spawning units/doodads, scenes, inspector, and debug overlays.
See ADR-043, ADR-044, ADR-047, and DV2 usability refresh.

## Hotkey registry

Code inventory (Slice 6): [`src/dev/hotkeys.rs`](../src/dev/hotkeys.rs) — `DEV_HOTKEY_REGISTRY`, `dev_shortcuts_suppressed()`.

Design spreadsheet for **all** keyboard and mouse bindings (Gameplay, Camera, Global, Dev Mode) is the **Hotkeys** sheet in [`Chasma Design.xlsx`](../Chasma%20Design.xlsx).

**When you add, remove, or change any hotkey in code:**

1. Update the matching row(s) in `tools/update_hotkeys_sheet.py` (the `ROWS` inventory).
2. Regenerate the sheet: `python tools/update_hotkeys_sheet.py`
3. Update this document if the change is player-facing.

Audit bindings in code with: `rg "KeyCode::" src/`

The sheet includes an **Overlap Notes** column for keys reused across contexts (e.g. `T` for spawn
team vs. inventory endpoint cycle). Resolve overlaps when redesigning input — do not add silent
conflicts between Gameplay and Dev Mode unless context-gated in code.

## Dev windows (Slice 3)

The legacy dev panel is hosted in a draggable **Catalog** window (`src/dev/window/`).

| Feature | Behavior |
|---------|----------|
| Drag | Title bar only; grab offset preserved; continues outside window bounds |
| Z-order | Click or drag brings window forward (`DevWindowRegistry` focus stack) |
| Clamp | Title bar stays recoverable (≥80 px grab region); re-clamps on viewport resize |
| Close | Hides window; reopen via **Windows → Catalog** launcher (top-left) |
| Collapse | Title-bar `−` hides body; position and tab state preserved |
| F12 | Hides workspace without destroying UI; positions restored on re-enable |
| Input | `DevWindowInteractionState` → `DevModeInputGate` → `PlayerHudHoverState.dev_panel_blocks` |

Window layout is **client-local session state** only — not stored in `WorldData` or scene saves.

## Catalog workspace (Slice 4)

The **Catalog** window is the primary dev surface for asset discovery and placement.

**Standard tabs:** Units, Doodads, Buildings, Items, Scenes.

**Advanced Mode** (toggle below tabs, default off): reveals World, Fields, **Editor**, and Debug tabs as **launchers** — each opens a dedicated window. Underlying systems keep running when launchers are hidden; open windows stay open when Advanced Mode turns off; debug overlays stay on if already enabled.

**Debug** (Slice 8): dedicated draggable window (`DevWindowId::Debug`) for overlay toggles, NV0 navigation diagnostics, and animation readouts. Open from **Catalog → Advanced → Debug** or **Windows → Debug**. Closing the window does not reset overlay flags.

**World** (Slice 8 / 11): dedicated window (`DevWindowId::World`) for time-of-day, day/night/twilight/manual lighting, and **project-default persistence**. Bounded sliders with numeric entry replace legacy +/- steppers. Open from **Catalog → Advanced → World** or **Windows → World**. Closing does not stop the cycle or reset lighting; dirty state persists for the session.

**Fields** (Slice 8): dedicated window (`DevWindowId::Fields`) for terrain field build, validation, probe, and overlay toggles. Open from **Catalog → Advanced → Fields** or **Windows → Fields**. Probe runs only when probe mode is on, the Fields window is open, and the pointer is not over dev UI.

**Navigation Editor** (Slice 7): dedicated draggable window (`DevWindowId::NavigationEditor`) for building navigation blueprint inspect/edit. Open from **Selected Object → Open Navigation Editor** or **Catalog → Advanced Mode → Editor**. Closed by default; reopen via **Windows → Navigation Editor** launcher. World mouse editing runs only while this window is visible.

**Selected Object** (Slice 5): dedicated draggable window driven by shared world selection — shows navigation summary strip and **Open Navigation Editor** for buildings.

**Placement:** The standalone Placement tab is removed. Select a definition on Units, Doodads, or Buildings; contextual placement controls appear below the catalog list. Active placement persists across tab switches; a banner shows when viewing other tabs. Cancel via **Cancel placement** or right-click (see Tool cancellation).

**Dev overlap:** `PlacementRules.avoid_doodads` defaults to false so dev placement allows overlap (unrelated validity checks remain).

Catalog session state (`CatalogSessionState`, Advanced Mode, tab memory) is client-local — not in scene saves.

Window visibility is session-only (`DevWindowRegistry`). Domain state (overlay flags, lighting, field data) is independent — hiding a window does not reset it.

## Shared tooltips (Slice 7–9)

Hover tooltips use `src/dev/tooltip/` — one shared popup (`ZIndex` 2000, above dev windows), **0.45 s hover delay**, viewport clamping, `FocusPolicy::Pass` (no click capture). Content model: `DevTooltipContent` (title, body, units, scope, shortcut, disabled reason). Critical errors remain visible in panel text, not tooltip-only. Tooltips clear on **F12** off; not scene-persisted.

## Shared dev widgets (Slice 9)

Presentation-only controls in `src/dev/widgets/` — they read domain state, emit existing actions, and never own authoritative gameplay data.

| Widget | Use when |
|--------|----------|
| `spawn_toggle_row` | Booleans (debug overlays, preview, snap) |
| `spawn_bounded_slider_row` | Bounded environment tuning with numeric entry (Slice 11) |
| `spawn_labeled_stepper_row` / `spawn_stepper_button` | Legacy +/- steppers (retired from World window in Slice 11) |
| `parse_numeric_draft` / `NumericDraft` | Direct numeric entry helpers (draft while typing; commit on Enter/blur) |
| `spawn_segmented_control` | Small enums (2–5 options) |
| `spawn_collapsible_section` | Advanced diagnostics, World/Fields groups |
| `spawn_confirmation_bar` | Destructive actions (inline confirm/cancel, no Esc) |
| `spawn_status_line` | Compact info/success/warning/error near actions |
| `spawn_badge` | Asset default, dirty, valid/warning/error labels |
| Search constants | Catalog/scene field colors, placeholders, tooltip text |

**Retrofitted (Slice 9):** Debug toggles + collapsible categories; World harness section; Fields build section; Catalog search, Advanced Mode, placement controls; Selected Object actions; Navigation Editor tooltips.

**Slice 13 (visual standardization):** Shared theme in `src/dev/widgets/theme.rs` — window chrome, spacing scale, typography, status colors, tooltip styling. Panels migrate to theme constants incrementally; Catalog host retains local row metrics.

## Dev UI Revamp — completion status

The 13-slice Dev UI Revamp is **functionally complete**. Remaining work is limited to non-blocking polish (additional panel theme migration, hotkey workbook regeneration when `Chasma Design.xlsx` is available).

**Client-local (not scene-persisted):** window positions, visibility, collapse, Z-order, Advanced Mode, catalog tab, collapsible sections, tooltip state, inventory drag preview, search debounce.

**Persisted separately:** scene entities/transforms/navigation overrides; `assets/environment/project_defaults.ron` via explicit Save.

**Manual test guide:** see consolidated checklist at end of this document.

## Consolidated manual test guide

1. **Startup / F12** — Toggle dev mode; workspace launcher appears; windows restore positions.
2. **Windows** — Drag title bars, close/collapse, reopen via launcher; verify clamp at 1280×720.
3. **Selection** — Gameplay ↔ dev selection sync; category exclusivity; presentation rings.
4. **Catalog** — Tabs, search, favorites 1–9, contextual placement, Advanced launchers.
5. **Placement** — All patterns; cancel; right-click; overlap allowed.
6. **Selected Object** — Summary, diagnostics toggle, building action sections, navigation strip.
7. **Transforms** — `,` `.` `/` and buttons; axis X/Y/Z during drag; text-field suppression.
8. **Navigation Editor** — Full workflow; dirty guards on scene load/F12.
9. **Debug** — Overlays persist when window closed.
10. **World** — Lighting sliders; dirty; Save/Revert/Reset project defaults.
11. **Fields** — Build/validate/probe; probe only when armed.
12. **Inventory drag** — Ghost validity; ground preview; intent on drop.
13. **Building actions** — All Selected Object sections; confirmations.
14. **Hotkeys** — Retained only; type in text fields without firing actions.
15. **Tooltips** — Hover delay; viewport clamp; disabled reasons.
16. **Input blocking** — UI scroll does not zoom camera; no world clicks through panels.
17. **Scenes** — Save/load; dirty navigation guard.
18. **Resolution** — 1280×720 minimum; resize while dragging windows.
19. **Non-dev build** — Project defaults read-only where applicable.


## Dev actions and hotkeys (Slice 12)

Building dev actions (construction, production, inventory, logistics, lifecycle, doors, terrain) moved to **Selected Object** collapsible sections with tooltips. No keyboard-only building shortcuts remain.

**World → Transitional utilities:** pile harness (validate, spawn, drop, pickup, loot) and treasury harness (transaction log, wealth sum, create settlement, inspect, deposit) are panel buttons only.

**Catalog → Items → Manage:** inventory manage actions are UI-only (no letter-key shortcuts).

**Removed transitional shortcuts (Slice 12):**

| Key | Former action | Replacement |
|-----|---------------|-------------|
| `[` `]` `,` `.` (World) | Time-of-day adjust | World window sliders and buttons |
| Building letter keys | Damage, production, logistics, etc. | Selected Object sections |
| P/D/O/H/G/L/V (World) | Pile harness | World harness buttons |
| C/Y/E/B/J (World) | Treasury harness | World harness buttons |
| Items `G` | Arm pile placement | **Spawn pile** button |
| Doodad arrows / `[` `]` | Nudge transform | Transform gizmo (`,` `.` `/`) |

**Retained global dev shortcuts:** F12, Ctrl+F, Tab, E (catalog filter), `,` `.` `/` (gizmo), 1–9 favorites, right-click cancellation policy.

**Retained context-local:** X/Y/Z (gizmo axis lock during drag), Delete (Navigation Editor edit mode).

Gameplay bindings (B, I, camera, unit commands) are unchanged. **O** toggles Terrain Analysis while dev mode is on.

## Inventory drag previews (Slice 10)

Gameplay inventory dragging (`src/ui/gameplay/inventory/`) uses extended `InventoryDragState` and client-local `InventoryDragPreviewState`.

| Behavior | Detail |
|----------|--------|
| Drag start | Records source inventory, entry index, revision, item definition, footprint, quantity; source entry stays visible (dimmed) |
| Inventory ghost | Translucent cell-aligned overlay; green-tint valid, red invalid; uses `can_place_footprint` + access checks |
| Ground preview | Translucent sphere at actor feet (matches `DropEntry` authoritative placement); shown when pointer leaves grid while not over HUD |
| Drop | Mouse **release** emits existing `MoveEntry`, `TransferToCell`, or `DropEntry` intents — no direct UI mutation |
| Cancel | Escape clears drag; invalid release shows compact reason in feedback line |
| Stale revision | `entry_revision_for_inventory` mismatch invalidates preview and blocks submit |
| Rotation | Not implemented — previews use authored orientation only |
| Equipment slots | Not implemented (I6 placeholder) |
| Dev windows | Item drag blocks dev window title-bar drag |

Preview validity is **client prediction only**; authoritative acceptance remains in `dispatch_inventory_intents`.

## World environment and project defaults (Slice 11)

Three-layer model:

| Layer | Source | Writable |
|-------|--------|----------|
| Built-in defaults | `TimeOfDaySettings::default()` / `EnvironmentSettings::default()` | No (Reset loads into runtime only) |
| Project defaults | `assets/environment/project_defaults.ron` | Dev **Save as Project Defaults** only |
| Runtime | `TimeOfDaySettings` + `EnvironmentSettings` resources | World window controls (immediate) |

**Startup order:** built-in → load/validate project file → initialize runtime resources in `EnvironmentPlugin::build` → environment startup systems.

**Not persisted in project defaults:** current clock hour, pause flag, window layout, collapsed sections.

**Scene saves:** independent — `SceneDefinition` does not serialize environment (ADR-045).

**Release builds:** read project defaults; write path is `#[cfg(feature = "dev")]` only.

**Dirty state:** compares authored snapshot (excluding transient clock/pause) to loaded baseline. Persists across World close and F12; lost on exit without save.

**Numeric drafts:** Intermediate input (`-`, `.`, empty) does not corrupt authoritative values. Invalid text shows validation feedback; focus suppresses global dev shortcuts (`DevTextFieldFocus`, including World environment numeric fields).

**Disabled controls:** Use `DevTooltipHoverZone` or `DevTooltipContent::disabled_reason` when the control cannot receive `Button` interaction.

## Toggle

| Key | Action |
|-----|--------|
| **F12** | Toggle dev mode on/off |

## Keyboard focus (DV2)

Dev Mode uses explicit text-field focus. Global shortcuts only fire when `dev_shortcuts_suppressed()` is false:

- Catalog search, scene name, or item quantity fields focused
- Selected Object delete confirmation pending
- Blueprint pending confirmation or Save-As-Variant draft active

| Key | Action |
|-----|--------|
| **Ctrl+F** | Focus search / scene name field |
| **Enter** | Exit search focus (does not trap focus) |
| Click search box | Focus field |
| Click elsewhere in panel | Remove focus |
| Click terrain | Remove focus |

**Esc** is not consumed by dev systems (reserved for future pause menu).

While search is **focused**, letter keys type into the field (including **T**, **,**, **.**, **/**).
While search is **unfocused**, **T** cycles spawn team (Player ↔ Wilds).

## Tool cancellation (Slice 6)

| Input | Action |
|-------|--------|
| **Cancel placement** button (Catalog) | Cancel armed placement, clear preview ghosts |
| **Right-click** (centralized policy) | See precedence below |

Right-click precedence (world, dev mode on):

1. Pointer over dev/gameplay UI → UI owns click
2. Active dev-window drag → consume
3. Active placement tool → cancel placement
4. Active transform (drag or tool) → cancel drag or exit transform tool
5. Blueprint edit pending drag / confirmation / variant draft → cancel pending only
6. No valid gameplay command target under cursor → clear dev/world object selection
7. Valid gameplay command → pass through to gameplay pipeline

Cancellation does **not** clear RTS unit selection unless step 6 clears world-object selection only.

## Transform shortcuts (retained)

| Key | Action |
|-----|--------|
| **,** | Move (translate gizmo) |
| **.** | Rotate |
| **/** | Scale only (not search, not RepeatMode) |

Gizmos are **world-aligned**. Dev placement/transform **always allows overlap**. Moved objects are **not** re-snapped to terrain (initial placement still terrain-snaps).

Removed dev meanings (Slice 6): **Esc**, **L** (coordinate space), hold **O** (overlap), hold **G** (follow-ground on transform). Building, pile, treasury, and inventory-manage letter keys removed in Slice 12 (see below). **O** opens Terrain Analysis (dev mode only).

## Catalog shortcuts (unfocused, not suppressed)

| Key | Action |
|-----|--------|
| **Tab** | Cycle panel tabs |
| **E** | Toggle enabled-only filter |
| **T** | Cycle spawn team |
| **F** | Toggle favorite on selected definition |
| **1–9** | Recall favorite slot |
| **Ctrl+1–9** | Assign favorite slot |

## Terrain Fields (Slice 8 — Fields window)

Terrain field authoring moved to the **Fields** window (`DevWindowId::Fields`). Open via Catalog → Advanced → Fields or **Windows → Fields**.

Open the **Fields** window via Catalog → Advanced → Fields or **Windows → Fields**.
| Button | Action |
|--------|--------|
| **Build field** | Build and package the selected field from its source profile |
| **Build all** | Build all enabled fields |
| **Validate** | Validate the selected field's source profile |
| **Reload** | Reload packaged tiles (diff + reassess affected buildings) |
| **Reassess** | Rebuild all building terrain assessments |
| **Next field** | Cycle the probed field |
| **Probe** | Toggle cursor field probe |
| **Gizmos** | Toggle sample gizmos |

**Overlay toggles** (Water / Iron / Copper / Stone): show colored field maps on terrain. Multiple can stay on at once. Overlays only appear where terrain chunks are loaded — pan the camera to streamed areas.

After **Build field** or **Build all**, the game auto-reloads packages and turns on the relevant overlay(s). **Terrain Analysis** (`O`, dev mode) is a separate panel with the same overlay data.

The **Fields window** (not a catalog tab) hosts build, validate, probe, and visualization controls.

## Terrain Analysis (dev mode, ADR-103)

| Key | Action |
|-----|--------|
| **O** | Toggle Terrain Analysis panel (dev mode only) |
| **[** / **]** | Decrease / increase overlay opacity (panel open) |

**Terrain Analysis** button (bottom-right HUD, dev mode only): select field, adjust opacity, cursor value readout.
Overlay uses CPU field tiles; cursor values from `sample_terrain_field_at`, not GPU readback.

## World selection (Dev UI Slice 1)

Gameplay RTS selection and dev inspector picking share one client-local authority:

- [`WorldSelectionState`](../src/client/selection/mod.rs) — active category (units, building, doodad, item pile, or none) plus object ids
- [`SelectedUnits`](../src/units/input/selection.rs) — unit id set; all writes go through [`apply_world_selection`](../src/client/selection/mod.rs)
- [`WorldInspectorState`](../src/dev/inspector/state.rs) — **derived snapshots only** (no authoritative `selected_*` fields)

Category exclusivity: selecting units clears building/doodad/pile; selecting any world object clears units.
Primary inspected/command HUD unit = lowest raw [`UnitId`](../src/world/unit/id.rs) in the selected set.

Non-commandable units (e.g. enemies) can be inspected in dev mode via Alt+click or dev picks, but
[`filter_commandable_unit_ids`](../src/world/ownership/controllability.rs) strips them before orders issue.
[`GameplayBuildingSelection`](../src/ui/gameplay/building_selection.rs) is a one-way mirror updated only by the selection API.

## Selection presentation (Dev UI Slice 2)

[`client/selection/presentation`](../src/client/selection/presentation/mod.rs) observes selection authority and spawns client-local visuals only.

| Category | Visual | Footprint source | Height strategy |
|----------|--------|------------------|-----------------|
| Units | Green terrain-conforming ring (per unit) | `collision_radius_meters` × 2 (min 0.9 m) | Terrain-draped annulus parented to render entity |
| Primary unit | Same ring, slightly higher opacity (0.95 vs 0.85) | Same | Same |
| Building | Green oriented outline | `effective_building_footprint_for_placement` + instance uniform scale + yaw | Authoritative anchor Y + 0.06 m lift (not terrain-dragged) |
| Doodad | Green oriented outline | `resolve_doodad_collision` shape; non-blockers use interaction radius | Authoritative placement Y + lift |
| Item pile | Small green terrain ring | `max(0.45 m, min(merge_radius×0.35, fallback_sphere×0.65))` | Terrain-draped at pile anchor |

Debug **Selection** overlay draws orange inspector-focus rings only when focus diverges from selection — it does not duplicate normal green rings. NV0 navigation footprint overlays remain diagnostic (cyan/blue) and independent of [`DebugOverlayConfig`](../src/debug/settings.rs) for normal selection.

Presentation entities are rebuilt from authoritative ids when selection changes, transforms commit, or render entities are replaced. They are not persisted in scene saves.

**Known gaps:** doodads with `DoodadCollisionShape::Baked` fall back to scaled circle until baked masks load at runtime (documented in collision resolver).

## Placement

1. Select a definition on **Units**, **Doodads**, or **Buildings** in Catalog.
2. Configure contextual placement controls below the catalog list (pattern, count, spacing, team, snap).
3. **Left-click** terrain to spawn.
4. **Shift+click** — larger batch count.
5. **Ctrl+click** — repeat last spawn.

The **Tool** status block (below tabs) shows active tool, selection, team, and brush mode live. Cancel via **Cancel placement** or right-click (see Tool cancellation).

## Transform editing (gizmos)

With a **doodad** or **building** selected, use **Selected Object** transform buttons or retained shortcuts:

| Key / UI | Action |
|----------|--------|
| **,** / Move | Translate gizmo (world-aligned) |
| **.** / Rotate | Rotate gizmo |
| **/** / Scale | Scale gizmo (disabled when authored dimensions are fixed) |
| **X / Y / Z** | Axis constraint during active gizmo drag |
| **Left-drag handle** | Preview; release commits via authoritative transform API |
| **Right-click** | Cancel drag or exit transform tool |

Gizmos are permanently **world-aligned**. Dev placement/transform **always allows overlap**. Moved objects are **not** re-snapped to terrain. Removed modifiers: **Esc** (dev), **L** (coordinate space), hold **G** (follow-ground), hold **O** (overlap).

Building **production repeat** cycles via Selected Object → **Production repeat** button (Production section actions are in collapsible groups below).

See [ADR-099](../ADRs/ADR-099-dev-transform-gizmos-and-edit-transactions.md).

## Items (Catalog tab)

The **Items** tab provides catalog browsing plus developer inventory editing. Use panel buttons only (no letter-key shortcuts).

| Subtab | Purpose |
|--------|---------|
| **Catalog** | Item definitions |
| **Profiles** | Inventory profile definitions |
| **Manage** | Add/remove/fill/clear, transfer, ground pile placement |

Select a target via world selection (unit, building, or item pile). Manage panel buttons cycle endpoints/entries, adjust quantity, and run transfers. **Spawn pile** arms ground placement; click terrain to place. **Validate** runs world inventory validation.

Ground piles are normal inventory entities. Transfers use the same authoritative APIs as gameplay.

## World — test harnesses

**World → Test harnesses** (collapsible) provides pile and treasury diagnostic buttons (ADR-090, ADR-093). No keyboard shortcuts. Status appears below the button rows.

Scene save/load round-trips inventories, instances, corpses, piles, and treasuries.

## Player inventory UI (I6 — ADR-092)

Separate from the F12 Items harness. Toggle with **I** on the gameplay HUD (primary selected unit).
Interact command opens containers, corpses, and world piles when armed.

Dev inspection of open inventory IDs and authoritative entries is via Catalog → Items → Manage,
Selected Object building actions, and World test harnesses; player UI state lives in `InventoryUiState`.

## Asset sizing calibration (DT1, read-only)

When a Unit, Doodad, or Building definition is selected, the **Asset sizing** block shows source bounds, desired dimensions, calculated scale, and migration state. No editing controls in DT1.

## Command UI — Attack (DV2)

The HUD exposes a single **Attack** command. Gameplay simulation is unchanged.

## Navigation debug overlays (NV0)

Dev Mode **Debug** toggles (master overlay must be on). Each category is independent — enable only what you need.

| Toggle | Shows |
|--------|--------|
| **Paths** | Active path polyline, start (blue) / end (red) markers, waypoint spheres (cyan; portal = magenta; active = yellow) |
| **Navigation/Pathing Mask** | Whole-world navigable (green) and blocked (reason-colored) 4 m navigation cells from authoritative passability; no selection required |
| **Blocked Area** | Blocked-only nav cells from **current movement authority** (blueprint boundaries vs doodad/terrain; not legacy footprint for blueprint buildings or ghosts). See [building-navigation-authority.md](building-navigation-authority.md). |
| **Nav footprints** | Building footprint outlines for **placement/diagnostic** context (not authoritative movement blocking when blueprint active). Highlighted when building selected. |
| **Nav entrances** | Portal rings + lines to destination (highlighted for selected building) |
| **Nav reservations** | Yellow occupancy cells reserved for construction |
| **Nav occupancy** | Red static blocked occupancy cells |
| **Nav blueprint** | Generated navigation blueprint polygons, entrances, and vertical transitions (NV1.2.5; from blueprint data, not mesh re-analysis) |

**Navigation/Pathing Mask** covers the full resident (loaded) navigation world. Specialized overlays (footprints, entrances, occupancy, reservations, blueprint) still draw near the camera, selected unit, or inspector-selected building/unit. They observe navigation and occupancy state only — pathfinding is unchanged when overlays are off.

**Inspector:** select a unit to see path status text (waypoints, index, length) and path highlight via overlay focus. Select a building to highlight its entrances and footprint.

### Building navigation blueprint editor (Slice 7)

Use the **Navigation Editor** window (not global hotkeys). Open from Selected Object or Catalog → Advanced → Editor when a placed building is selected.

| Control | Action |
|---------|--------|
| **Inspect** / **Edit** | Read-only vs editable session |
| **Floor − / +** | Cycle floors (sparse/negative IDs supported) |
| **Select / Add corner / Add entrance** | Walkable outline and entrance tools |
| **Delete** | Delete selection (also **Del** when editor focused, no text field) |
| **Save instance / Apply to asset / Reset / Save as variant** | Explicit persistence scopes with confirmation |
| **Regenerate…** | Mesh-based generation (`data-import` feature) |
| **Validate** | Run inspection validator |
| **Overlay blueprint / entrances / runtime path** | Toggle existing `DebugOverlayConfig` diagnostics |

Removed global shortcuts (Slice 7): **N**, **E** (edit), **`[`/`]`**, tool digits **1–3**, **Ctrl+S** family, **Shift+R**. **E** still toggles catalog enabled-only filter when no transform target.

Unsaved edits prompt before selection change, window close, or **F12** dev-mode off.

See `docs/navigation-blueprint.md` for validation rules and persistence semantics.

Requires `cargo run --features dev`.

## Panel layout

- Legacy panel content lives inside the **Catalog** dev window (368px width; draggable — see Dev windows above)
- **Windows → Catalog** launcher (top-left) reopens a closed Catalog window
- Long catalog labels truncate with ellipsis
- Search field shows placeholder when empty; green border when focused
- Future transparency option reserved (not implemented)
