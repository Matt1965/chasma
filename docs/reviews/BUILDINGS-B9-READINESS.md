# Building System Readiness

B9 completion review — July 2026.

## Verification Results

| Check | Result |
|-------|--------|
| `cargo fmt --check` | Pass (after format) |
| `cargo test --lib` | 1060 passed |
| `cargo test --lib --features dev` | 1171 passed (includes scene v5 tests) |
| `cargo check --lib` | Pass |
| `cargo check --lib --features dev` | Pass |
| `cargo check --features terrain-import` | Not re-run this session (no terrain-import changes) |
| Manual dev build checklist | Deferred to operator (13-point list in B9 spec) |

## Catalog and Data Pipeline

- Dev builds load `BuildingCatalog` / `FootprintCatalog` from Excel import (`world/mod.rs`).
- Scene capture writes v5 with buildings, doors, tasks, space/unit state, and ID counters.
- Missing building definitions fail load with `BuildingRestoreError` / `SceneApplyError`.

## Authoritative Runtime

- `BuildingRecord` remains sole instance truth on `WorldData`.
- `dev_clear_units_and_doodads` clears tasks, doors, and space registry.
- No ECS authoritative building state introduced.

## Occupancy and Passability

- `rebuild_building_world_indexes` is the canonical post-load rebuild entry point.
- Round-trip test `building_scene_round_trip_preserves_state` confirms occupancy cell count matches after scene apply.
- Incremental vs full rebuild equivalence covered in `rebuild::tests`.

## Build Mode and Validation

- B4 build mode unchanged in B9; polish items (ghost cache, repeat-placement toggle) deferred pending UX profiling.

## Construction / Destruction / Ruins

- Worker labor-only construction preserved (ADR-085).
- `sync_construction_tasks` runs after load rebuild; restored tasks not duplicated.

## Spaces / Portals / Visibility

- Interior activation + door state restore unchanged from v4 path; uses runtime `InteriorProfileCatalog`.
- `current_space_id` on units persists in scenes.

## Interiors / Doors / Children

- Door snapshots restored by `definition_key` after interior activation.
- Child doodad/building ID lists persist on `SceneBuildingRecord`.

## Interactions / Tasks

- `SceneTaskRecord` round-trips construction tasks.
- `SceneUnitState::Working` preserved in v5.
- Task validation rejects unknown buildings/units before apply.

## Persistence

- Scene v5: tasks, counters, Working state.
- Atomic rollback on validation failure (existing `DevWorldEntityBackup`).
- ID allocators restored via `dev_restore_building_runtime_counters`.

## Performance Findings

- No hot-path regressions identified in this pass.
- Unified rebuild API removes duplicate occupancy rebuild in scene load path.
- Ghost-validation cache, portal direct lookup, and per-chunk occupancy micro-opts deferred until profiling evidence.

## Underground Readiness

- Architecture supports negative/support elevations and portal-linked spaces (ADR-083/084).
- No underground gameplay content or excavation editor shipped in B9.
- Full below-surface fixture test deferred; model does not block future content.

## Dev Tooling

- Scene load uses runtime catalogs from panel/actions.
- Dev inspector compile fixes (SpaceId, NavigationWaypoint, overlay focus).
- Building/task diagnostics overlay expansion deferred.

## Remaining Deferred Work

- Production world save (Phase 7) atop same structures
- Performance profiling suite for dense settlements
- Build mode UX polish (repeat placement, rejection hierarchy)
- Runtime LOD tiers beyond residency gating
- Asset validation CLI for building GLB/scene conventions
- Underground dev fixture scene

## Known Limitations

- Completed/canceled tasks are serialized if present; prune on load may remove invalid ones.
- Door/space graph not fully serialized — rebuilt via interior activation + door snapshots.
- `verify_instance_indexes` gated to dev/test in `rebuild_building_world_indexes`.

## Recommendation

**Ready with non-blocking caveats** for economy/resource development.

Building architecture is frozen per ADR-086. Proceed with hauling/economy on the established `TaskStore` and `BuildingRecord` seams; address performance polish and production save when Phase 7 begins.
