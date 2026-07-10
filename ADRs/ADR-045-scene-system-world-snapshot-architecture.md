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
load: validate plan → backup → clear → restore_unit_record/restore_doodad_record → verify indexes → sync id counters
```

[`apply_scene`](../src/dev/scenes/load.rs) validates **all** catalog references and record
invariants into a temporary [`RestorePlan`](../src/dev/scenes/load.rs) **before** mutating
[`WorldData`](../src/world/data.rs). Invalid files fail with structured
[`SceneApplyError`](../src/dev/scenes/load.rs) and leave the current world unchanged.

On apply failure after clear, a [`DevWorldEntityBackup`](../src/dev/scenes/load.rs) rolls back
the prior units/doodads and id counters.

## Validated restore APIs (REVIEW-A5)

Scene load does **not** call low-level `insert_unit` / `insert_doodad` directly.

| API | Responsibility |
|-----|----------------|
| [`validate_unit_for_restore`](../src/world/unit/restore.rs) | Catalog/enabled, within-scene duplicate ids, placement chunk, vitals normalization policy |
| [`restore_unit_record`](../src/world/unit/restore.rs) | Normalize persistent state, clear ephemeral combat timing, insert with preserved id |
| [`validate_doodad_for_restore`](../src/world/doodad/restore.rs) | Catalog/enabled/kind/scale, duplicate ids and procedural keys |
| [`restore_doodad_record`](../src/world/doodad/restore.rs) | Insert with preserved id; re-register procedural keys |

`create_unit` / `create_doodad` are not used for restore (they allocate new ids).

## Identity policy

**Preserve snapshot ids** (Option A):

- Reject duplicate `UnitId` / `DoodadId` within the scene file
- After successful apply, advance `next_unit_id` / `next_doodad_id` to scene counters via
  [`dev_restore_id_counters`](../src/world/data.rs)

No id remapping or reference rewriting in the current scene format.

## Persistent vs transient state

**Restored:** placement, ownership, definition, movement state (`Idle`/`Moving`), source.

**Normalized on restore:** vitals from catalog (`max_hp`, alive `current_hp`); `combat_state = Peaceful`;
`attack_cycle = None`.

**Cleared on load** (via [`dev_clear_units_and_doodads`](../src/world/data.rs)):

- All unit/doodad instances
- In-flight projectiles, removal queue, kill attributions
- Command buffer and movement smoothing scratch

Projectiles and combat timing are **transient simulation state** — not in scene files.

## Catalog failure behavior

Missing or disabled definitions reject the **entire** load. No starter substitution, no
best-effort partial apply.

## Index verification

After apply, [`verify_instance_indexes`](../src/world/data.rs) checks unit/doodad location
maps, procedural doodad keys, and duplicate ids.

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
- Clears units/doodads and transient simulation queues only — terrain and authored extent unchanged
- Does not modify movement, pathfinding, steering, or interaction systems
- Scene restore is validated authoring, not raw WorldData insertion

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

- Scene restore bypasses `create_*` id allocation — uses dedicated restore APIs instead
- Procedural doodad keys re-registered on load when source is procedural
- No terrain editing in scenes (terrain must already be resident/imported)
- Attack targets and in-flight projectiles are not captured; combat state resets on load

# Verification

`cargo test --lib dev::scenes --features dev`
