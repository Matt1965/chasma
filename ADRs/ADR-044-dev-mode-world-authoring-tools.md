# ADR-044: Dev Mode World Authoring Tools (v2)

# Status

Accepted (DEV MODE v2 — batch placement, brushes, terrain-aware spawning)

# Context

ADR-043 introduced F12 dev mode with single-click spawn via [`create_unit`](../src/world/unit/authoring.rs)
and [`create_doodad`](../src/world/doodad/authoring.rs). Designers need efficient world layout
authoring: lines, grids, circles, and scattered batches — still through WorldData APIs only.

# Decision

## Pipeline

```text
Input → Dev brush settings → pattern offsets → placement rules → batch spawn → WorldData
                              ↓
                     preview (gizmos only, no ECS)
```

## Brush system (`src/dev/tools/brush.rs`, `pattern.rs`)

[`BrushMode`](../src/dev/tools/brush.rs): SingleClick, Line, Circle, Grid, RandomScatter.

Pattern generation is deterministic:

- Scatter seed = `dev_placement_seed(world_seed, anchor, definition_id)`
- Reuses [`DeterministicRng`](../src/world/doodad/generation/rng.rs)

Safety cap: [`MAX_BRUSH_SPAWN_COUNT`](../src/dev/tools/brush.rs) = 256 per operation.

Reusable buffers: [`BrushPointBuffer`](../src/dev/tools/brush.rs), [`BatchSpawnScratch`](../src/dev/tools/batch_spawn.rs).

## Batch spawn (`batch_spawn.rs`)

[`BatchSpawnRequest`](../src/dev/tools/batch_spawn.rs) carries brush params + [`PlacementRules`](../src/dev/tools/placement_rules.rs).

[`execute_batch_spawn`](../src/dev/tools/batch_spawn.rs) calls authoring APIs only; reports
attempted / spawned / rejected / failed counts.

## Placement rules (`placement_rules.rs`)

Reuses authoritative world queries (no simulation changes):

| Rule | World API |
|------|-----------|
| Terrain snap | [`ground_world_position`](../src/world/terrain/query.rs) |
| Slope limit | [`is_position_slope_walkable`](../src/world/terrain/query.rs) + catalog max slope |
| Doodad collision | [`is_position_blocked_by_doodads`](../src/world/obstacle/query.rs) |
| Biome (doodads) | [`WorldData::biome_at`](../src/world/data.rs) + `DoodadDefinition::allows_biome` |
| Min distance | Batch peer spacing in XZ global space |

## Preview (`preview.rs`)

[`DevPlacementPreview`](../src/dev/tools/preview.rs) stores valid/invalid points.
Drawn via gizmos under U-UI3 interaction overlay category — **no ECS entities**, no WorldData mutation.

## UI (`panel.rs`)

New **Placement** tab: brush mode cycle, count/spacing/radius adjust, terrain snap + preview toggles.
Units/Doodads tabs still select definitions; Placement tab configures batch behavior.

# Consequences

## Positive

- Efficient multi-entity authoring without bypassing simulation authority
- Deterministic scatter for reproducible dev layouts
- Validation reuses existing terrain/obstacle/biome systems

## Negative

- Placement tab requires selecting definition on another tab first
- Biome enforcement skipped when biome mask unavailable (same as procedural pipeline)
- Grid overlay toggle still reserved from v1

# Verification

- `cargo test --lib --features dev` (tools module tests)
- `cargo check --features dev`
