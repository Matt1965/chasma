# ADR-085: Building Interactions, Tasks, and Construction Labor

## Status

Accepted (B8)

## Context

B5 introduced building lifecycle and temporary timed construction progression. B7 added interiors and doors. B8 must connect buildings to the general interaction and task path so construction advances only from worker labor, without economy, inventories, or production outputs.

ADR-072 describes future settlement automation; this ADR implements the authoritative task seam consumed by that future work.

## Decision

### Capability and interaction points

- `BuildingInteractionProfile` on `BuildingInteractionProfileCatalog` defines capabilities (`construction_site`, `workstation`, `door_control` seam) and authored `InteractionPointDefinition` entries (local offset, task type, enabled lifecycle states, capacity).
- Runtime identity: `(BuildingId, point_key)` reservations on `TaskStore`; no global interaction-point entities.

### Task system (`world/task/`)

- `TaskStore` on `WorldData` owns `TaskRecord`, unit assignments, and interaction-point reservations.
- Task types for B8: `ConstructBuilding`, `OperateWorkstation`.
- Targets: `TaskTarget::Building` or `InteractionPoint`.
- Priority: `PlayerAssigned` > `High` > `Normal` > `Low`; tie-break by priority, `created_tick`, `TaskId`.

### Construction labor

- `BuildingConstructionSettings::default().auto_timed_progress = false`.
- `step_all_worker_tasks` applies labor on fixed ticks:
  `progress_delta = construction_speed × delta_seconds / build_time_seconds`.
- Multiple workers may share one building task; each reserves a distinct interaction point; `unit_task` maps each worker to the task.

### Player flow

- `query_world_interaction` classifies nearby buildings as `ConstructionSite` or `Workstation`.
- `InteractionOrderPlan::ConstructBuilding` / `OperateWorkstation` issued from terrain clicks when workers are selected.
- Client dispatcher calls `assign_construct_building_task` / `assign_operate_workstation_task` (player-directed only in B8).

### Unit orders

- `UnitOrder::Work { task_id, target }` paths to the reserved interaction point; `UnitState::Working { task_id }` while in range.
- Animation does not apply labor; missing clips fall back to idle/locomotion.

### Workstation foundation

- Completed workstations accept `OperateWorkstation` tasks; no recipe inputs/outputs in B8.

### Cancellation and hooks

- Unit death, building destruction, player orders, and invalid access cancel tasks and release reservations via `cancel_unit_task` / `prune_invalid_building_tasks`.
- `place_player_building` calls `sync_construction_tasks`; `destroy_building` prunes tasks.

## Consequences

- ADR-082 timed auto-progress is dev/test-only via `BuildingConstructionSettings::dev_auto_timed()`.
- ADR-042 gains `ConstructionSite`, `Workstation`, and `InteractionTargetRef::Building`.
- Economy, hauling, recipes, and autonomous schedulers remain future phases on this task seam.

## Non-goals (B8)

No items, inventories, production outputs, worker skill curves, or complex priority schedulers.
