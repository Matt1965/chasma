# ADR-068: Environment Singleton and Input Ownership (REVIEW-B5)

# Status

Accepted (REVIEW-B5 — cross-system correctness cleanup)

# Context

REVIEW-B5 identified:

- `single_mut()` on environment lights could silently skip updates or panic in edge cases
- `KeyT` was bound to both dev spawn affiliation and time-of-day toggle
- Player-control systems relied partly on plugin registration order
- Doodad catalog IDs derived inconsistently from workbook `Name` vs slug conventions in tests/dev tools
- Interaction resolver still mapped resource nodes to move orders and used `UnitId(0)` fallback

# Decision

## Environment singleton policy

| Count | Behavior |
|-------|----------|
| 0 | No panic; directional update skipped (`Missing`) |
| 1 | Normal update (`Single`) |
| >1 | No panic; update refused; dev warning logged (`Duplicate`) |

[`EnvironmentLightingInitialized`](../src/environment/lighting.rs) prevents duplicate spawns on repeated startup.
[`EnvironmentSingletonReport`](../src/environment/debug.rs) validates at PostStartup (dev).

## Keyboard binding ownership

| Binding | Owner |
|---------|-------|
| F12 | Dev Mode toggle |
| Space / Shift+Space | Simulation pause / step |
| Shift+T | Dev spawn affiliation (dev only, panel not hovered) |
| `[` `]` `,` `.` | Time-of-day hour/speed (World Tools tab only) |
| Cycle/pause/presets | Dev panel buttons (primary) |

Canonical reference: [`src/input/bindings.rs`](../src/input/bindings.rs).

## Player-control ordering (Update frame)

```text
RuntimeSyncSystems (doodad/unit/projectile render mirrors)
  → simulation tick + trace flush
  → sync_selection_policy_state
  → DevModeSystems (before input collect)
  → update_player_hud_hover_state (before input collect)
  → ClientIntentCollectSystems
  → HUD command/squad clicks (before dispatch)
  → ClientIntentDispatchSystems
  → flush_intent_dispatch_trace
  → GameplayPresentationSystems
  → DebugPresentationSystems
```

Declared via shared [`SystemSet`](../src/player/plugin.rs) at composition root — not plugin insertion order alone.

## Catalog identifier policy

1. Optional workbook column `Definition ID` / `Doodad ID` (preferred when present)
2. Otherwise [`normalize_doodad_definition_id`](../src/data_import/schema.rs) slugs display `Name`
3. Machine ids: lowercase alphanumeric + underscore
4. `Name` → display name; `Description` → display fallback
5. Import rejects duplicate normalized ids

Weapon dev import failure now yields **empty** catalog (aligned with doodad/unit), not starter injection.

## Interaction/command consistency (B3 carryover)

- Resource nodes / interactable objects → `InteractionOrderPlan::NoOp`
- Attack without unit target → `NoOp` (no `UnitId(0)` fallback)

# Consequences

**Benefits:**

- Recoverable environment states do not panic
- One owner per critical global binding
- Explicit cross-plugin client/control ordering
- Deterministic doodad ids across import, dev tools, and tests

**Costs:**

- Slugified display names become ids when `Definition ID` column absent
- Time-of-day keyboard presets removed (panel-only)

# References

- ADR-038 (intent pipeline)
- ADR-041 (command layer)
- ADR-052 (environment)
- ADR-065 (simulation tick orchestrator)
