# ADR-110: Generic Production Execution Engine (EP5)

## Status

Accepted

## Context

EP1–EP4 established production runtime progress, operation catalog definitions, and
role-tagged building inventory bindings. Progress could complete cycles without mutating
inventory. EP5 must convert completed production cycles into authoritative inventory
mutations through one generic, data-driven execution path.

## Decision

### Execution ownership

- **Building production runtime** (`step_workstation_operation`) owns when a cycle completes.
- **`execute_production_cycle`** (`src/world/building/operation/execute.rs`) owns how item
  I/O is validated and applied.
- **Inventory runtime APIs** (`consume_stack_item`, `place_stack_first_fit`) remain the only
  mutation surface.

### Generic pipeline

On each progress threshold crossing:

1. Resolve `OperationDefinition` inputs/outputs through `BuildingInventoryBindingId` → `InventoryId`.
2. Validate inputs (quantity available per authored definition).
3. Validate outputs (cumulative simulated placement per destination inventory).
4. Snapshot affected inventories.
5. Consume all inputs, then produce all outputs.
6. Roll back all snapshots on any failure.
7. Increment `completion_count` only after successful execution.

No operation-specific branches. `OperationEffectKind` outputs are skipped (no inventory mutation).

### Blocking

Execution failures map to existing `OperationalLimitingFactor` values:

| Failure | Factor |
|---------|--------|
| Missing binding | `InvalidInventoryBinding` |
| Missing inventory record | `MissingInventory` |
| Insufficient items | `MissingInput` |
| Output cannot fit | `OutputBlocked` |
| Invalid/missing I/O definition | `InvalidOperation` |

Blocked execution leaves progress at/above the completion threshold and does not increment
`completion_count`.

### Atomic guarantees

- Pre-validate inputs and outputs before any mutation.
- Inventory snapshots restore prior state on failure.
- No partial consumption across a single cycle.

### Catalog content

Starter operations exercise the runtime without terrain logic:

- **Mine Iron** — no inputs; `iron_ore` → `primary_output`
- **Smelt Iron** — `iron_ore`×2 + `coal`×1 → `iron_bar` + `slag`
- **Bake Bread** — `flour` + `water` + `fuel` → `bread`

## Rejected designs

| Design | Reason |
|--------|--------|
| Operation-specific runtime (`if operation == mine_iron`) | Violates generic engine goal; blocks future content scaling |
| Extraction-specific execution in EP5 | Terrain sampling/depletion deferred; Mine Iron is catalog-only |
| Partial inventory mutations on failure | Risks item loss; violates transactional expectation |
| Incrementing `completion_count` before execution | Allows “ghost completions” without production |
| Parallel error-reporting system | Duplicates `OperationalLimitingFactor` |

## Consequences

- Production buildings require runtime inventory bindings for item I/O.
- Dev inspector exposes resolved I/O, inventory contents, and blocking via `assess_production_execution`.
- EP6+ can add terrain extraction by extending validation, not replacing execution.

## References

- ADR-107 (production runtime)
- ADR-108 (operation catalog)
- ADR-109 (inventory bindings)
- ARCHITECTURE.md — Building Production Execution (EP5)
