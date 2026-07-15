# ADR-095: Building Runtime Asset and Scene Integration (BA1)

# Status

Accepted (BA1 — catalog-driven GLB presentation)

# Context

ADR-079 B2 intentionally used placeholder cuboids sized from catalog footprints.
ADR-078 defined `BuildingDefinition::render_key` and asset paths under
`assets/buildings/`, but the runtime layer never loaded glTF scenes.

Real building GLBs (`hut.glb`, `barn.glb`, `chest.glb`) exist on disk while
`smelter`, `workbench`, and collision-only keys remain absent — those must not
display neutral placeholder cubes that look like valid content.

# Decision

## Asset convention

| Topic | Policy |
|-------|--------|
| Path | `assets/buildings/{render_key}.glb` |
| Scene | glTF Scene 0 (`DEFAULT_GLTF_SCENE_INDEX`) until per-definition override |
| Units | Meters |
| Origin | Placement anchor at foundation/ground contact |
| Forward | Matches building quantized rotation (no runtime per-asset hacks) |
| Scale | `Vec3::ONE` at root unless future data-driven scale is added |

Optional scene node naming (graceful when absent):

- `space:{space_id}` — presentation tag only; authoritative Spaces remain catalog/baked data
- `roof:` prefix — roof/ceiling hide candidates for future visibility wiring

## Authority

Presentation derives from:

`BuildingRecord` + `BuildingDefinition` + lifecycle state → render key → `SceneRoot`

The GLB never defines placement, occupancy, collision, health, ownership,
construction state, or Space topology.

## Shared asset cache

[`BuildingSceneAssets`] (`src/buildings/assets.rs`):

- Maps render key string → `Handle<Scene>` (shared across all instances)
- Preloads unique keys from catalog at startup (`render_key`, `preview_render_key`)
- Tracks missing/failed keys with once-only warnings

## Runtime spawn (`BuildingsRuntimePlugin`)

`sync_building_render_entities` mirrors doodad/unit async lifecycle:

1. Wait for `LoadState::Loaded` before spawning `SceneRoot`
2. Ground-anchor transform (no cuboid half-height offset)
3. Respawn only when active render key changes
4. Lifecycle-only changes apply tint without respawn

## Lifecycle visuals

Per-lifecycle GLB keys are not on `BuildingDefinition` yet. All lifecycle states
resolve to `render_key`; construction presentation uses
[`lifecycle_building_color`] tint on scene materials.

Fallback chain when optional stage art is absent:

1. State-specific key (future)
2. Complete `render_key` + lifecycle tint (current)
3. Diagnostic fallback (missing/failed asset)

## Diagnostic fallback

Spawn a magenta-tinted footprint cuboid only when:

- definition missing
- render key unset
- asset load failed

Never use neutral affiliation cubes as the normal success path.

## Build mode ghost

`sync_build_mode_ghost_scene` loads `preview_render_key` or `render_key` with
planned-state tint. Footprint gizmo overlay remains (ADR-081).

## Dev diagnostics

Building inspector snapshot includes desired render key, resolved path, load state,
runtime entity, fallback reason, and discovered scene tag counts.

## Validation

Excel import continues to warn on missing `.glb` files. Runtime never panics on
missing presentation.

# Consequences

- Valid configured buildings display real GLBs; missing assets are obvious.
- Occupancy/navigation unchanged (footprint catalog authority).
- Future per-lifecycle GLB keys extend `lifecycle_render_key` without sync rewrite.
- Roof/Space visibility can consume cached `BuildingSceneTags` when implemented.

[`BuildingSceneAssets`]: ../src/buildings/assets.rs
[`lifecycle_building_color`]: ../src/buildings/placeholder.rs
