# ADR-129: Collision & Gameplay Metric Synchronization (AT3)

## Status

Accepted

## Context

ADR-126 requires one metric composition per concern so visual size, collision, picking,
placement, and occupancy never diverge into independent “scale knobs.” AT2 made ECS
`Transform` presentation-only via `definition baseline × instance`. Several gameplay paths
still ignored baseline and/or instance (notably doodad `CollisionShape::None` / `Baked`,
building continuous occupancy queries, and pick radii).

## Decision

### Single composition rules (no second scale channel)

| Content | Visual | Collision / occupancy / pick / placement spacing |
|---|---|---|
| Doodad | baseline × instance | authored collision/placement meters × **(baseline × instance)_xz** |
| Building | baseline × instance | authored footprint meters × **instance** only; baseline must ≈ footprint via validation |
| Unit | baseline (no instance today) | authored collision radius (unit validation remains AT5) |

Do **not** introduce `visual_scale`, `collision_scale`, or `editor_scale` as independent concepts.

### Doodads

- `resolve_doodad_collision` applies composed XZ to **all** shape variants (`None` ≡ Circle;
  `Baked` falls back to scaled circle until baked masks load).
- Pick / interaction / Dev placement spacing use `doodad_interaction_radius_meters` /
  `doodad_definition_placement_radius_meters` (same compose).
- Mismatch diagnostics compare **meters vs meters** (approx final visual XZ vs collision radius).

### Buildings

- Registration, continuous occupancy query, placement overlap, pick, and ghosts use
  `effective_building_footprint_for_placement(..., instance_uniform_scale)`.
- Definition baseline does **not** multiply footprints (ADR-126 preferred path).
- Import + finalize warn/fail when navigable visual final size diverges from footprint (~25%).
- Circle footprints participate in topology checks (diameter vs visual W/D).

### Ownership

| Layer | Owns |
|---|---|
| Catalog | Desired meters, baked baseline, authored collision/footprint meters |
| Instance | Placement pose + instance scale |
| `resolve_doodad_collision` / `effective_building_footprint_for_placement` | Effective gameplay shapes |
| Presentation Transform | Composed visual only |

## Migration notes

- Blocking doodads with `CollisionShape::None` (Excel default) now scale with baseline × instance;
  scenes that relied on unscaled radii while using large baselines will block more/less correctly —
  fix authored `block_radius_meters` if needed.
- Dev-scaled buildings now affect passability queries and pick reach consistently with occupancy cells.
- No scene format change; revalidate occupancy after load (already required).

## Non-goals

- Auto-deriving footprints from desired meters (later AT)
- Unit collision ↔ visual enforcement (AT5)
- Loading real baked doodad masks (scaled circle fallback until then)

## Consequences

- Players see what they collide with
- One mental model: meters composed by ADR-126 rules
- Validation catches content drift instead of a second scale slider
