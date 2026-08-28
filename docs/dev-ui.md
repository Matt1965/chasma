# Chasma Dev UI — graphical interface architecture

This document describes the **graphical, interactable** developer interface: windows, panels, widgets, input routing, and state ownership. It does **not** cover world-space debug gizmos/overlays (see `DebugOverlayConfig` / `DebugOverlayPlugin` in `src/debug/`).

**Before changing Dev UI:** read this file end-to-end.

---

## Terminology

| Term | Meaning |
|------|---------|
| **Dev mode** | F12 authoring mode; `DevModeState.enabled` |
| **Dev window** | Floating panel shell (`DevWindowId`) — Save, Catalog, Debug, World, etc. |
| **Catalog tab** | `DevTab` inside the **Catalog** window only (Units/Doodads/Buildings/Items) |
| **Backing state** | Resource fields (`DevModeState`, `DebugOverlayConfig`, etc.) |
| **Widget** | Bevy UI entity with `Button`, `Interaction`, marker components |
| **Sync system** | Update system that mutates widget visuals from backing state each frame |
| **Handler system** | Update system that mutates backing state from `Interaction::Pressed` |

**Critical distinction:** *state exists* ≠ *a clickable widget exists*. The Debug window deliberately shows **read-only summary text** that mirrors toggle state but is **not** interactive.

---

## 1. Root registration and schedules

### Plugin entry

| Item | Location |
|------|----------|
| Plugin | `src/dev/mod.rs` — `DevModePlugin` |
| Registered | `src/player/plugin.rs` — `#[cfg(feature = "dev")] app.add_plugins(DevModePlugin)` |
| Overlay plugin (separate) | `src/debug/plugin.rs` — `DebugOverlayPlugin` (world gizmos, not panel UI) |

Dev UI exists **only** with `--features dev`.

### System sets

| Set | Role |
|-----|------|
| `DevModeInputSystems` | Input, handlers, sync that affects interaction |
| `DevModePresentationSystems` | Gizmos, ghosts, non-blocking presentation |

### Startup chain (panel construction)

All run once in order (`src/dev/mod.rs`):

```
setup_dev_workspace          → window shells + launcher
setup_dev_panel              → Catalog window body content
setup_save_window_panel
setup_selected_object_panel
setup_navigation_editor_panel
setup_debug_window_panel     → Debug window body content
setup_world_window_panel
setup_fields_window_panel
setup_dev_tooltip
```

**Rule:** panel `setup_*` systems query `DevWindowBody` by `DevWindowId` and attach children. They run **after** `setup_dev_workspace` creates bodies.

### Update chain (representative)

```
DevModeInputSystems:
  reset_dev_input_gate
  handle_dev_window_pointer          → drag, close, launcher
  sync_dev_window_presentation       → visibility, z-order, collapse
  sync_debug_panel_content           → read-only summary text
  sync_debug_panel_button_styles     → toggle checkbox marks
  handle_debug_toggle_buttons        → toggle clicks
  sync_collapsible_sections          → show/hide section bodies
  handle_collapsible_toggles
  sync_dev_debug_controls            → DevModeState.debug_config → DebugOverlayConfig resource
  … panel-specific handlers …
```

---

## 2. Window system

### Authority

| Concern | Owner |
|---------|--------|
| Window IDs | `src/dev/window/id.rs` — `DevWindowId` |
| Open/closed, position, collapse, focus, drag | `src/dev/window/state.rs` — `DevWindowRegistry` |
| Shell spawn | `src/dev/window/setup.rs` — `setup_dev_workspace`, `spawn_*_window` |
| Pointer/drag/close | `src/dev/window/systems.rs` |
| Components | `src/dev/window/components.rs` |

### Window lifecycle

```
Launcher button / API registry.show(id)
  → DevWindowRegistry.session(id).visible = true
  → sync_dev_window_presentation sets DevWindowRoot Visibility + position + ZIndex

Startup (once):
  spawn DevWindowRoot + title bar + DevWindowBody (empty)
  → setup_*_panel attaches panel root as child of body

Close (×):
  → registry.hide(id) — state preserved, entities kept

Collapse (−):
  → registry.session(id).collapsed — body hidden, title bar remains
```

### Windows

| `DevWindowId` | Panel setup | Default visible |
|---------------|-------------|-----------------|
| Save | `setup_save_window_panel` | yes |
| Catalog | `setup_dev_panel` | yes |
| Selected Object | `setup_selected_object_panel` | yes |
| Navigation Editor | `setup_navigation_editor_panel` | no |
| Debug | `setup_debug_window_panel` | no |
| World | `setup_world_window_panel` | no |
| Fields | `setup_fields_window_panel` | no |

Launcher: **Windows** group (Save/Catalog/Selected Object) and **Advanced** group (Debug/World/Fields/Nav Editor).

### Z-order / focus

- Focus raises `ZIndex` on `DevWindowRoot` (`DevWindowRegistry.focus_window`).
- Clicking panel UI calls `focus_dev_window_on_panel_press` / title bar drag region.

---

## 3. Panel patterns

Each major window follows the same shape:

1. **`setup_*_panel`** (Startup) — spawn static widget tree under `DevWindowBody`.
2. **`sync_*_content` / `sync_*_panel`** (Update) — refresh text, visibility, values from resources.
3. **`handle_*`** (Update, `Changed<Interaction>`) — apply clicks to backing state.
4. **`sync_*_button_styles` / `sync_*_toggles`** (Update) — checkbox marks, button chrome.

### Debug window (`src/dev/debug_window/panel.rs`)

**Two separate UI layers:**

1. **Read-only summary** — `DevDebugSummaryText`, updated by `sync_debug_panel_content` → `format_debug_summary()`. Shows lines like `Overlay master: false`, `Combat: false`. **Not clickable.**
2. **Interactive toggles** — `TOGGLE_GROUPS` → `spawn_collapsible_section` → `spawn_toggle_row` → `DevDebugToggleButton { flag }`. **These are the real controls** (18×18 px checkbox + label).

Handlers: `handle_debug_toggle_buttons` → `toggle_debug_flag` → mutates `DevModeState.debug_config`.

Style sync: `sync_debug_panel_button_styles` → `sync_toggle_styles_with_marker`.

Overlay sync (separate): `sync_dev_debug_controls` copies `debug_config` → `DebugOverlayConfig` resource for gizmo systems.

### World window (`src/dev/world_environment/`)

- Setup: `spawn_environment_controls` in `world_window/panel.rs`
- Toggles: `DevWorldCycleToggle`, `DevWorldWaterEnabledToggle`, etc.
- Sliders: `spawn_bounded_slider_row` + `handle_world_slider_interaction`
- Handlers in `world_environment/systems.rs`

### Save window (`src/dev/save_window/panel.rs`)

- Buttons: `DevSaveSceneButton { action }`
- Text field: `DevSaveSceneNameField` + keyboard focus via `DevTextFieldFocus`
- Handler: `handle_save_window_interaction`

### Catalog window (`src/dev/panel.rs`)

- Tabs: `DevTabButton` (only visible tabs: Units/Doodads/Buildings/Items)
- Search: text field with focus handling
- List rows: `DevCatalogRowButton`
- Handler: `handle_dev_panel_ui_interaction`

**Note:** `DevTab::Debug`, `DevTab::WorldTools`, etc. exist in the enum but are **hidden from Catalog tabs** (`visible_tabs()` in `catalog/state.rs`). Debug/World/Fields moved to **separate floating windows**.

---

## 4. Widget library

Location: `src/dev/widgets/`

| Widget | Module | Spawn API | Interaction |
|--------|--------|-----------|-------------|
| Toggle/checkbox | `toggle.rs` | `spawn_toggle_row(parent, label, tooltip, marker)` | `Button` + `DevWidgetToggle` + domain marker; mark via `DevWidgetToggleMark` |
| Action button | `button.rs` | `spawn_action_button` | `Button` + `DevWidgetActionButton` + `DevButtonChrome` |
| Stepper | `button.rs` | `spawn_stepper_button`, `spawn_labeled_stepper_row` | `Button` |
| Slider | `slider.rs` | `spawn_bounded_slider_row` | `DevWidgetSliderTrack` drag + `DevWidgetSliderValue` |
| Collapsible section | `section.rs` | `spawn_collapsible_section` | `DevCollapsibleToggleButton` + `DevCollapsibleBody` |
| Search field styling | `search.rs` | constants only; fields spawned inline in panels | `Button` wrapper + text child |
| Numeric | `numeric.rs` | parse/format helpers | used with sliders and World numeric fields |
| Tooltip | `tooltip/` | `DevTooltipTarget` on entities | `sync_dev_tooltip_presentation` |
| Theme | `theme.rs` | colors, fonts, `toggle_button_bg`, etc. | — |

### Toggle visual model

- Row = small square `Button` (18px) + text label.
- Checked state = inner `DevWidgetToggleMark` visibility + border/background via `sync_toggle_styles_with_marker`.
- **Not** Unicode checkbox characters (see `widgets/toggle.rs` tests).

### Required markers on interactive widgets

- `DevPanelUi` — marks dev panel entities for hover/input gating
- `DevWindowUi` — marks dev window entities for focus/drag routing
- Domain marker — e.g. `DevDebugToggleButton`, `DevWorldCycleToggle`

---

## 5. Input routing

```
Pointer event
  → Bevy UI Interaction on Button/field
  → Handler system (Changed<Interaction>, Pressed)
  → Mutate backing resource (DevModeState, etc.)
  → gate.block_gameplay_mouse = true

Same frame / next frame:
  → Sync system reads backing state
  → Updates Text, Visibility, BackgroundColor, toggle marks
```

### Input gate

`DevModeInputGate` (`src/dev/input.rs`):

- Reset each frame in `reset_dev_input_gate`
- Set by UI handlers and `DevWindowInteractionState` (`apply_dev_window_input_gate`)
- Blocks gameplay mouse, camera drag, scroll when over dev UI

### Text focus

`DevTextFieldFocus` on `DevModeState` — while set, global dev shortcuts suppressed (`dev_shortcuts_suppressed`).

### Handler registration

All handlers must be added in `DevModePlugin::build` under `DevModeInputSystems`. Missing registration = visible button that does nothing (**failure class H**).

---

## 6. State ownership

| State | Authoritative resource | Consumed by |
|-------|------------------------|-------------|
| Dev mode on/off | `DevModeState.enabled` | All panels via `registry.window_active(enabled, id)` |
| Debug overlay flags | `DevModeState.debug_config` (`DebugOverlayConfig`) | Debug panel handlers; synced to `DebugOverlayConfig` resource |
| Window open/visible | `DevWindowRegistry` | `sync_dev_window_presentation` |
| Window position/collapse | `DevWindowRegistry` sessions | drag/collapse handlers |
| Collapsible section expanded | `DevCollapsibleState` | `sync_collapsible_sections` |
| Catalog tab | `DevModeState.active_tab` | Catalog panel only |
| World environment draft | `WorldEnvironmentUiState` + ECS env resources | World panel |
| Tooltip hover | `DevTooltipState` | tooltip presentation |

### Debug toggle persistence

- Toggle values live on `DevModeState.debug_config` — **not** on window visibility.
- Closing Debug window does **not** reset flags.
- Scene save may snapshot some flags via `SceneDebugFlagsSnapshot` (partial; not all overlay fields).

---

## 7. UI construction model

**Retained mode:** widgets spawned once at Startup; Update systems sync values and styles.

**Not rebuilt** each frame. Adding a control requires editing the Startup `setup_*` spawn tree.

**Dynamic parts:** list rows (catalog, save scenes), text content, collapsible visibility, scroll position.

---

## 8. Failure modes (derived from architecture)

| Class | Description | Example |
|-------|-------------|---------|
| **A — Backing state without widget** | Field + handler + tests exist; `ToggleDef` or spawn missing | Enum match arm added, not `TOGGLE_GROUPS` |
| **B — Widget without handler** | Button spawned; no `handle_*` or missing match arm | — |
| **C — Handler without style sync** | Clicks work; checkbox mark never updates | Missing `sync_*` match arm |
| **D — Parallel/non-live path** | Code updated in tests/helpers, not `setup_*` | — |
| **E — Tests prove state, not UI** | Unit tests on `DebugOverlayConfig` only | Relationship Links tests |
| **F — Summary mistaken for controls** | Read-only text looks like toggle list | Debug `format_debug_summary` block |
| **G — Input gate / inactive window** | Handler returns early when `!window_active` | — |
| **H — Handler not registered** | System missing from `DevModePlugin` | — |
| **I — Layout/clipping** | Widget exists but scrolled/collapsed off-screen | Collapsed `DevCollapsibleBody` |
| **J — Summary drift** | New flag omitted from `format_debug_summary` | Relationship Links absent from summary text |
| **K — Stale binary** | Source updated; runtime not rebuilt | — |

---

## 9. Relationship Links failure — root cause (local worktree)

**Reported:** "Relationship Links toggle added"; runtime Debug window showed no such control.

**What was actually changed (local diff):**

- `DebugOverlayConfig.relationship_links` field
- `DevDebugToggleFlag::RelationshipLinks` enum variant
- `TOGGLE_GROUPS` entry (label "Relationship Links")
- `sync_debug_panel_button_styles` + `toggle_debug_flag` match arms
- State-only unit tests
- Overlay/gizmo code (out of Dev UI scope)

**What the live Debug window shows at the top:**

- `format_debug_summary()` — read-only text (`Combat: false`, etc.). **Does not include Relationship Links** even after the feature was added (**class J**).

**Why the report felt false:**

1. **Class F/E/J combined:** The prominent summary block looks like the entire Debug UI but is not interactive and was not updated for the new flag. Users (correctly) see no new control in the visible text area.
2. **Actual toggles** are separate 18px checkboxes inside collapsible sections below the summary, with different labels ("Combat overlay" vs summary's `Combat:`). Easy to miss if scrolling, collapsed, or mistaken for non-interactive text.
3. **Tests did not verify** that `setup_debug_window_panel` produces one `DevDebugToggleButton` per `ToggleDef` (**class E**).
4. **Possible stale binary** if `cargo run --features dev` was not rebuilt after panel changes (**class K**).

**Precise root cause:** Implementation validated **backing state and overlay wiring**, not **live widget construction + user-visible confirmation**. The Debug window's read-only summary exacerbates confusion by duplicating flag names without providing interaction or updating for new flags.

---

## 10. Known-good interactable traces

### A. Debug window toggle — "Combat overlay"

```
Startup: setup_debug_window_panel
  → TOGGLE_GROUPS → spawn_toggle_row(..., DevDebugToggleButton { flag: Combat })

Widget: Button + DevWidgetToggle + DevDebugToggleButton + DevWidgetToggleMark

Press: handle_debug_toggle_buttons
  → toggle_debug_flag(Combat) → dev_state.debug_config.combat ^= true

Sync: sync_debug_panel_button_styles
  → sync_toggle_styles_with_marker(|t| config.combat, ...)

Overlay: sync_dev_debug_controls → DebugOverlayConfig resource
```

### B. Catalog tab button

```
Startup: setup_dev_panel → DevTabButton { tab: Units }

Press: handle_dev_panel_ui_interaction → dev_state.active_tab = tab

Sync: sync_dev_panel_button_styles → menu_button_bg(..., active_tab == tab)
       sync_dev_catalog_chrome → row visibility
```

### C. World — "Cycle enabled" toggle

```
Startup: spawn_environment_controls → spawn_toggle_row(..., DevWorldCycleToggle)

Press: handle_world_cycle_toggles

Sync: sync_world_environment_toggles
```

### D. Save — "Save Current World" button

```
Startup: setup_save_window_panel → DevSaveSceneButton { action: SaveCurrent }

Press: handle_save_window_interaction → save_current_world(...)

Sync: sync_save_window_content (list/status refresh)
```

### E. Catalog search field

```
Startup: setup_dev_panel → search Button + text child

Focus: handle_dev_panel_ui_interaction → DevTextFieldFocus::CatalogSearch

Sync: sync_dev_search_box_style, sync_dev_panel_content (filtered list)
Keyboard: dev_mode_keyboard_input routes typing when focus set
```

---

## 11. Canonical recipes

### RECIPE A — Add a boolean toggle to an existing window

**Example: Debug window (most common for overlay flags)**

1. **Backing state** — add field to `DebugOverlayConfig` in `src/debug/settings.rs` (+ `production()` / `development()` defaults).
2. **Panel enum** — add variant to `DevDebugToggleFlag` in `src/dev/debug_window/panel.rs`.
3. **Visual spawn** — add `ToggleDef { label, flag, tooltip }` to `TOGGLE_GROUPS` in the correct section.
4. **Style sync** — add arm in `sync_debug_panel_button_styles` closure passed to `sync_toggle_styles_with_marker`.
5. **Click handler** — add arm in `toggle_debug_flag`.
6. **Summary (optional but recommended)** — update `format_debug_summary` if the flag should appear in the read-only header block.
7. **Overlay sync** — if it gates gizmos, add `DebugOverlayCategory` + run_if helper in `src/debug/settings.rs` and register overlay system in `src/debug/plugin.rs`.
8. **Dev→overlay copy** — automatic via existing `sync_dev_debug_controls` (copies whole `debug_config`).
9. **Handler registered** — `handle_debug_toggle_buttons` already in `DevModePlugin` (verify not removed).
10. **Tests** — state default + handler logic (level 1); widget count invariant (level 2, recommended).
11. **Manual** — open Debug window, expand section, click 18px box, verify mark + overlay behavior.

**Skeleton (Debug window):**

```rust
// settings.rs
pub relationship_links: bool,  // in struct + production() default false

// panel.rs — enum
RelationshipLinks,

// TOGGLE_GROUPS
ToggleDef { label: "Relationship Links", flag: DevDebugToggleFlag::RelationshipLinks, tooltip: "..." },

// sync_debug_panel_button_styles
DevDebugToggleFlag::RelationshipLinks => config.relationship_links,

// toggle_debug_flag
DevDebugToggleFlag::RelationshipLinks => state.debug_config.relationship_links = !state.debug_config.relationship_links,
```

**Other windows:** use domain marker component (e.g. `DevWorldCycleToggle`) + `spawn_toggle_row` in that window's setup + dedicated handler/sync pair in that module.

---

### RECIPE B — Add a button

1. Spawn with `spawn_action_button` or manual `Button` bundle + domain marker (e.g. `DevSaveSceneButton`).
2. Add `DevPanelUi` + `DevWindowUi` + optional `DevTooltipTarget`.
3. Handler: `Query<(&Interaction, &Marker), Changed<Interaction>>`, check `Pressed`.
4. Register handler in `DevModePlugin` if new.
5. Sync: update status text or list in dedicated `sync_*` system.
6. Manual: click and verify action + visual feedback (`DevButtonChrome` flash).

---

### RECIPE C — Add display-only text

1. Startup: spawn `Text` + marker component (e.g. `DevDebugSummaryText`).
2. Update: `Query<&mut Text, With<Marker>>` in `sync_*` system.
3. Gate on `registry.window_active(dev_state.enabled, window_id)`.
4. Manual: verify text updates when backing state changes.

---

### RECIPE D — Add a slider

**Canonical sliders exist** — `spawn_bounded_slider_row` in `src/dev/widgets/slider.rs`.

1. Assign `field_id: u32` (see `world_environment/fields.rs` patterns).
2. Spawn row in panel setup.
3. Handler: `handle_world_slider_interaction` pattern — drag on `DevWidgetSliderTrack`.
4. Sync: `sync_world_environment_sliders` — fill width + numeric label.
5. Use `DevSliderDragState` to block camera while dragging.

---

### RECIPE E — Add text input / search

1. Spawn `Button` wrapper (focus target) + `Text` child for content.
2. On press: set `DevModeState.text_focus = DevTextFieldFocus::...`.
3. Route keys in `dev_mode_keyboard_input` or dedicated handler.
4. Sync style: focused vs idle border/background (see `sync_dev_search_box_style`, `sync_save_window_name_field_style`).
5. Manual: click field, type, verify focus suppresses shortcuts.

---

### RECIPE F — Add a section to an existing window

1. Add `DevCollapsibleSectionId` variant in `widgets/section.rs`.
2. `spawn_collapsible_section(root, id, title, tooltip, |body| { ... })` in panel setup.
3. Collapse state automatic via `DevCollapsibleState` + `sync_collapsible_sections`.
4. Default expanded: implement `default_expanded()` for new id.

---

### RECIPE G — Add a new Dev window (reference only)

1. Add `DevWindowId` variant + title/launcher label (`window/id.rs`).
2. Session defaults in `DevWindowRegistry::default` (`window/state.rs`).
3. `spawn_*_window` in `setup_dev_workspace` (`window/setup.rs`).
4. `setup_*_panel` querying new body; register in Startup chain (`dev/mod.rs`).
5. Visibility sync automatic via `sync_dev_window_presentation`.
6. Launcher button in `ADVANCED_LAUNCHER` or `WINDOWS_LAUNCHER` array.
7. Handlers/sync systems + register in `DevModePlugin`.
8. Optional: `default_*_position` in `window/math.rs`.

---

## 12. Hypothetical walkthrough — "Example Overlay" toggle (Debug window)

**Do not implement** — file checklist only.

| Step | File | Change |
|------|------|--------|
| 1 | `src/debug/settings.rs` | `example_overlay: bool`, default false, `DebugOverlayCategory::ExampleOverlay`, run_if |
| 2 | `src/debug/plugin.rs` | Register overlay draw system with run_if |
| 3 | `src/dev/debug_window/panel.rs` | `DevDebugToggleFlag::ExampleOverlay` |
| 4 | same | `ToggleDef` in appropriate `TOGGLE_GROUPS` section |
| 5 | same | `sync_debug_panel_button_styles` arm |
| 6 | same | `toggle_debug_flag` arm |
| 7 | same | (recommended) `format_debug_summary` token |
| 8 | `src/dev/debug_window/tests.rs` | default off test |
| 9 | (recommended) new test | `ToggleDef` count == spawned toggle button count |

**Scope:** ~2–4 files, ~15–30 logical lines, 1–2 unit tests, **mandatory manual click verification**.

If overlay draw system is also required, add `src/debug/overlay/` + plugin registration (~2 more files).

---

## 13. Testing and definition of done

| Level | What it proves | Cannot prove |
|-------|----------------|--------------|
| **1 — Logic/state** | Defaults, toggle fn mutates resource | Widget exists or is clickable |
| **2 — Construction** | Entity counts, markers present after Startup | Visual appearance, hit targets |
| **3 — Interaction wiring** | Handler fires on synthetic `Interaction` change | Layout, z-order, clipping |
| **4 — Compile/integration** | Plugin registers systems | Runtime with human eyes |
| **5 — Manual runtime** | User sees control and clicks it | — |

### Rules

- **Never report runtime PASS** for "checkbox appears and works" based on levels 1–4 alone.
- If Composer cannot click the live control: report **MANUAL RUNTIME VERIFICATION PENDING**.
- Recommended invariant test for Debug toggles:

```text
count(DevDebugToggleButton) == count(all ToggleDef in TOGGLE_GROUPS)
```

- After adding a Debug flag, check **both** the collapsible toggle row **and** whether `format_debug_summary` should mention it.

---

## 14. Architecture complexity findings

| Finding | Severity | Notes |
|---------|----------|-------|
| Debug summary vs toggle rows duplicate information | **Actively dangerous** | Summary looks interactive; uses different labels; drifts from `ToggleDef` |
| `DevTab::Debug` vs `DevWindowId::Debug` | Confusing but documented | Catalog tab hidden; Debug is its own window |
| `DevDebugFlags` alias of `DebugOverlayConfig` | Harmless | Same struct, two names |
| `debug_config` copied to `DebugOverlayConfig` resource | Learn it | Two copies synced each frame; overlay reads resource |
| Dual namespaces `dev/` vs `debug/` | Learn it | `dev/` = panel UI; `debug/` = overlay/gizmos + shared config type |
| Many sync systems per frame | Harmless | Predictable once mapped |
| 18px toggle boxes easy to miss | Confusing | Visually subtle vs World window checkboxes in dense lists |

**Do not refactor during feature work** unless explicitly approved.

---

## 15. File index (quick reference)

| Area | Primary files |
|------|----------------|
| Plugin / schedules | `src/dev/mod.rs`, `src/player/plugin.rs` |
| Dev state | `src/dev/dev_mode.rs` |
| Windows | `src/dev/window/{id,state,setup,systems,components,math}.rs` |
| Debug panel | `src/dev/debug_window/panel.rs` |
| Catalog panel | `src/dev/panel.rs`, `src/dev/catalog/` |
| Save panel | `src/dev/save_window/panel.rs` |
| World panel | `src/dev/world_window/panel.rs`, `src/dev/world_environment/` |
| Fields panel | `src/dev/fields_window/panel.rs` |
| Selected Object | `src/dev/selected_object/panel.rs` |
| Navigation Editor | `src/dev/navigation_editor/panel.rs` |
| Widgets | `src/dev/widgets/*.rs` |
| Tooltips | `src/dev/tooltip/` |
| Input gate | `src/dev/input.rs` |
| Overlay config type | `src/debug/settings.rs` |
| Overlay→dev sync | `src/dev/debug_controls.rs` |

---

## 16. Checklist — "Add one toggle to Debug window"

- [ ] Field on `DebugOverlayConfig` + defaults
- [ ] `DevDebugToggleFlag` variant
- [ ] `ToggleDef` in `TOGGLE_GROUPS`
- [ ] `sync_debug_panel_button_styles` arm
- [ ] `toggle_debug_flag` arm
- [ ] Update `format_debug_summary` (if users read summary block)
- [ ] Overlay category + system if needed (`debug/` not `dev/`)
- [ ] State unit test (default off)
- [ ] Widget count test (recommended)
- [ ] **Manual:** F12 → Advanced launcher → Debug → expand section → click checkbox → verify mark + behavior
- [ ] Report manual status honestly in PR/task notes
