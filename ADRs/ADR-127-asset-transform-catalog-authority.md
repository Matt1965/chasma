# ADR-127: Asset Transform Standardization — Catalog Authority (AT1)

## Status

Accepted

## Context

ADR-126 (AT0) locked meters-first catalog authority. DT1–DT4 already embedded
`AssetSizingDefinition` on content definitions, but building pivot/yaw still lived in parallel
legacy fields and builtins, and MissingSizingData was a quiet warning.

## Decision

AT1 establishes **definition-owned sizing data** without changing the runtime transform pipeline.

### Authoritative fields (`AssetSizingDefinition`)

- Desired metric dimensions
- Measured / explicit source dimensions
- Baked baseline import scale
- Pivot correction (`model_local_offset_meters`)
- Import rotation correction

### Legacy mirrors

`BuildingDefinition.model_local_offset` and `model_yaw_correction_degrees` remain for compatibility.
`normalize_building_sizing_authority` folds legacy + temporary builtins into `asset_sizing`, then
syncs mirrors. Catalog load and building finalize call this path.

### Dev Mode

Asset sizing panel shows source, desired, baseline, pivot, rotation, approx final meters, and
prominently flags MissingSizingData.

### Import

MissingSizingData emits an AT1 error-level report entry (still soft for row validity) plus a
warning that presentation may be wrong until migrated.

### Non-goals (deferred)

- Applying a new presentation composition pipeline
- Content Excel/GLB rebakes (AT2)
- Collision/footprint unification (AT3)
- Removing builtins permanently (AT2)

## Consequences

- Save files unchanged (instance scale already persisted; definitions ship with catalogs)
- Existing assets continue loading; unmigrated content still gets scale 1.0 with louder diagnostics
- AT2 can author Desired meters on the core set without further schema invention
