# ADR-034: Multi-Unit Selection System (U9)

# Status

Accepted (U9 — SC2-style multi-unit selection)

# Context

U8 introduced single-unit selection, move commands, and a selection ring via the
Player layer ([`ADR-033`]). U10 will refine move distribution across groups; U9
adds SC2-style multi-select without formations, group behavior, or UI panels.

Selection must remain client-local runtime state. [`WorldData`] continues to own
authoritative unit placement, orders, and paths.

# Decision

## Module ownership (`src/units/input/`)

| File | Responsibility |
|------|----------------|
| [`selection.rs`](../src/units/input/selection.rs) | [`SelectedUnits`] (`HashSet<UnitId>`) |
| [`picking.rs`](../src/units/input/picking.rs) | Ray pick, [`world_position_to_screen`] |
| [`box_select.rs`](../src/units/input/box_select.rs) | Marquee drag state and screen-rect tests |
| [`commands.rs`](../src/units/input/commands.rs) | Multi-unit [`issue_unit_order`] dispatch |
| [`mod.rs`](../src/units/input/mod.rs) | [`handle_unit_selection_input`] system |

[`PlayerPlugin`] wires input + [`sync_unit_selection_indicators`] after
[`UnitRuntimeSystems`] so picks use current render transforms.

## Selection model

- **Runtime-only:** [`SelectedUnits`] is a Bevy `Resource`; never written to
  [`WorldData`].
- **Multi-unit:** `HashSet<UnitId>` — no ordering, no duplicates.
- **Persistence:** Selection survives movement and path updates; invalid ids are
  pruned when units are removed from world data.

## SC2 input rules

| Action | Behavior |
|--------|----------|
| Left click unit | Replace selection |
| Shift + left click unit | Toggle unit in set |
| Left click terrain | Clear selection (no shift) |
| Left drag (marquee) | Select units in screen rect |
| Shift + marquee release | Add units in rect to selection |
| Right click terrain | `MoveTo` for **all** selected units |

Left-button actions commit on **release** so click vs drag is distinguished
([`BOX_SELECT_DRAG_THRESHOLD_PX`]).

## Picking and box select

- **Click pick:** Ray-sphere against [`UnitRenderEntity`] transforms (front-most
  hit). Uses catalog collision radius with usability floor.
- **Box select:** Project each visible unit render position with
  [`Camera::world_to_viewport`]. Units inside the normalized screen rectangle are
  selected. Iterates render entities only — no full-world scan.
- **Terrain move target:** Unchanged from U8 — render ray for X/Z,
  [`ground_world_position`] for authoritative Y ([`terrain_click_to_world_position`]).

## Selection indicators

Each selected unit gets its own green [`Annulus`] ring parented to that unit's
render entity ([`UnitSelectionIndicator`]). Indicators despawn when deselected or
when the unit leaves the selection set.

## Command dispatch

[`issue_move_orders_to_selection`] loops selected ids and calls
[`issue_unit_order`] per unit. No direct [`UnitState`] mutation, no pathfinding
or movement changes in U9.

# Consequences

**Benefits:**

- SC2-familiar multi-select with clear separation from simulation authority
- Reuses U5–U7 order and movement pipeline unchanged
- Box select scales with visible render entities

**Costs:**

- Picking and box select depend on render entity transforms (acceptable for
  local client U9)
- Per-unit selection rings increase ECS entity count linearly with selection size

**Deferred (U10+):**

- Formation-aware move distribution
- Group hotkeys, control groups, command card
- Attack-move, abilities

# References

- ADR-033 (U8 player unit control)
- ADR-028 (unit runtime layer)
- ADR-030 (unit orders)
- ADR-032 (navigation)

[`ADR-033`]: ADR-033-player-unit-control.md
[`WorldData`]: ../src/world/data.rs
[`SelectedUnits`]: ../src/units/input/selection.rs
[`world_position_to_screen`]: ../src/units/input/picking.rs
[`issue_unit_order`]: ../src/world/unit/orders.rs
[`handle_unit_selection_input`]: ../src/units/input/mod.rs
[`PlayerPlugin`]: ../src/player/plugin.rs
[`sync_unit_selection_indicators`]: ../src/player/indicator.rs
[`UnitRuntimeSystems`]: ../src/units/sync.rs
[`BOX_SELECT_DRAG_THRESHOLD_PX`]: ../src/units/input/box_select.rs
[`UnitRenderEntity`]: ../src/units/components.rs
[`terrain_click_to_world_position`]: ../src/units/input/terrain_click.rs
[`ground_world_position`]: ../src/world/terrain/query.rs
[`UnitSelectionIndicator`]: ../src/units/components.rs
[`issue_move_orders_to_selection`]: ../src/units/input/commands.rs
[`UnitState`]: ../src/world/unit/state.rs
