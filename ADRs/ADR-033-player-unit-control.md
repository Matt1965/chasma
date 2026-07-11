# ADR-033: Player Unit Control (U8)

# Status

Accepted (U8 — SC2-style selection and move)

# Context

U5–U7 established authoritative unit orders, pathfinding, and movement on
[`WorldData`]. U3 established the unit runtime layer for glTF rendering. The
player needs StarCraft II-style unit interaction: single selection, move commands,
and a selection ring — without group select, box select, attack-move, or a command
card.

ADR-014 keeps camera input on middle-mouse rotate; left/right clicks are
available for gameplay. ARCHITECTURE places player-facing interaction in the
Gameplay layer, separate from world data and rendering authority.

# Decision

## Player layer (`src/player/`)

Introduce a Player layer registered by `PlayerPlugin` after
[`UnitsRuntimePlugin`] in `AppPlugin`:

| Concern | Owner |
|---------|--------|
| [`PlayerUnitSelection`] (client-local `Option<UnitId>`) | Player layer |
| Mouse pick rays, left/right click handling | Player layer |
| [`issue_unit_order`] / [`step_all_unit_movement`] calls | Player layer (thin tick) |
| Green selection ring ECS entity | Player layer (`sync_unit_selection_indicator`) |
| Authoritative placement, paths, state | [`WorldData`] |

Player systems run in [`PlayerControlSystems`], after [`UnitRuntimeSystems`], so
picks use current render transforms and selection visuals attach to render entities.

## Control model (SC2 baseline)

- **Left-click unit** — select (replace existing selection)
- **Left-click terrain** — clear selection
- **Right-click terrain** — `MoveTo` for selected unit; selection unchanged
- **Middle-mouse** — camera rotate (unchanged, ADR-014)
- No shift-modifier, box select, group keys, or attack commands in U8

## Picking

- Cursor ray via [`Camera::viewport_to_world`] on [`RtsCamera`]
- **Units:** ray-sphere test against visible [`UnitRenderEntity`] transforms;
  front-most hit wins; radius derived from catalog `collision_radius_meters` with
  a usability floor
- **Terrain:** ray cast against exaggerated heightfield surface in render space
  (respecting [`TerrainRenderAssets::vertical_scale`]) to recover clicked **X/Z only**;
  authoritative target via [`terrain_click_to_world_position`] →
  [`ground_world_position`] (heightfield Y, never render mesh Y)

## Selection indicator

A flat green [`Annulus`] mesh is spawned as a [`ChildOf`] the selected unit's
render entity, slightly above local Y=0. Despawned when selection clears or changes.

## Debug logging

[`PlayerInteractionSettings::debug_unit_interaction`] enables logs for render hit,
authoritative target, and generated path metrics (length, straight-line ratio).

## Movement tick

[`step_all_unit_movement`] runs each frame in the player layer so issued move
orders visibly advance without a separate simulation plugin (future layer may
subsume this tick).

# Consequences

**Benefits:**

- SC2-familiar controls with clear layer boundaries
- Orders remain authoritative on [`WorldData`]; selection is client-local
- Reuses existing navigation and movement from U5–U7

**Costs:**

- Picking reads render entities (acceptable for local client U8; future
  multiplayer may need replicated pick proxies or server-side validation)
- Terrain pick is ray-marched, not mesh-accurate (sufficient for authoritative
  heightfields)

# References

- ADR-014 (camera input boundaries)
- ADR-028 (unit runtime layer)
- ADR-030 (unit orders)
- ADR-032 (chunk grid navigation)
- ADR-069 (combat responsiveness — player commands resolve immediately)
- ARCHITECTURE Gameplay Layer

[`WorldData`]: ../src/world/data.rs
[`PlayerUnitSelection`]: ../src/player/selection.rs
[`issue_unit_order`]: ../src/world/unit/orders.rs
[`step_all_unit_movement`]: ../src/world/unit/movement.rs
[`UnitsRuntimePlugin`]: ../src/units/plugin.rs
[`UnitRuntimeSystems`]: ../src/units/sync.rs
[`PlayerControlSystems`]: ../src/player/plugin.rs
[`UnitRenderEntity`]: ../src/units/components.rs
[`RtsCamera`]: ../src/camera/components.rs
[`TerrainRenderAssets`]: ../src/terrain/spawn.rs
[`ground_world_position`]: ../src/world/terrain/query.rs
