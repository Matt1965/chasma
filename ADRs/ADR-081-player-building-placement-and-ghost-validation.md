# ADR-081: Player Building Placement and Ghost Validation (B4)

# Status

Accepted (B4 — player build mode, ghosts, placement validation)

# Context

B3 established generalized footprints, occupancy, and passability. Players still
could not place buildings through gameplay UI — only Dev Mode instant spawn existed.

B4 adds the first player-facing placement workflow without construction workers,
resource consumption, spaces/portals, or terrain editing.

# Decision

## Client-local Build Mode state

[`BuildModeState`] lives in the client/UI layer, **not** on [`WorldData`].

Phases:

| Phase | Behavior |
|-------|----------|
| `Inactive` | Normal gameplay |
| `CatalogOpen` | Build panel visible |
| `GhostPlacing` | Armed definition + rotation + live validation |

## Input flow (ADR-038)

```
Input → ClientIntent → dispatch → validate → place_player_building → WorldData → sync
```

Never: UI → direct `WorldData` insert. Never: ghost entity → authoritative truth.

### Controls

| Key | Action |
|-----|--------|
| `B` | Enter/exit Build Mode |
| `Esc` | Cancel ghost; else exit Build Mode |
| Right-click (world) | Cancel ghost |
| `R` | Rotate 90° (quantized) |
| Left-click (valid) | `ClientIntent::PlaceBuilding` |

Search field focus suppresses build shortcuts. Dev Mode text focus suppresses `B`.

While ghost placing, normal move/attack world clicks are suppressed; unit selection
is preserved.

## Placement validation

[`validate_building_placement`] in `src/world/building/placement_validation.rs` is a
**pure** API (no `WorldData` mutation).

Checks per footprint support cell:

- definition/footprint enabled
- quantized rotation
- terrain resident + grounded
- per-cell slope ≤ `max_slope_degrees`
- height variation ≤ `max_height_variation_meters` (default 2 m)
- static occupancy (buildings + doodads)
- unit overlap (authoritative positions/radii)

Uses B3 passability/occupancy helpers — no parallel obstacle logic.

### Anchor policy

- Global XZ snaps to **2 m occupancy grid**
- Y from authoritative heightfield (`ground_world_position`)
- No silent Y=0 when terrain missing

## Commit pipeline

`ClientIntent::PlaceBuilding` → dispatch reruns validation → [`place_player_building`]:

- `BuildingLifecycleState::Planned`
- `BuildingSource::Authored`
- Player `owner_id` / `team_id` / `Affiliation::Player`
- Atomic occupancy registration (`OccupancyState::Reserved`)
- Rollback record if occupancy fails

## Occupancy policy (B4)

| Lifecycle | Movement blocks | Placement blocks |
|-----------|-----------------|------------------|
| `Planned` | No (walkable) | Yes (`Reserved`) |
| `Complete` | Yes (`Blocked`) | Yes |

B5 construction will transition Planned → UnderConstruction → Complete.

## Ghost presentation

Client-local gizmos (`draw_build_mode_ghost`):

- Footprint cell outline (required)
- Color: valid=green, static=red, units=amber, terrain=red, unavailable=gray
- Status text in catalog panel (reason label)

Planned buildings render as translucent placeholder cuboids via runtime sync.

## Dev Mode distinction

Dev spawn remains separate (`BuildingSource::Dev`, typically `Complete`).
Player Build Mode always validates and creates `Planned` records.

# Consequences

- Players can preview and commit buildings through HUD + intents
- Placement and movement share occupancy authority from B3
- Construction simulation (B5) can attach to `Planned` lifecycle seam

# References

- ADR-038, ADR-040, ADR-050 (client intent + HUD)
- ADR-079, ADR-080 (building runtime + occupancy)
- ADR-043/044 (Dev Mode — unchanged spawn path)

[`BuildModeState`]: ../src/ui/gameplay/build_mode/state.rs
[`validate_building_placement`]: ../src/world/building/placement_validation.rs
[`place_player_building`]: ../src/world/building/authoring.rs
[`WorldData`]: ../src/world/data.rs
