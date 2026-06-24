# ADR-049: System Cleanup and Production Freeze Pass

# Status

Accepted (U-REVIEW1 — pre-combat baseline freeze)

# Context

Before U13+ combat and gameplay expansion, the simulation layer must be
**production-clean**: no hidden hardcoded catalogs, no silent fallback injection,
and strict separation between dev tooling and runtime authority.

U-series features introduced dev preview paths, test fixtures, and temporary
scaffolding. Some patterns (starter catalogs, command fallbacks, dev biome
imports) risk leaking into production behavior if left ungated.

# Decision

## Rationale

Remove or gate all temporary scaffolding so the runtime baseline is:

1. **Data-driven** — unit and doodad definitions come from Excel import only.
2. **Fail-closed** — missing catalog data logs/errors; no silent dummy injection.
3. **Dev-isolated** — preview spawns, biome PNG import, dev catalog resolution,
   and inspector tools run only under `feature = "dev"` or explicit dev-mode checks.
4. **Test-explicit** — in-memory starter fixtures exist only under `#[cfg(test)]`.

## Catalog purity

| Build | UnitCatalog / DoodadCatalog source |
|-------|--------------------------------------|
| Production (`default` features) | Empty via `Default` → `starter_definitions()` returns `Vec::new()` |
| Dev (`feature = "dev"`) | Excel import at startup; empty + warn on failure |
| Unit tests (`cfg(test)`) | `starter_definitions()` fixtures for isolated tests |

`starter_definitions()` is **not** exported from the crate root outside tests.

## Fallback elimination

| Removed / gated | Replacement |
|-----------------|-------------|
| Hardcoded starter catalogs in non-test builds | Empty default; Excel in dev |
| `build_command_plan_or_fallback_move` in public API | `build_command_plan` only; dispatcher returns `Ignored` on error |
| Dev preview unit grid spawn | `#[cfg(feature = "dev")]` in units plugin |
| Dev biome PNG import | `#[cfg(any(test, feature = "dev"))]` exports only |
| Silent Excel import recovery with injected defs | Warn + empty catalog |

Intentional **non-simulation** fallbacks retained with explicit scope:

- Terrain mesh **albedo** height-gradient when no sidecar (render-only, ADR-013).
- Steering **separation** numeric epsilon away from coincident neighbors (simulation correctness).
- Command **placeholders** (`HoldPosition`, `AttackMove`) documented until U13+ combat.

## Architecture freeze statement

> **Simulation layer is now production-clean and data-driven.**

Meaning:

- No hidden dev logic in core movement/pathfinding/steering/formation systems.
- No temporary scaffolding in the production runtime path.
- No hardcoded gameplay unit/doodad definitions outside `#[cfg(test)]` fixtures.
- `WorldData` remains the sole simulation authority; ECS is render mirror only.

## Verification

[`src/review/production_baseline.rs`](../src/review/production_baseline.rs) encodes
regression tests for empty-catalog behavior, fail-closed command building, and
deterministic generation with explicit test fixtures.

# Consequences

- Production builds require Excel import pipeline wiring before playable content.
- Unit tests must construct catalogs explicitly (`UnitCatalog::default()` in tests
  still resolves to fixtures under `cfg(test)`).
- Dev builds without a workbook get empty catalogs and explicit warnings.

# Non-goals

- No new gameplay systems.
- No performance refactors.
- No Dev Mode feature rework (gating/audit only).
