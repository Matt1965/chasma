# ADR-083: Navigable Spaces, Portals, and Automatic Interior Visibility

## Status

Accepted (B6, July 2026)

## Context

B3 introduced grid occupancy and passability. B4/B5 introduced buildings with lifecycle and
occupancy-by-state. Navigation (ADR-032) was surface-only with `WorldPosition` waypoints.

B6 generalizes navigation and presentation around **Spaces** connected by **Portals**, replacing
rigid floor assumptions while keeping grid A* and `WorldPosition` authority.

## Decision

### Space vs floor

- `SpaceId` is navigation truth (`SpaceId::SURFACE = 0` is canonical exterior).
- `display_floor_label`, `reference_elevation`, and `visibility_group_id` are presentation metadata.
- Numeric floor labels do not imply vertical ordering for pathfinding or visibility.

### Unit space authority

`UnitRecord.current_space_id` on `WorldData` is authoritative. Rendering and client view read it;
they never infer space from render Y alone.

### Space registry

`SpaceRegistry` on `WorldData` holds `SpaceRecord` and `PortalRecord` graphs, building-space
registration, and deterministic `space_route()` BFS for cross-space planning.

### Portals and stair transitions

`PortalRecord` connects `from_space` → `to_space` with a global XZ transition region and
`to_position`. `try_portal_transition` applies directional entry with per-unit lockout
(`UnitPortalTransitionState`) to prevent oscillation at thresholds.

Space changes occur when the unit enters the portal region (simulation tick), not from camera
proximity or animation timing.

### Cross-space pathfinding

`NavigationWaypoint` extends paths with `space_id` and optional `portal_id`.
`find_path_with_spaces` stitches single-space A* segments along `space_route()` portal chains.
Grid A* is unchanged; portal edges connect `(space_id, cell)` nodes across spaces.

### Passability and grounding

`query_passability_in_space` routes interior queries through occupancy for that `space_id` and
skips terrain slope. `sample_support_height` / `ground_position_in_space` use terrain heightfield
for surface and authored `floor_y_global` for interior spaces.

### Automatic interior visibility

Client-local `ActiveViewedSpace` follows the **primary selected unit** (lowest `UnitId` when
multi-selected). `ViewFollowLock` optionally prevents auto-follow for planning. Visibility never
mutates `WorldData`.

Default policy: active space visible; co-visible `visibility_group_id` peers visible; spaces above
active reference elevation hidden by default (`space_hidden_by_default`).

### Building integration

On `hut` construction complete, `two_story_hut_profile()` registers ground/upper spaces and
stair/exterior portals relative to building placement.

### Persistence

Scene v3 serializes `current_space_id` on units. Derived registry/visibility links rebuild on load.
`ActiveViewedSpace` is client preference only.

### Underground seam (not implemented)

Architecture supports sub-surface `SpaceId`s, entrance portals, and hiding surface/upper spaces
without assuming `SpaceId` ordering equals vertical order.

## Consequences

- All `NavigationPath` consumers use `NavigationWaypoint`.
- Move orders resolve paths from unit `current_space_id` to surface goals by default.
- Future B7 doors extend portal enabled/state without replacing the graph model.
- Multiplayer will replicate `current_space_id` and portal enabled flags; view state remains local.

## Non-goals (B6)

Room simulation, furniture, door lock gameplay, elevators, ladders, underground content, manual
up/down primary UX, navmesh, building stacking, interior lighting.
