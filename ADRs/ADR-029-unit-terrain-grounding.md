# ADR-029: Unit Terrain Grounding

# Status

Accepted (U4 — authoritative height snap)

# Context

U2 placed [`UnitRecord`] instances on [`WorldData`] with author-specified positions.
U3 synced disposable render entities from that placement. Authoring may set Y before
terrain is resident; simulation eventually needs authoritative Y from heightfields.

ADR-010 and ADR-023 established that terrain **meshes** and render
[`TerrainRenderAssets::vertical_scale`] are presentation only. Heightfield samples
on [`WorldData`] are simulation authority (ADR-005, ADR-008).

Doodad placement finalization already snaps Y via [`WorldData::sample_height_at_position`]
(ADR-022). Units need an explicit, separate grounding API — not automatic on
[`create_unit`] — so records can exist before terrain loads.

# Decision

## Explicit grounding on WorldData

Add `src/world/unit/grounding.rs`:

- [`ground_unit_position`] — read-only height snap for a [`WorldPosition`]
- [`ground_unit_to_terrain`] — mutates [`UnitRecord::placement`] Y via [`relocate_unit`]

Rules:

- Sample resident [`ChunkData`] heightfields only — never terrain runtime meshes
- If terrain is unavailable: return [`UnitGroundingError::TerrainUnavailable`], **no mutation**
- X/Z, rotation, state, source, and metadata unchanged
- [`create_unit`] unchanged — grounding is a separate explicit call

## Shared terrain query module

Promote reusable height/slope helpers to `src/world/terrain/query.rs`:

- [`ground_world_position`] — height snap (shared with future systems)
- [`estimate_slope_degrees`] — moved from doodad `terrain_validation` (re-export preserved)

Future movement and pathfinding will use the same query layer against heightfields.

## Render sync unchanged

U3 [`sync_unit_render_entities`] reads [`WorldData`] placement each tick. Grounding
updates authoritative Y; render Y follows on the next sync via existing vertical
scale (ADR-028). Sync does not write back to world data.

## Error types

[`UnitGroundingError`]:

- `UnitNotFound`
- `TerrainUnavailable`

No string errors.

# Consequences

**Benefits:**

- Clear separation between authoring, grounding, and rendering
- Safe failure when terrain not resident
- Shared query seam for units, doodads, and future locomotion

**Deferred:**

- Automatic per-frame grounding
- Slope-constrained placement for units (definition `max_slope_degrees` exists but is not enforced in U4)
- Movement, pathfinding, collision

# References

- ADR-005 (height queries)
- ADR-010 (visualization vs truth)
- ADR-022 (doodad terrain snap)
- ADR-027 (unit data ownership)
- ADR-028 (unit runtime sync)

[`UnitRecord`]: ../src/world/unit/record.rs
[`WorldData`]: ../src/world/data.rs
[`ground_unit_position`]: ../src/world/unit/grounding.rs
[`ground_unit_to_terrain`]: ../src/world/unit/grounding.rs
[`ground_world_position`]: ../src/world/terrain/query.rs
[`relocate_unit`]: ../src/world/data.rs
[`create_unit`]: ../src/world/unit/authoring.rs
[`UnitGroundingError`]: ../src/world/unit/grounding.rs
[`sync_unit_render_entities`]: ../src/units/sync.rs
