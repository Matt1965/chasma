# ADR-080: Generalized Occupancy and Baked Footprints (B3)

# Status

Accepted (B3 — static occupancy foundation, doodad migration)

# Context

ADR-078/079 established building **definitions** and **instances** on [`WorldData`].
ADR-031 added doodad circle obstacles as a separate query path consumed by movement
(U6) and navigation (U7).

Phase B3 needs one authoritative, deterministic footprint and static-occupancy
system shared by buildings, doodads, and future structures (walls, furniture, ruins,
construction sites) without player build mode, ghosts, construction progress, spaces,
doors, runtime mesh rasterization, or dynamic unit collision on the occupancy grid.

# Decision

## Occupancy is derived authoritative index

Static occupancy is **world-derived data**, rebuildable from:

- [`BuildingRecord`] + [`BuildingDefinition`] / [`FootprintDefinition`]
- [`DoodadRecord`] + [`DoodadDefinition`]

It is **not** derived from render entities, GLB transforms at runtime, physics
colliders, or presentation meshes.

[`WorldData::occupancy`] stores a chunk-keyed derived index
([`ChunkOccupancyGrid`]). It is not primary truth and is not serialized as
authoritative state.

## Module layout

`src/world/occupancy/` owns:

| File | Responsibility |
|------|----------------|
| `cell.rs` | Cell resolution, coords, quantized rotation |
| `footprint.rs` | [`FootprintShape`], geometry, occupied-cell expansion |
| `catalog.rs` | [`FootprintCatalog`], [`FootprintId`] |
| `grid.rs` | Per-chunk occupancy storage |
| `registration.rs` | Insert/move/remove lifecycle, atomic plans, rebuild |
| `query.rs` | Geometric static-occupancy queries (WorldData + catalogs) |
| `passability.rs` | Composed passability aggregator |
| `bake.rs` | Offline triangle/GLB rasterization (`data-import`) |
| `error.rs` | Structured [`OccupancyError`], [`OccupancySource`] |

## Footprint shapes

[`FootprintShape`]:

| Variant | Use |
|---------|-----|
| `Circle { radius_meters }` | Doodads, simple round structures |
| `Rectangle { width_meters, depth_meters }` | Simple axis-aligned buildings |
| `BakedCellMask { … }` | Irregular walls/doorways from offline bake |

[`FootprintDefinition`] is catalog data (no render geometry). Buildings may reference
[`FootprintId`] or use inline [`FootprintSpec`] from [`BuildingDefinition`] until
baked catalogs are fully wired.

### BakedCellMask fields

- `cell_size_meters`, `width_cells`, `depth_cells`, `local_origin`
- `blocked_cells` (row-major `z * width + x`)
- optional `forced_open_cells`, `forced_blocked_cells` (data-authored overrides)
- `space_id` seam (default surface `0`; multi-space baking deferred to B6)

## Cell resolution and rotation

- **Occupancy cells: 2 m** ([`OCCUPANCY_CELL_SIZE_METERS`])
- **Navigation cells: 4 m** (existing [`NavigationConfig`]) — each nav cell spans
  a deterministic 2×2 occupancy block
- **Rotation (B3):** quantized **90°** yaw for `Rectangle` and `BakedCellMask`
  ([`QuantizedRotation`]); `Circle` ignores rotation
- Unsupported oblique rotations are **rejected** ([`OccupancyError::InvalidRotation`]),
  not silently resampled

## Collision mesh convention

Offline baker input (GLB):

- Primary node name: **`occupancy_collision`**
- Optional future multi-space nodes: `occupancy_collision_<space_id>`
- Render geometry is **not** used by default
- `Circle` / `Rectangle` footprints do not require a collision node
- `MeshDerived` / bake-from-GLB **fails clearly** when the node is missing — no
  silent bounding-box fallback

Baking rules (horizontal slice only for B3):

1. Transform collision geometry to building-local XZ
2. Rasterize at occupancy cell size
3. Mark intersected cells blocked
4. Apply manual overrides
5. Reject non-finite geometry, empty results, oversized masks
6. Record source asset path/hash for stale-bake detection

Bake output: versioned RON [`FootprintDefinition`] (recommended path
`assets/buildings/footprints/<footprint_id>.ron`). No runtime Excel/GLB parsing
for occupancy queries.

## Registration lifecycle

[`register_building_occupancy`], [`register_doodad_occupancy`],
[`update_*_occupancy`], [`unregister_source_occupancy`],
[`rebuild_occupancy_index`]:

- Deterministic registration order
- Cross-chunk footprints register in every overlapped chunk
- Plans validate first; apply atomically; failed updates preserve prior state
- Each cell stores [`OccupancyState`] + [`OccupancySource`] (`Building` / `Doodad`)

B3 policy: surface space only; static completed blocking (no construction-state
occupancy — deferred to B5).

## Composed passability

[`query_passability_at`] is the concrete aggregator (no trait-object registry on
the A\* hot path). Fixed order with short-circuiting:

1. Terrain availability / grounding
2. Terrain slope
3. Static occupancy (buildings + doodads via geometric footprint overlap)
4. Future dynamic blockers / movement modifiers seam

[`PassabilityResult`]: `Passable { movement_cost_multiplier }` |
`Blocked { reason, source }` | `Unavailable { reason }`.

Occupancy errors **fail closed** for movement and navigation.

Navigation ([`src/world/navigation/grid.rs`]) and per-step movement
([`step_unit_movement`]) both consume this API so they cannot disagree about
static occupancy. Unit-unit steering remains separate (ADR-036).

## Doodad migration (ADR-031 superseded for blocking)

Doodad blocking maps to `FootprintShape::Circle` using `block_radius_meters` /
`blocks_movement`. [`src/world/obstacle/query.rs`] delegates to occupancy
passability; parallel circle-overlap paths are removed.

- Inclusive `<=` boundary behavior preserved
- Missing definitions on blocking kinds: conservative radius + fail-closed bool helpers
- Non-blocking doodads remain passable

## Explicit non-goals (B3)

- Player build mode, ghosts, construction progress
- Spaces/portals/floors, doors, stairs, underground
- A\* algorithm rewrite, navmesh
- Runtime mesh rasterization
- Dynamic unit collision on occupancy grid
- Free-rotation mask resampling

# Consequences

**Benefits:**

- One footprint model for buildings, doodads, and future static blockers
- Deterministic, rebuildable occupancy index
- Navigation and movement share one passability contract
- Offline bake seam ready for irregular assets and B6 height/space slicing

**Follow-ups:**

- B4/B5: construction-state occupancy
- B6: multi-space / height-band baking
- Full footprint Excel → RON bake pipeline export in dev import
- Player placement consuming footprint validation

# References

- ADR-078, ADR-079 (building catalog + runtime)
- ADR-031 (doodad obstacles — migrated to occupancy)
- ADR-032 (navigation — now uses passability)
- ADR-066 (movement blocking semantics)

[`WorldData::occupancy`]: ../src/world/data.rs
[`ChunkOccupancyGrid`]: ../src/world/occupancy/grid.rs
[`FootprintShape`]: ../src/world/occupancy/footprint.rs
[`FootprintDefinition`]: ../src/world/occupancy/footprint.rs
[`FootprintCatalog`]: ../src/world/occupancy/catalog.rs
[`FootprintId`]: ../src/world/occupancy/catalog.rs
[`OCCUPANCY_CELL_SIZE_METERS`]: ../src/world/occupancy/cell.rs
[`QuantizedRotation`]: ../src/world/occupancy/cell.rs
[`OccupancyError`]: ../src/world/occupancy/error.rs
[`OccupancySource`]: ../src/world/occupancy/error.rs
[`query_passability_at`]: ../src/world/occupancy/passability.rs
[`register_building_occupancy`]: ../src/world/occupancy/registration.rs
[`register_doodad_occupancy`]: ../src/world/occupancy/registration.rs
[`rebuild_occupancy_index`]: ../src/world/occupancy/registration.rs
[`BuildingRecord`]: ../src/world/building/record.rs
[`BuildingDefinition`]: ../src/world/building/catalog/definition.rs
[`FootprintSpec`]: ../src/world/building/footprint.rs
[`DoodadRecord`]: ../src/world/doodad/record.rs
[`DoodadDefinition`]: ../src/world/doodad/catalog/definition.rs
[`NavigationConfig`]: ../src/world/navigation/grid.rs
[`step_unit_movement`]: ../src/world/unit/movement.rs
