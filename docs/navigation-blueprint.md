# Building Navigation Blueprint (NV1.1)

Gameplay metadata describing how units move through building interiors. Blueprints are **not** render meshes and **not** collision geometry.

## Ownership

| Layer | Location |
|-------|----------|
| Data model | `src/world/building/navigation_blueprint/` |
| Catalog | `BuildingNavigationBlueprintCatalog` (Bevy `Resource`) |
| On-disk format | `assets/buildings/navigation_blueprints/catalog.ron` |
| Asset link | `BuildingDefinition.navigation_blueprint_id` |
| Instance override | `BuildingInteriorState.navigation_blueprint_override` |

Runtime interior activation still uses B7 `InteriorProfile` for doors and child objects. When a navigation blueprint catalog is available, **NV1.3** registers spaces, portals, and walkable floor outlines from the blueprint as the authoritative interior navigation source.

Blueprints are loaded and resolvable at startup; completed buildings with `navigation_blueprint_id` consume them during interior activation.

## Coordinate space

All blueprint geometry is in **building-local space** (meters):

- **X** — width (matches footprint authoring)
- **Y** — elevation (floor height)
- **Z** — depth

World placement composes via `BuildingPlacement` and asset transform standardization (ADR-126–129). Instance uniform scale applies to blueprint distances at runtime.

## Schema version

`schema_version` must be `1` (constant `BUILDING_NAVIGATION_BLUEPRINT_SCHEMA_VERSION`).

## Catalog file

```ron
(
    definitions: [
        (
            id: "barn_interior",
            display_name: "Barn Navigation",
            schema_version: 1,
            metadata: (
                source_render_key: Some("barn"),
            ),
            floors: [ /* ... */ ],
            entrances: [ /* ... */ ],
            vertical_transitions: [ /* ... */ ],
            enabled: true,
        ),
    ],
)
```

Load at startup via `load_building_navigation_blueprint_catalog()`. Missing or invalid files fall back to Rust starters with a warning.

## Floor model

Floors use **sparse integer ids** (`floor_id: i32`). Intermediate ids may be absent:

| floor_id | Present? |
|----------|----------|
| -1 | optional basement |
| 0 | ground |
| 1 | optional mezzanine |
| 2 | optional attic |

Each floor defines:

| Field | Purpose |
|-------|---------|
| `key` | Stable string referenced by entrances and transitions |
| `display_label` | UI / debug label |
| `elevation_meters` | Building-local Y |
| `visibility_group_id` | Interior visibility grouping (ADR-083) |
| `walkable_outline` | Closed polygon in local XZ (≥ 3 vertices, CCW) |

## Entrances

Exterior portals from surface into a floor:

- `local_position_xz` — portal center on building exterior
- `radius_meters` — traversal disc radius
- `interior_spawn_local` — spawn XYZ after entering
- `bidirectional` — default `true`

## Vertical transitions

Stairs, ramps, and reserved `Ladder` kind between two floor keys:

- `from_local_position_xz` / `from_radius_meters` — transition trigger on source floor
- `to_local_position` — destination on target floor
- Maps to `PortalType::Stair` / `Ramp` via `blueprint_portal_templates()` (extension point)

## Instance overrides

`BuildingNavigationBlueprintInstanceOverride` on `BuildingInteriorState`:

1. **`inline_blueprint`** — full per-instance blueprint (does not modify asset)
2. **`blueprint_id`** — reference to another catalog entry (variant promotion seam)

Resolution order (`resolve_building_navigation_blueprint`):

1. Inline override
2. Override catalog id
3. `BuildingDefinition.navigation_blueprint_id`

## Future workflow (NV1.2 automatic generation)

```
GLB → automatic generation → developer edits → saved blueprint
```

Implemented in `src/world/building/navigation_blueprint/generate.rs` and invoked during dev building import (`resolve_dev_navigation_blueprint_catalog`).

### Generation pipeline

1. For each **Navigable** building with `interior_profile_id` or `navigation_blueprint_id`
2. Load collision GLB (`occupancy_collision` node preferred, visible mesh fallback)
3. Detect walkable horizontal surfaces → floor clusters by elevation
4. Build convex-hull floor outlines in building-local meters (baseline scale applied)
5. Detect `portal__*` markers for entrances / stairs / ramps
6. Synthesize a ground entrance from floor outline when no portal markers exist
7. Validate blueprint; emit warnings (never silent failure)
8. Write `assets/buildings/navigation_blueprints/catalog.ron`

### Cache invalidation

`assets/buildings/navigation_blueprints/cache_manifest.ron` stores per-blueprint:

- `collision_source_hash` (file size + mtime, same as occupancy bake)
- `render_source_hash` (optional)
- `baseline_scale_milli` from asset sizing
- `generator_version` (`NAVIGATION_BLUEPRINT_GENERATOR_VERSION`)

Regeneration runs when any of these change. Report: `logs/navigation_blueprint_report.md`.

### Authoring hooks in GLB

| Node / convention | Use |
|-------------------|-----|
| `occupancy_collision` | Preferred walkable/collision mesh for analysis |
| `portal__<name>` | Entrance / stair / ramp hints (`entrance`, `stair`, `ramp`, `ladder` in name) |

`metadata.source_render_key` and `metadata.generation_revision` record generator provenance.

## Dev inspection (NV1.2.5)

Read-only validation and visualization before runtime pathfinding consumes blueprints (NV1.3+).

### Inspector workflow

1. Select a building in Dev Mode (Inspector tab).
2. Press **`N`** to enter blueprint inspection (bird's-eye camera, overlay on).
3. Use **`[`** / **`]`** to cycle floors (sparse / negative ids supported).
4. Press **`1`–`9`** to highlight a validation diagnostic in the world overlay.
5. Press **`Shift+R`** to force-regenerate the blueprint for the selected building.
6. Press **`Esc`** to exit and restore the previous camera.

### Overlay toggle

**Nav blueprint** in the Dev panel (`DebugOverlayConfig.nav_blueprint`) draws generated floor polygons, vertices, entrances, and vertical transitions in world space from blueprint data (not mesh re-analysis). Independent from NV0 occupancy/resource-mask overlays.

### Validation

`validate_blueprint_for_inspection()` extends schema validation with dev diagnostics (errors / warnings / info): duplicate vertices, self-intersection, entrance boundary checks, transition floor references, etc. Results appear in the inspector panel and can be focused in the overlay.

### Camera helper

`frame_building_for_inspection()` in `src/dev/inspector/blueprint_inspection.rs` is reusable for the blueprint editor.

## Blueprint editor (NV1.4)

Visual refinement of generated navigation blueprints in Dev Mode. The editor layers on NV1.2.5 inspection — it does not replace automatic generation.

### Workflow

1. Select a building in the Inspector.
2. Press **`N`** to enter blueprint inspection (bird's-eye camera + overlay), or press **`E`** to enter edit directly.
3. Press **`E`** from inspection to begin editing (working copy of the resolved blueprint).
4. Use **`[`** / **`]`** to cycle floors.
5. Edit geometry with mouse and tool hotkeys (see below).
6. Press **`Ctrl+S`** to save the working copy to `assets/buildings/navigation_blueprints/catalog.ron`.
7. Press **`Esc`** to exit edit (returns to inspection; unsaved edits are discarded).
8. Press **`Esc`** again from inspection to restore the previous camera.

### Tools

| Key | Action |
|-----|--------|
| `1` | Select — click/drag vertices, entrances, transitions |
| `2` | Add vertex — click a floor edge |
| `3` | Add entrance — click inside the floor polygon |
| `Del` | Delete selected element |
| `+` / `-` | Adjust entrance or transition radius when selected |

### Editing model

- All edits apply to a **working copy** in `BlueprintInspectionState` (`src/dev/inspector/blueprint_edit.rs`).
- Mutations live in `src/world/building/navigation_blueprint/edit.rs` with polygon guards (minimum vertex count, edge length, degenerate polygons).
- Live validation uses `validate_blueprint_for_inspection()`; diagnostics appear in the inspector panel and overlay.

### Persistence (NV1.5)

Effective blueprint resolution order:

1. **Instance override** — `BuildingInteriorState.navigation_blueprint_override.inline_blueprint`
2. **Asset default** — `BuildingDefinition.navigation_blueprint_id` → catalog entry
3. **Generated** — catalog entry keyed by `blueprint_id_for_building()` when no explicit asset link exists

#### Editor save actions

| Action | Binding | Target |
|--------|---------|--------|
| **Save Instance** | Ctrl+S | Inline override on the selected building only |
| **Apply to Asset** | Ctrl+Shift+S | Catalog default (`catalog.ron`); confirms inheriting instance count |
| **Reset to Asset** | Ctrl+Alt+R | Clears instance override; resolves asset/generated |

- Saves validate via `prepare_blueprint_for_save()` (schema + inspection errors block write).
- Catalog writes are atomic (temp file + rename).
- Excel is not touched during routine editing; `assets/buildings/navigation_blueprints/catalog.ron` is the asset-default store.
- Scene persistence (v15+) serializes `navigation_blueprint_override` on `SceneBuildingRecord`.
- After save, `refresh_building_navigation_runtime()` rebuilds spaces/portals/runtime for affected instances without destroying interior children.
- **Regenerate from Mesh** (Shift+R) confirms before replacing authored/generated catalog data; instance overrides are preserved.

#### Source indicators

Inspector shows authority as **Instance Override**, **Asset Default**, or **Generated**, plus **(unsaved)** when the editor working copy differs from persisted state.

### Variant promotion (NV1.6)

**Save As Variant** (`Ctrl+Shift+V` in edit mode) promotes the edited blueprint into a new independent building asset:

1. Clone the source `BuildingDefinition` with a new id and display name.
2. Fork the edited blueprint into `{variant_id}_nav` in the navigation catalog.
3. Upsert both catalogs and export to RON (`assets/buildings/catalog.ron`, `navigation_blueprints/catalog.ron`).
4. Optionally replace the selected instance with the new variant (`Enter` / `Esc`).

Variants share mesh references and asset sizing with the source; blueprint data is fully independent after creation. Excel is not modified directly — the RON export is the dev-side artifact for later pipeline emission.

### Architecture seams

- **Variant promotion** — `BuildingNavigationBlueprintInstanceOverride::blueprint_id` remains a future catalog-reference override seam.
- **Undo** — edit ops are isolated functions so a command stack can wrap them later.

## Runtime consumption (NV1.3)

When a building completes (or interior is activated) and a navigation blueprint resolves:

1. **`register_building_navigation_profile()`** — registers B6 spaces and portals from blueprint templates using `building_model_world_transform` (asset sizing + placement).
2. **`BuildingNavigationRuntimeStore`** — caches per-floor world-space walkable polygons for passability and space resolution.
3. **Interior passability** — `query_interior_passability()` tests positions against blueprint floor outlines (not collision meshes).
4. **Cross-space pathfinding** — `find_path_in_spaces()` / `astar_path_in_space()` use space-scoped walkability; portal routes come from blueprint entrances and vertical transitions.
5. **Move orders** — `resolve_navigation_space_at_position()` infers goal space from runtime floor polygons.

`InteriorProfile` still owns doors, access policies, and interior child spawns. Portals without doors (e.g. barn entrance, stairs) are enabled at registration.

### Transform composition

Blueprint geometry is building-local. Runtime world positions compose:

```
world = building_model_world_transform(definition, placement, layout)
      × blueprint local point
```

Instance uniform scale flows through asset transform standardization (ADR-126–129). Building resize changes composed outlines without re-authoring the blueprint.

### Debug visualization (NV0)

With navigation debug toggles enabled in Dev Mode:

| Toggle | Runtime blueprint data |
|--------|------------------------|
| **Nav blueprint** | Catalog overlay (NV1.2.5) — authored/generated blueprint |
| **Nav entrances** / **Nav footprints** | Runtime floor outlines from `BuildingNavigationRuntimeStore` |
| **Nav entrances** | Portal markers from registered `SpaceRegistry` portals |
| **Path** | Active unit path with portal transition waypoints highlighted |

The selected unit's `current_space_id` highlights the active runtime floor polygon in the navigation overlay.

## Runtime interior navigation (NV2)

Building Navigation Blueprints are the **gameplay authority** for interior movement. Collision geometry and building occupancy on the surface do not define interior walkability.

### Path planning

- `resolve_navigation_start_space()` reconciles tracked `current_space_id` with position-based floor detection.
- `resolve_navigation_space_at_position()` picks the floor whose Y is closest when footprints overlap in XZ (multi-floor buildings).
- Cross-space paths stitch surface segments, blueprint entrance portals, interior floor A*, and vertical transition portals.
- Exterior entrance portal discs are walkable on the surface even when under building occupancy.

### Movement

- Move orders resolve start/goal spaces from runtime floor polygons before pathfinding.
- `step_unit_movement()` syncs space from position, prefers planned portal transitions, and uses blueprint polygon passability for interiors.
- `interior_position_walkable()` is strict for blueprint-activated buildings (blocks positions outside floor outlines).

### Cache invalidation

- `reposition_building_navigation_runtime()` rebuilds world-space floor outlines and portal poses when a building moves or scales (dev gizmo / transform edit).
- Full blueprint refresh still uses `refresh_building_navigation_runtime()` after editor saves.

### Debug (path overlay)

With **Path** debug enabled for selected units:

- Cyan floor outline for the active interior space
- Purple portal disc for the active transition waypoint
- Orange sphere for the world move target
- Yellow segment for the current local waypoint pursuit

### Fallback

If no blueprint resolves at activation, interior spaces and portals fall back to the B7 `InteriorProfile` templates (legacy path). Runtime passability treats missing runtime data as walkable inside interior spaces.

## Extension points

| Function | Purpose |
|----------|---------|
| `blueprint_space_templates()` | Floor → owned space templates |
| `blueprint_portal_templates()` | Entrances + transitions → portal templates |
| `build_navigation_runtime()` | Blueprint + transform → runtime floor cache |
| `resolve_building_navigation_blueprint()` | Effective blueprint for a building instance |

## Related systems

- **Interior profiles (B7)** — doors, children, legacy space/portals (`InteriorProfile`)
- **Spaces/portals (B6)** — runtime `SpaceRegistry` instances
- **NV0 debug overlays** — visualize portals and footprints in dev mode
