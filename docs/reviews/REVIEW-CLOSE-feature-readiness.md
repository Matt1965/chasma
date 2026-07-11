# REVIEW-CLOSE: Feature Readiness Report

**Date:** July 2026  
**Scope:** Final cleanup after audit passes A1–B6  
**Recommendation:** **Ready for feature development** (with listed non-blocking caveats)

---

## Verification

| Check | Result |
|-------|--------|
| `cargo test --lib` | 874 passed |
| `cargo check` | Clean |
| `cargo check --features dev` | Clean (0 warnings after REVIEW-CLOSE) |
| `cargo check --features terrain-import` | Clean |
| `cargo fmt --check` | Run at close |
| `cargo clippy --lib --features dev -D warnings` | **Not clean** — ~129 pre-existing style lints (redundant closures, manual range contains, etc.); deferred as P4 — not rustc `unused`/`dead_code` warnings |
| Dev release startup | Schedule initializes; no panic (post B6 schedule fix) |

---

## Warning Status

| Metric | Value |
|--------|-------|
| Warnings before REVIEW-CLOSE | ~38 (`cargo check --features dev`) |
| Warnings after REVIEW-CLOSE | 0 (`cargo check --features dev`, lib) |
| Intentional allowances | `OPTIONAL_COLUMNS` in weapon schema (`#[allow(dead_code)]` — documented Excel seam) |

---

## Cleanup Completed

### Intent dispatch reporting

- `IntentDispatchReport::rejected()`, `total()`, `rejected_reason_counts()`
- Semantics: each intent → exactly one of **Applied** / **Ignored** / **Rejected**
- **Applied:** selection/command state changed or valid order issued
- **Ignored:** intentionally irrelevant (empty frame, NoOp plan, harmless duplicate)
- **Rejected:** validation/unavailability failure (hold/interact placeholders, malformed target, etc.)

### APIs removed / privatized / test-gated

- Removed `record_kill_attribution` thin wrapper (use `WorldData::record_kill_attribution`)
- Removed `build_chunk_mesh_finalized` (production uses `seam_weld_heights` + `build_chunk_mesh_scaled`)
- `units_by_owner`, `range_check_for_units`, `standoff_center_distance_matches_weapon_range` → test/`pub(crate)` only
- Removed unused dev/registry helpers; trimmed combat re-exports
- Moved HUD tooltip helper into test module; removed unused style constants

### Panic paths corrected

- **Projectiles:** stale/missing target at step/impact → reject trace, no `expect`
- **Combat engagement:** `combat_pair()` replaces `unwrap()` after validation races

### Validation consolidated

- `validate_loaded_chunk` delegates span/spacing/samples to `validate_heightfield_against_config` only
- Single tolerance authority in `src/world/terrain/contract.rs`

### Diagnostic consistency

- Interaction: `SlopeWalkability::Unavailable` → `BlockedArea` ("Terrain unavailable"), not silent `None`
- Interaction blocking uses `is_position_blocked_by_doodads` (fail-closed on query errors)
- Movement/navigation unchanged (already fail-closed)

### Documentation

- `ROADMAP.md` — review closure status table
- `DESIGN.md` — game design direction (combat, progression, AI, settlement, food)
- ADRs 069–073 — accepted design directions for future features
- `Chasma Design.xlsx` — `Weapons` sheet added (data fix for dev catalog)
- ADRs updated in prior B6 pass (067, 031, 049)

---

## Remaining Deferred Items (P3/P4)

- Residency-delta terrain streaming optimization
- A* allocation pooling / nav grid caching
- Health-bar batching/instancing optimization
- EXR/biome orientation golden fixtures
- `loads_committed_sample_world` slow-test investigation (~60s+)
- Settings extraction for remaining magic numbers
- Production Excel wiring without `feature = "dev"`
- Combat `.expect` in test-only helpers (acceptable)

---

## Architecture Status

| Invariant | Status |
|-----------|--------|
| `WorldData` authoritative | Confirmed |
| ECS derived presentation | Confirmed |
| Fixed simulation tick (`run_simulation_tick`) | Confirmed |
| Intent → command pipeline | Confirmed |
| Catalogs data-driven (Excel in dev) | Confirmed |
| Debug/dev isolated (`feature = "dev"`) | Confirmed |
| Combat/projectile authoritative + deterministic iteration | Confirmed |

---

## Recommendation

**Ready for feature development.**

Non-blocking caveats:

1. Dev catalogs require valid `Chasma Design.xlsx` sheets (`Units`, `Weapons`, `Doodads`) — no silent starter injection in production.
2. Hold/Interact commands reject until U13+ implementation.
3. Some command/movement reason variants reserved for future debug layers (documented seams).
