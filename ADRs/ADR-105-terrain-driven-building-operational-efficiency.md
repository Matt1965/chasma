# ADR-105: Terrain-Driven Building Operational Efficiency and Output-Rate Integration

## Status

Accepted (TF5)

## Context

ADR-104 (TF4) computes and caches `terrain_efficiency_basis_points` beneath building operational footprints, previews expected output in Build Mode, and exposes suitability in the selected-building panel. The worker task seam (`TaskType::OperateWorkstation`) was intentionally a no-op after arrival—production output scaling was deferred to TF5.

## Decision

### Operational efficiency query

- `building_operational_efficiency()` is the single authoritative query for runtime output rate.
- Factors combine as fixed-point basis points: `final = terrain × worker × condition × other` (TF5 activates terrain only; other factors default to 100%).
- Result includes `can_operate`, per-factor efficiencies, `limiting_factor`, and `assessment_revision`.
- Uses `ensure_building_terrain_assessment()` — no per-tick field resampling when cache is valid.

### Output progress (not task duration)

- Terrain scales **output rate**, not worker task duration or travel time.
- `ProductionProgress` uses fixed-point units (`1_000_000` = one completion threshold).
- `BASE_OPERATION_PROGRESS_PER_TICK = 10_000` at 100% efficiency → 100 ticks per completion unit.
- `scale_progress()` applies efficiency with deterministic rounding; remainder retained across ticks.

### Workstation labor integration

- `step_workstation_operation()` runs inside `step_all_worker_tasks` when `BuildingOperationParams` are supplied.
- `OperateWorkstation` tasks transition to `TaskState::BlockedWaiting` when `can_operate = false`; progress is zero.
- `BuildingOperationStore` holds per-building fractional progress and completion count.
- Simulation wires params via `BuildingSimulationParams` → `run_simulation_tick`.

### UI and dev probes

- Selected building panel shows terrain output, final output rate, limiting factor, and operation progress.
- Build Mode labels preview line **Expected Output Rate**.
- Dev inspector snapshot includes optional operation probe fields; gizmo building commits call `mark_dirty` on terrain assessments.

## Consequences

- Future worker/condition/input factors plug into `combine_output_efficiency` without changing task duration.
- Item spawn / recipe output remains a future seam—TF5 records completion count only.
- Transform edits must invalidate assessments (`mark_dirty`) so efficiency recomputes on next query.
- Tests assert deterministic tick parity at 50%/100%/150% efficiency and preview=runtime efficiency match.
