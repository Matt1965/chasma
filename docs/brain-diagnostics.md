# Brain diagnostics (dev-only)

Brain lives in **Debug** (`DevWindowId::Debug`) under **Brain diagnostics**. It answers:

> Why is this unit doing what it is doing?

## Views

- **Unit** — follows [`WorldSelectionState`](../../src/client/selection/mod.rs) primary unit. Empty when nothing is selected.
- **Settlement** — follows [`CameraSettlementContext`](../../src/client/settlement_context/mod.rs) focused settlement. Empty when camera focus does not resolve a settlement.

## Data sources (authoritative)

| UI section | Source |
|------------|--------|
| Current action / task | `UnitRecord`, `TaskStore`, `CombatState` |
| Authority | `TaskPriority::PlayerAssigned`, `StrategicTaskOrigin`, active combat only |
| Decision spine | `NeedEvaluationStore`, `StrategicTaskOrigin`, `TaskRecord`, `WorkerAssignmentReport` |
| Candidate race | `WorkerEvaluation.ranked_candidates` (SA7 step scoring) |
| Why not | `WorkerCandidateDiagnostic.block_reason` |
| Decision history | `BrainDecisionHistory` (dev resource; task/combat/activity transitions) |
| Need pressures | `NeedEvaluationStore` / `NeedSnapshot` |
| Intents | `SettlementIntentStore` / `SettlementIntentPlan` |
| Arbitration bars | `ArbitrationScoreBreakdown` on `SettlementIntent` |
| Rejected intents | `RejectedIntentCandidate` / `IntentRejectionReason` |
| Downstream | `BuildingIntentPropagationStore`, `TaskStore`, `WorkerAssignmentReport` |

## Intentionally omitted

- Rest/idle utility (not an existing authority)
- Inferred or recomputed scores in UI
- Per-tick state dumps
- Settlement directly “choosing” a worker (SA7 owns assignment)

## Supported history transitions

Task assignment change, combat enter/exit, working activity change. Max 8 entries per unit.
