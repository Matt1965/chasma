# ADR-116: Settlement Runtime State (SA1)

## Status

Accepted

## Context

ADR-115 defined Settlement AI architecture around a single weighted-need arbiter. Before needs,
arbitration, or task assignment can exist, settlements need a persistent **memory** object:
identity-adjacent configuration that survives save/load and from which all future SA analysis can be
rebuilt.

EP9 already stores production planner config (`SettlementProductionPlanner`). SA1 introduces the
general **`SettlementState`** that every autonomous group owns — towns, hives, packs, camps, and
player settlements alike — without implementing any AI behavior.

## Decision

### SettlementState is authoritative persistent truth

`SettlementState` lives on `WorldData` via `SettlementStateStore`, keyed by `SettlementId`. It
answers:

> What information about this settlement survives saving and loading?

It does **not** answer what the settlement should do. No evaluation, planning, task generation, or
worker control runs in SA1.

### Parallel to SettlementRecord

`SettlementRecord` remains the treasury/anchor identity (ADR-093). `SettlementState` is a parallel
store for SA policy/memory. Creating a settlement ensures both (plus an EP9 production planner
entry).

### One runtime for all autonomous groups

Player settlements, AI factions, wildlife packs, and bandit camps use the **same** `SettlementState`
type. Differences are authored data (`SettlementKind`, policies, need targets), never separate
codepaths.

### Persistent vs derived (rebuild principle)

**Persisted:** kind, policies, need targets, modifiers, emergency flags, planner lifecycle
scheduling (enabled/paused/ticks/interval/diagnostics config), extension seams.

**Never persisted / never authoritative:** need pressures, priority values, response graphs, planner
caches, temporary diagnostics, the runtime `dirty` flag.

**Rebuild principle:** after load it is always valid to discard planner caches, diagnostics, and
derived analysis and regenerate them deterministically from `SettlementState`. Import always sets
`planner.dirty = true`.

### Policies and targets are intent only

`SettlementPolicies` and `NeedTarget` values are configuration. They do not execute. Shortages and
pressures are SA2+.

### Modifiers and emergencies are storage seams

`SettlementModifier` and `SettlementEmergencyState` provide obvious places for faction/player/
scenario/weather/event adjustments and emergency flags. No system responds to them in SA1.

### Scene format v13

`SceneSettlementStatePersistence` captures `SettlementStateSaveState`. Pre-v13 scenes load with
default states ensured for every `SettlementRecord`.

## Rejected designs

- **Separate player settlement runtime** — one simulation model; player sets policies only.
- **Separate wildlife runtime** — packs/herds are `SettlementKind` data, not parallel systems.
- **Planner-owned persistent data as settlement truth** — EP9 planner remains a production Response
  path; SettlementState owns general SA memory.
- **Worker-owned settlement state** — workers stay simple (tasks/inventory/location only).
- **Building-owned strategic state** — buildings keep operation policy/state; strategy lives on the
  settlement.
- **Persisting derived analysis** — violates rebuild principle.

## Consequences

- Dev inspector shows SettlementState summary alongside EP9 planner diagnostics.
- `mark_settlement_planner_dirty` also dirties SettlementState lifecycle (still no evaluation).
- SA2 (ADR-117) reads targets/policies/modifiers/emergencies from SettlementState and writes only
  transient `NeedSnapshot` caches on `NeedEvaluationStore`.

## References

- ADR-115, ADR-114, ADR-093, ADR-072
- ARCHITECTURE.md Settlement AI section
