# Dev Mode (F12)

Runtime authoring UI for spawning units/doodads, scenes, inspector, and debug overlays.
See ADR-043, ADR-044, ADR-047, and DV2 usability refresh.

## Toggle

| Key | Action |
|-----|--------|
| **F12** | Toggle dev mode on/off |

## Keyboard focus (DV2)

Dev Mode uses explicit text-field focus. Global shortcuts only fire when **no** text field is focused.

| Key | Action |
|-----|--------|
| **/** or **Ctrl+F** | Focus search / scene name field |
| **Esc** | Clear focus; also cancels armed placement tool |
| **Enter** | Exit search focus (does not trap focus) |
| Click search box | Focus field |
| Click elsewhere in panel | Remove focus |
| Click terrain | Remove focus |

While search is **focused**, letter keys type into the field (including **T**).
While search is **unfocused**, **T** cycles spawn team (Player ↔ Wilds).

## Tool cancellation (DV2)

| Input | Action |
|-------|--------|
| **Esc** | Cancel placement selection, clear preview ghosts, clear search focus |
| **Right-click** (world, not over UI) | Cancel placement tool; dev mode stays active |

Cancellation does **not** clear RTS unit selection.

## Catalog shortcuts (unfocused)

| Key | Action |
|-----|--------|
| **Tab** | Cycle panel tabs |
| **E** | Toggle enabled-only filter |
| **T** | Cycle spawn team |
| **F** | Toggle favorite on selected definition |
| **1–9** | Recall favorite slot |
| **Ctrl+1–9** | Assign favorite slot |

## Placement

1. Select a definition on **Units** or **Doodads** tab.
2. Optional: configure brush on **Placement** tab.
3. **Left-click** terrain to spawn.
4. **Shift+click** — larger batch count.
5. **Ctrl+click** — repeat last spawn.

The **Tool** status block (below tabs) shows active tool, selection, team, and brush mode live.

## Command UI — Attack (DV2)

The HUD exposes a single **Attack** command (no separate Attack Move button).

When Attack is armed:

| Target | Result |
|--------|--------|
| Enemy unit | Direct attack |
| Ground | Attack-move |
| Friendly unit | Existing move/interaction rules |

Gameplay simulation is unchanged; only presentation simplified.

## Items tab shortcuts

| Key | Action |
|-----|--------|
| **I** | Items subtab (definitions) |
| **P** | Inventory profiles subtab |
| **H** | Inventory harness subtab (ADR-088 I2) |

### Inventory harness (H subtab)

Detached inventories only — no unit/building attachment in I2.

| Key | Action |
|-----|--------|
| **N** | Create detached inventory (selected profile, or `unit_backpack_standard`) |
| **A** | Add stack of selected item (qty 5, first-fit) |
| **U** | Create and place unique instance of selected item |
| **H** | Split half of entry 0 |
| **M** | Merge entry 1 into entry 0 |
| **O** | Auto-sort |
| **V** | Validate inventory and store invariants |
| **Del** / **Backspace** | Delete detached test inventory |

## World Tools — piles and treasuries (ADR-090, ADR-093, ADR-094)

| Key | Action |
|-----|--------|
| **V** | Run `validate_world_inventory_state` (full I1–I7 inventory audit) |
| **P/D/O/H/G/L** | Item pile harness (see ADR-090) |
| **C/Y/E/B/J** | Treasury harness (see ADR-093) |

Scene save/load (v7) round-trips inventories, instances, corpses, piles, and treasuries.

## Player inventory UI (I6 — ADR-092)

Separate from the F12 Items harness. Toggle with **I** on the gameplay HUD (primary selected unit).
Interact command opens containers, corpses, and world piles when armed.

Dev inspection of open inventory IDs and authoritative entries is via building inspector (I/G) and
pile harness (World Tools); player UI state lives in `InventoryUiState` (read-only in future dev overlay).

## Panel layout

- Panel width: 368px (top-right)
- Long catalog labels truncate with ellipsis
- Search field shows placeholder when empty; green border when focused
- Future transparency option reserved (not implemented)
