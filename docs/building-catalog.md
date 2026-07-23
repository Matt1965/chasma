# Building Catalog Authoring (B1)

Building definitions are **data-only** catalog entries. Runtime instances and
rendering are separate from occupancy; static footprints and the derived occupancy
index are documented in [ADR-080](../ADRs/ADR-080-generalized-occupancy-and-baked-footprints.md)
(B3).

## Workbook sheets

| Sheet | Purpose |
|-------|---------|
| **Building Categories** | Grouping metadata (`residential`, `production`, …) |
| **Buildings** | One row per buildable structure |

Sheets live in `Chasma Design.xlsx` at the repo root.

## Buildings — required columns

| Column | Notes |
|--------|-------|
| Building ID | Stable string id (e.g. `hut`) |
| Name | Display name |
| Category | Must match `Category ID` on Building Categories sheet |
| Model File Path | glTF under `assets/buildings/` (stem becomes render key) |
| Health | Max HP (`> 0`) |
| Build Time | Construction baseline in seconds |
| Footprint Type | `Rectangle`, `Circle`, or `MeshDerived` |
| Enabled | `Y` / `N` (blank defaults to `Y`) |

## Buildings — optional columns

| Column | Notes |
|--------|-------|
| Collision File Path | Collision mesh for baker input; **required** for `MeshDerived` |
| Preview File Path | Optional ghost/preview mesh; falls back to Model when unset |
| Footprint Width / Depth | Required for `Rectangle` |
| Footprint Radius | Required for `Circle` |
| Max Slope | Placement slope limit (default 40°) |
| Construction Stages | Placeholder ref (B4+) |
| Task Provider | Placeholder ref for ADR-072 task generation |
| Animation Profile | Optional; must exist in Animation Profiles sheet |
| Interaction Profile | Placeholder ref (B8+) |
| Default Space | Placeholder navigable space id (B6+) |

## Footprint types

- **Rectangle** — simple axis-aligned footprint from width × depth (meters).
- **Circle** — circular footprint from radius (meters).
- **MeshDerived** — occupancy is baked offline from the collision mesh into a
  `BakedCellMask` footprint (see below). B1 stores the collision reference;
  dev import can rasterize via the `data-import` baker.

## Occupancy and footprints (B3)

Footprints are authoritative **catalog geometry**, not render meshes.

| Shape | Authoring | Collision GLB |
|-------|-----------|---------------|
| Rectangle | `Footprint Width` / `Depth` | Not required |
| Circle | `Footprint Radius` | Not required |
| MeshDerived | Baked mask in `FootprintCatalog` | **Required** — node `occupancy_collision` |

- Occupancy cells are **2 m**; navigation remains **4 m** (2×2 occupancy cells per nav cell).
- Rectangle and baked masks accept **90° quantized** rotation only.
- Optional manual overrides in baked data: `forced_open_cells`, `forced_blocked_cells`.
- Recommended bake output: `assets/buildings/footprints/<footprint_id>.ron`.
- Runtime queries use [`query_passability_at`]; the chunk occupancy index is derived and rebuildable.

## Asset paths

Model and collision paths normalize like other catalogs:

- `hut.glb` → render key `hut` → `assets/buildings/hut.glb`
- `assets/buildings/fort/wall.glb` → `fort/wall`

Import warns (does not fail) when expected `.glb` files are missing on disk.

At runtime (ADR-095 BA1), valid `render_key` values load `assets/buildings/{key}.glb`
as shared `SceneRoot` instances. Missing or failed assets show a magenta diagnostic
cuboid sized from the footprint — not a neutral placeholder cube.

Optional glTF node names for future interior visibility (presentation only):

- `space:{space_id}` — must match catalog Space ids when present
- `roof:` prefix — roof/ceiling hide candidates

### Placement anchor (ADR-096)

- Player/dev building anchors use **0.1 m** XZ quantization (not 2 m cell snapping).
- Footprint cells are rasterized from the continuous anchor.
- Optional `model_local_offset` and `model_yaw_correction_degrees` correct GLB pivot vs footprint.

## Dev export

Successful dev import writes `assets/buildings/catalog.ron` containing both
categories and definitions (RON mirrors the in-memory catalogs).

## Philosophy

Definitions describe **what can be built**. Instances describe **what was built**.
Keep runtime state off definitions — construction progress, ownership, and damage
belong on `BuildingRecord` (B2+).

Runtime instances live on [`WorldData`] in chunk-keyed stores (ADR-079 B2).
Presentation is derived in `src/buildings/` and must not become authority.

See [ADR-079](ADRs/ADR-079-building-runtime-foundation.md).

Interior navigation metadata (floors, entrances, vertical transitions) is authored separately via [navigation blueprints](navigation-blueprint.md) (`navigation_blueprint_id` on definitions).

## Variant promotion (NV1.6)

In dev mode, edited navigation blueprints can be promoted into new independent building assets via **Save As Variant** (`Ctrl+Shift+V` in blueprint edit mode). See [navigation-blueprint.md — Variant promotion](navigation-blueprint.md#variant-promotion-nv16).

- Clones the source `BuildingDefinition` with a new id and display name.
- Forks the edited blueprint into `{variant_id}_nav` (fully independent after creation).
- Shares mesh references and asset sizing with the source; does not duplicate GLB files.
- Upserts in-memory catalogs and exports to `assets/buildings/catalog.ron` and `assets/buildings/navigation_blueprints/catalog.ron`.
- Excel is not modified directly; the RON snapshots are the dev-side artifacts for later pipeline emission.
- New variants appear in the dev Buildings tab immediately and can be placed without restart.
