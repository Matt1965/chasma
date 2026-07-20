# ADR-111: Terrain-Influenced Extraction Operations (EP6)

## Status

Accepted

## Context

EP1–EP5 established building production runtime, operation catalog definitions, role-tagged
inventory bindings, and generic production execution. Mine Iron existed as catalog content but
terrain did not influence extraction efficiency. EP6 must deliver the first complete
extraction chain — Mine Iron, Mine Stone, Pump Water — by integrating permanent Terrain Fields
into operational efficiency while keeping production execution generic.

## Decision

### Terrain Fields are permanent environmental potential

- Terrain Fields store **potential**, not remaining resources.
- Extraction is **continuous**; nothing is depleted, consumed, or mined out.
- Buildings sample fields through cached `BuildingTerrainAssessment`; workers never interact
  with terrain fields directly.

### Operation terrain requirements

`OperationDefinition.terrain_requirements` declares which fields an operation cares about for
efficiency (e.g. Mine Iron → iron field). Requirements are validated at catalog load time.

Operation-scoped efficiency (`terrain_efficiency_for_operation`) filters the cached building
assessment to the selected operation's requirements. Production steps never resample terrain.

### Operational efficiency integration

`building_operational_efficiency` accepts an optional selected `OperationDefinition`. When the
operation declares terrain requirements, efficiency uses operation-scoped terrain from the cached
assessment. The existing formula remains:

```
Base Labor × Worker Labor × Operational Efficiency → Progress
```

Terrain modifies operational efficiency only. No extraction-specific progress formulas.

### Generic production execution unchanged

`execute_production_cycle` remains inventory-only. Extraction operations have no inputs; outputs
flow through normal binding resolution, validation, and atomic placement.

### Invalid terrain behavior

Buildings may be placed on unsuitable terrain. The building is not destroyed and placement is
not blocked. Poor or absent field values reduce operational efficiency to zero through existing
limiting factors (`TerrainAverageBelowMinimum`, `TerrainResponseZero`, etc.).

### Assessment lifecycle

Terrain assessment occurs on:

- Building placement
- Terrain changes (dirty marking)
- Explicit rebuild (dev tools, move)
- First efficiency query when cache is cold

Production consumes cached assessments only.

### Starter content

| Operation   | Terrain requirement | Output    | Building        |
|-------------|---------------------|-----------|-----------------|
| Mine Iron   | iron (30% avg min)  | iron_ore  | iron_mine       |
| Mine Stone  | stone (25% avg min) | stone     | stone_quarry    |
| Pump Water  | water (20% avg min) | water     | water_well      |

Building compatibility is declared via `supported_operations`, not inferred from names.

## Rejected designs

| Design | Reason |
|--------|--------|
| Finite ore nodes / deposit ownership | Conflicts with permanent-field model; adds depletion state |
| Terrain depletion / chunk exhaustion | Fields are immutable environmental data |
| Operation-specific extraction runtime | Violates EP5 generic execution; duplicates inventory logic |
| Workers mining terrain directly | Buildings own production; workers contribute labor only |
| Resampling terrain every production tick | Violates assessment cache architecture (ADR-104/105) |
| Storing remaining resources on Terrain Fields | Conflates potential with inventory |

## Consequences

- Extraction operations require both building field requirements (assessment) and operation
  terrain requirements (operation scope).
- Dev inspector shows terrain assessment summary, revision, stale flag, and KeyF refresh.
- Session terrain assessments are rebuilt after load; production `last_efficiency_revision`
  round-trips through save state and triggers reassessment when stale.

## References

- ADR-101 (terrain field authority)
- ADR-104 (building terrain assessment)
- ADR-105 (operational efficiency)
- ADR-110 (generic production execution)
- ARCHITECTURE.md — Building Production Extraction (EP6)
