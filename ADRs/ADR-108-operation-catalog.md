# ADR-108: Operation Catalog (EP3)

## Status

Accepted (EP3)

## Context

ADR-107 (EP2) established the generic building production runtime: buildings own progress,
workers contribute labor, and `BuildingOperationPolicy` stores player/AI intent including a
typed `OperationDefinitionId` seam. EP2 deliberately omitted authored operation content,
building compatibility, and catalog validation.

EP3 adds the authoritative **Operation Catalog** — immutable authored data describing what
buildings *can* produce, without performing production or owning runtime state.

## Decision

### Authority and ownership

| Concept | Owner | Persisted in saves |
|---------|--------|-------------------|
| `OperationDefinition` | `OperationCatalog` (Bevy `Resource`) | No |
| `OperationDefinitionId` | `BuildingOperationPolicy` | Yes |
| Runtime progress | `BuildingOperationState` | Yes |
| Player/AI intent | `BuildingOperationPolicy` | Yes |
| Building compatibility | `BuildingDefinition.supported_operations` | No (catalog) |

The catalog is read-only at runtime. Policy stores **IDs only**, never full definitions.

### Operation definitions

`OperationDefinition` is generic authored content:

- Stable `OperationDefinitionId` (not display names or raw strings at authority boundaries)
- Display metadata: name, description, `OperationCategory`
- Production parameters: `base_labor`, `max_workers`, `repeatable`, `requires_collection`
- Future seams (typed, serializable, validated; not executed): inputs, outputs (items and
  non-item effects), terrain/tool/power/skill requirements

Categories (`Extraction`, `Processing`, `Crafting`, `Agriculture`, `Research`, `Medical`,
`Ritual`) assist UI, filtering, validation, and future AI. No runtime behavior is driven
by category alone.

### Building compatibility

Buildings declare `supported_operations: Vec<OperationDefinitionId>` explicitly.
`default_operation_id` is optional; when exactly one operation is supported, auto-selection
is acceptable. When multiple are supported, only an authored default or no selection is
allowed — never arbitrary first-list selection.

`task_provider_id` is deprecated in favor of explicit operation lists.

### Validation

- **Catalog build:** duplicate IDs, invalid labor/worker counts, malformed future I/O
- **Building bindings:** unknown/duplicate supported ops, invalid defaults, unknown input items
- **Runtime:** selected operation must exist in catalog and be supported by the building;
  missing definitions surface as `ProductionValidationIssue::MissingOperationDefinition` or
  `OperationalLimitingFactor::InvalidOperation` — no silent substitution

### Integration

- `BuildingOperationParams` carries `&OperationCatalog` for stepping and commands
- `set_production_selected_operation` and `cycle_production_selected_operation` validate
  through `validate_operation_selection`
- Dev inspector production panel shows supported/selected/default operations and validation
- Starter catalog ships mine_iron, mine_stone, pump_water, smelt_iron, bake_bread,
  grow_prispods, research for validation and selection only (no gameplay outputs)

## Deferred

| Phase | Scope |
|-------|--------|
| EP4 | Role-tagged building inventory bindings |
| EP5 | Recipe execution, item consumption/production, terrain sampling |
| Future | Fuel, hauling, warehouses, power, tools, skills, AI research effects |

## Rejected models

- Runtime-owned `OperationDefinition` copies in policy or state
- Worker-owned production definitions
- Implicit building compatibility inferred from building category or name
- String-based operation IDs without a typed newtype
- Placeholder booleans where typed future requirement refs are needed
- Serializing the catalog in world saves

## Consequences

- Content authors add operations to `OperationCatalog` and wire buildings explicitly.
- Save/load remains stable across catalog renames if IDs are preserved.
- EP5 can execute `inputs`/`outputs` without changing policy/state layout.
- EP4 inventory roles attach to operations without duplicating catalog fields on runtime state.
