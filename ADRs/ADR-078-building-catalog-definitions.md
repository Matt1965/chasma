# ADR-078: Building Catalog Definitions (B1)

# Status

Accepted (B1 — building definitions and data pipeline)

# Context

Chasma treats buildings as a first-class world object type alongside terrain,
doodads, units, and projectiles. ARCHITECTURE.md reserves an **Occupancy Layer**
for structures; ROADMAP Phase 6 will add footprints and runtime instances.

Before placement, rendering, construction, or navigation integration, the engine
needs a stable **type-definition** layer mirroring doodads (ADR-016) and units
(ADR-027).

# Decision

## Catalog owns type definitions; WorldData owns instances (later)

[`BuildingCatalog`] and [`BuildingCategoryCatalog`] are read-only Bevy
[`Resource`]s in the World Data Layer (`src/world/building/`). They are
initialized at startup and are **not** stored on [`WorldData`].

| Concern | Owner (B1) |
|---------|------------|
| Building type definitions | [`BuildingCatalog`] |
| Category metadata | [`BuildingCategoryCatalog`] |
| Building instances / construction state | **Deferred** (B2+) |
| Occupancy / navigation | **Deferred** (B3+) |

## BuildingDefinition contents (B1)

Each [`BuildingDefinition`] includes:

- `id`, `display_name`, `category_id`
- `render_key` (model asset stem → `assets/buildings/{key}.glb`)
- `collision_render_key` (future offline occupancy baker input)
- optional `preview_render_key`
- `max_hp`, `build_time_seconds`, `max_slope_degrees`, `enabled`
- [`FootprintType`] + [`FootprintSpec`] (`Rectangle`, `Circle`, `MeshDerived`)
- placeholder seams: `construction_stages_ref`, `task_provider_id`,
  `animation_profile_id`, `interaction_profile_id`, `default_space_id`

No runtime state, ECS entities, or occupancy baking in B1.

## Footprint types (describe only)

| Type | B1 behavior |
|------|-------------|
| `Rectangle` | `width_meters` × `depth_meters` on definition |
| `Circle` | `radius_meters` on definition |
| `MeshDerived` | requires `collision_render_key`; occupancy baking deferred |

## Excel import

Workbook sheets:

- `Building Categories` — category id, display name, description, enabled
- `Buildings` — definition rows validated against categories

Dev startup imports via `resolve_dev_building_catalog()` and exports
`assets/buildings/catalog.ron` (mirrors doodad RON export).

Production builds use empty catalogs unless a future non-Excel path is added
(ADR-049).

## Layer boundaries

- B1 does **not** spawn render entities, mutate [`WorldData`], or change navigation.
- Collision mesh convention and occupancy baker are documented seams only.

# Consequences

- B2 can add `BuildingRecord` on [`WorldData`] referencing [`BuildingDefinitionId`].
- B3 can consume `collision_render_key` for mesh-derived occupancy baking.
- B6 can attach navigable spaces via `default_space_id` placeholder.

[`BuildingCatalog`]: ../src/world/building/catalog/registry.rs
[`BuildingCategoryCatalog`]: ../src/world/building/category/registry.rs
[`BuildingDefinition`]: ../src/world/building/catalog/definition.rs
[`FootprintType`]: ../src/world/building/footprint.rs
[`FootprintSpec`]: ../src/world/building/footprint.rs
