# ADR-128: Asset Transform Composition (AT2)

## Status

Accepted

## Context

ADR-126 defined a single composed visual scale. AT1 made `AssetSizingDefinition` authoritative for
definition-owned meters and corrections. Runtime still had overlapping scale paths (legacy unit
`render_scale`, building flat vs child transforms, possible double yaw).

## Decision

### One composed visual scale

```
definition baseline  (catalog: baked import+desired, or explicit)
        ×
instance scale       (placement; default 1.0)
        =
presentation Transform.scale
```

Offline, import measurement and catalog desired meters produce the **single** definition baseline.
Runtime does not multiply separate “import baseline” and “catalog baseline” factors — that product
is already baked into `calculated_baseline_scale` / `explicit_baseline_scale`.

### Ownership

| Layer | Owns |
|---|---|
| Definition | Baseline scale, pivot offset, rotation correction |
| Instance | Placement position/orientation, instance scale |
| Presentation (ECS Transform) | Composed result only — never authoritative |

### API

- `compose_visual_scale(baseline, instance)`
- `building_visual_scale` / `doodad_visual_scale` / `unit_visual_scale`
- Building model child / flat world transform both use the same composition
- Building **anchor** carries placement pose only; definition rotation correction applies once on
  the model (no double yaw)

### Non-goals

- Collision / occupancy scaling (AT3)
- Content Excel/GLB rebakes (still AT2 content pass in ADR-126 roadmap; this ADR is runtime compose)
- Changing save format (instance scale already persisted)

## Migration notes

- Existing scenes keep working: instance milli scales unchanged; definition baselines already on catalogs
- Buildings with both legacy yaw mirror and `asset_sizing.rotation_correction` no longer double-apply yaw
- Units prefer metric baseline; legacy `render_scale` only when sizing is missing
- Call sites should use `*_visual_scale` / `compose_visual_scale`; older `*_final_render_scale` /
  `*_baseline_render_scale` aliases remain as wrappers

## Consequences

- Rendering, build-mode ghosts, Dev gizmos, and sync paths share one scale composition
- Collision systems unchanged until AT3
