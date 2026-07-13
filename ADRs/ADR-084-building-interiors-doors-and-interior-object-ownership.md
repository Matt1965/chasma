# ADR-084: Building Interiors, Doors, and Interior Object Ownership

## Status

Accepted (B7)

## Context

B6 introduced navigable `Space` records, `Portal` edges, and interior visibility. B7 makes
building interiors authoritative: walls via occupancy, doors with runtime state, and authored
child objects without a separate interior simulation layer.

## Decision

### Interior profiles

- `InteriorProfileCatalog` holds authored profiles keyed by `InteriorProfileId`.
- `BuildingDefinition.interior_profile_id` links a building type to a profile.
- Profiles contain space templates, portal templates, door templates, and child placements.

### Functional vs stateless objects

- **Doodad** — stateless scenery (chairs, rugs, debris).
- **Building** — owned functional state (workbenches, stations, future storage).

Classification is by ownership semantics, not physical size.

### Door authority

- `DoorRecord` on `WorldData.door_store` is the single source of truth for passability.
- `PortalRecord.enabled` is **derived** from `DoorState` via `DoorStore::sync_portal_enabled`.
- `DoorState`: Open, Closed, Locked, Destroyed.
- `DoorAccessPolicy`: Everyone, OwnerOnly, Team, Locked (runtime ownership; not catalog faction).

### Walls

- B7 walls contribute blocked cells through baked occupancy on the parent building scene.
- Walls are not separate targetable records; destructible wall sections are deferred.

### Interior activation

- Interiors activate only when parent building reaches **Complete** (`activate_building_interior`).
- `deactivate_building_interior` runs on ruins, destruction, and parent removal.
- Child objects are not spawned before Complete.

### Parent / child ownership

- `BuildingRecord.parent_building_id` and `DoodadMetadata.parent_building_id` link children.
- `BuildingInteriorState` tracks door ids and child ids for deterministic cleanup.
- ECS hierarchy is presentation only.

### Navigation

- Pathfinding consults `portal_traversable` / `space_route_for_unit` with optional unit ownership.
- Movement auto-opens closed doors for authorized units before portal transition.

### Room metadata

- `SpaceRecord.room_tag` and `SpaceTemplate.room_tag` store optional zone labels only.
- No room simulation, bonuses, or temperature in B7.

### Persistence

- Scene format v4 captures interior activation, child ids, door snapshots, and doodad parent metadata.
- Scene load re-activates interior shells and reapplies door states by `definition_key`.

## Consequences

- Door animation is presentation-only; passability never waits on animation.
- Interior child duplicates are prevented on scene restore when child ids are already present.
- Worker tasks, production, storage inventory, and procedural furnishing remain out of scope.

## Related

- ADR-083 — Navigable Spaces, Portals, and Interior Visibility
- ADR-082 — Building Construction, Vitals, and Ruins
- ADR-080 — Generalized Occupancy and Baked Footprints
