# ADR-045: Scene System + World Snapshot Architecture

# Status

Accepted (DEV MODE v3 — WorldData scene snapshots)

# Context

ADR-043/044 established dev mode for single-click and batch placement via WorldData
authoring APIs. Designers need reproducible test scenarios: save/load world layouts,
manage named scenes, and reload deterministic debugging environments without touching
simulation rules or ECS state.

# Decision

## Scenes are WorldData snapshots, not ECS snapshots

[`SceneDefinition`](../src/dev/scenes/snapshot.rs) serializes:

- Unit records (id, definition, placement, Idle/Moving state, source)
- Doodad records (id, definition, placement, source)
- World seed, optional camera pose, optional debug flags
- Monotonic id counters for deterministic restore

No ECS entities, meshes, render assets, or command trace buffers are stored.

## Pipeline

```text
capture_scene(WorldData) → SceneDefinition → RON file
load: validate → dev_clear_units_and_doodads → insert_unit/insert_doodad → sync id counters
```

[`apply_scene`](../src/dev/scenes/load.rs) validates catalog references **before** clearing
world entities so invalid files fail without corruption.

## Persistence (dev-only)

- Directory: [`dev_scenes/`](../src/dev/scenes/save.rs) (`DEV_SCENES_DIR`)
- Format: **RON** with [`SCENE_VERSION = 1`](../src/dev/scenes/snapshot.rs)
- Index: `dev_scenes/index.ron` via [`SceneRegistry`](../src/dev/scenes/registry.rs)

Not linked to production save/load or multiplayer sync.

## Determinism

Capture iterates [`sorted_unit_ids`](../src/world/data.rs) and
[`sorted_doodad_ids`](../src/world/data.rs). Repeated save/load cycles produce identical
record ordering and positions when catalogs are unchanged.

## UI

New **Scenes** tab (F12 dev panel):

- Save Current World, Reload Last Scene, Clear World, Delete Scene
- Scene name input + searchable scene list (click row to load)

## Safety

- Compiled only with `dev` feature (`src/dev/` module)
- Clears units/doodads/command buffer/smoothing only — terrain and authored extent unchanged
- Does not modify movement, pathfinding, steering, or interaction systems

## Relationship to future multiplayer replays

Scenes capture **authoritative world instance data** suitable for deterministic local
scenario setup. Multiplayer replays would require input/command journals and synchronized
simulation ticks — explicitly out of scope. Scene format provides a compatible data layer
(units/doodads/placements) that replay systems could reference later.

# Consequences

## Positive

- Reproducible RTS test beds and balance tuning setups
- Human-readable RON diffs for scenario review
- Validation reuses existing catalog and WorldData APIs

## Negative

- Scene restore uses `insert_unit`/`insert_doodad` (not `create_*`) to preserve ids
- Procedural doodad keys re-registered on load when source is procedural
- No terrain editing in scenes (terrain must already be resident/imported)

# Verification

`cargo test --lib dev::scenes --features dev`
