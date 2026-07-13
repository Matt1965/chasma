# Occupancy and Footprint Asset Authoring (B3)

Static occupancy is derived from **catalog footprints** and **world records**, not
from render meshes or ECS transforms at runtime. See
[ADR-080](../ADRs/ADR-080-generalized-occupancy-and-baked-footprints.md).

## Cell resolution

| Grid | Size |
|------|------|
| Occupancy cells | **2 m** |
| Navigation cells | **4 m** (unchanged) |

Each navigation cell covers a fixed 2×2 block of occupancy cells.

## Footprint shapes

### Circle / Rectangle

Author in the Buildings sheet (`Footprint Type`, width/depth/radius). No collision
mesh required. Used for simple structures and all blocking doodads (`block_radius_meters`).

### MeshDerived (irregular buildings)

1. Add a mesh node named **`occupancy_collision`** to the building GLB (or a
   dedicated collision GLB referenced by `Collision File Path`).
2. Geometry is rasterized **offline** at 2 m horizontal resolution (single surface
   slice for B3).
3. Export a versioned [`FootprintDefinition`] to
   `assets/buildings/footprints/<footprint_id>.ron`.
4. Reference the footprint from [`BuildingDefinition::footprint_id`] when wired.

**Do not** rely on render geometry for occupancy. A dev-only render-mesh fallback
is not part of B3 production workflow.

### Optional multi-space seam (future)

Node naming convention for B6+: `occupancy_collision_<space_id>`.

## Manual overrides

After rasterization, baked RON may include:

- `forced_open_cells` — unblock a cell (e.g. doorway correction)
- `forced_blocked_cells` — force block despite raster gap

Overrides must lie within mask bounds. Conflicts are validation errors at import.

## Rotation

- **Circle:** rotation ignored.
- **Rectangle / BakedCellMask:** **90° quantized** yaw only (`0`, `90`, `180`, `270`).
- Oblique rotations are rejected at registration — masks are not resampled.

## Stale bake detection

Baked footprints record `source_asset` and `source_hash` (file metadata hash in
dev bake). Re-import when collision assets change.

## Runtime authority

| Authoritative | Not authoritative |
|---------------|-------------------|
| `WorldData` building/doodad records | Render entities |
| `BuildingCatalog` / `DoodadCatalog` | GLB scene transforms at runtime |
| `FootprintCatalog` | Physics colliders |
| Derived `WorldData::occupancy` index | Presentation meshes |

Rebuild occupancy with [`rebuild_occupancy_index`] after bulk world loads.

[`FootprintDefinition`]: ../src/world/occupancy/footprint.rs
[`BuildingDefinition::footprint_id`]: ../src/world/building/catalog/definition.rs
[`rebuild_occupancy_index`]: ../src/world/occupancy/registration.rs
