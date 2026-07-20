# ADR-115: Settlement AI Architecture (SA Foundation)

## Status

Accepted (architecture foundation — not an implementation phase)

## Purpose

This ADR defines the **authoritative long-term architecture for Settlement AI (SA)**. It is the
architectural foundation that every future SA implementation phase (SA1–SA9) must conform to.

It does **not** implement systems. It defines ownership, responsibilities, the decision pipeline,
planning cadence, priority propagation, the event model, knowledge model, and the recommended
implementation roadmap.

Where this document and an accepted implementation ADR disagree about *what exists today*, the
implementation ADR wins for current behavior. Where they disagree about *intended structure*, this
document wins and the implementation should be reconciled toward it.

This ADR preserves the decisions in ADR-072 (settlement automation philosophy) and ADR-107–ADR-114
(EP1–EP9 building production, operations, inventories, execution, terrain extraction, hauling,
production chains, and the production planner). Nothing here rewrites those systems; it defines the
layers that sit **above** them.

---

## 1. Design Goal

Settlements should feel like **living civilizations**. The player should observe shortages,
expansion, specialization, recovery, adaptation, mistakes, and long-term planning — all **emergent**,
never scripted.

The mechanism is deliberately **not** a monolithic settlement brain, and equally **not** a growing set
of per-domain decision engines full of conditionals. It is a single idea applied uniformly:

> The settlement continuously computes a set of **weighted needs**. One generic **arbiter** weighs
> them and services the most pressing unmet need by dispatching that need's **data-defined response**.
> The settlement fixes itself because needs re-weigh every cycle — not because anyone wrote rules for
> each situation.

Emergence comes from needs competing over time, not from bespoke logic per resource, per building, or
per emergency. Adding a new behavior (Defense, Medicine, Research) is normally a **data entry** — a
need with a weight and a response mapping — not new decision code.

This mirrors the existing production planner (ADR-114): it reads stock, compares to targets, and writes
only `BuildingOperationPolicy`. It does not execute, haul, or command workers. Every SA layer follows
the same discipline.

---

## 2. Authoritative Ownership

### 2.1 The hierarchy

```
World          — simulation authority, time, terrain, factions, all persistent records
  └─ Faction   — cross-settlement coordination (need-weight/constraint nudges only; optional layer)
      └─ Settlement — strategy: weighted needs, one arbiter, data-defined responses, emergencies
          └─ Building — operations, capabilities, local demand, operational state
              └─ Task — a single unit of executable work (shared pool)
                  └─ Worker — executes exactly one task at a time
```

**Direction of authority is strictly downward.** Each layer may read the layer(s) below to make
decisions, and writes intent to the layer directly beneath it. No layer reaches up to command a
layer above it. Workers never generate settlement strategy; buildings never decide expansion.

### 2.2 Adjustment to the expected hierarchy

The prompt's expected chain (World → Settlements → Buildings → Tasks → Workers) is **correct and
preserved**, with three clarifications that reflect how the codebase already works and must continue
to work:

1. **Tasks are a shared marketplace, not building-owned objects.** Today `TaskStore` lives on
   `WorldData` and is indexed by building *and* by unit (`task_store` in `src/world/data.rs`).
   Buildings, settlement directives, and emergencies all *generate* tasks into this shared pool.
   Workers *claim* from it. "Buildings own operations" remains true; "buildings own the worker" does
   not. This keeps a single assignment surface rather than N per-building schedulers.

2. **An Assignment layer sits between Tasks and Workers.** This is the one structural gap in the
   current code: task assignment is entirely **imperative** today (`assign_construct_building_task`,
   `assign_operate_workstation_task`, `assign_hauling_task`), with no autonomous scheduler. The
   architecture reserves a distinct **Assignment layer** (SA7) that matches available tasks to
   available workers by priority and eligibility. It is a layer, not a property of buildings or
   workers.

3. **A Faction layer may sit above Settlements** (SA9). It coordinates trade, military, expansion, and
   technology by nudging *need weights and constraints* on member settlements (the same lever the
   player and any future top-level AI use — see §5). It never owns buildings, tasks, or workers, adds
   no new decision path, and settlements remain fully functional with no faction present.

### 2.3 Data-first ownership

Per ARCHITECTURE.md Principle 6 and the existing implementation, **all persistent SA state lives on
`WorldData`**, never as the source of truth in ECS entities or UI. Current authoritative stores that
SA builds on:

| Store (`WorldData`) | Owns |
|---|---|
| `settlement_store` | Settlement records, treasuries, building membership index |
| `production_planner_store` | Per-settlement `SettlementProductionPlanner` config (ADR-114) |
| `building_production` | `BuildingOperationPolicy` (intent) + `BuildingOperationState` (truth) |
| `building_inventory_bindings` | Role-tagged inventory channels per building |
| `task_store` | All work tasks (Construct, Operate, Haul) + unit/building indexes |
| `hauling_requests`, `inventory_reservations`, `logistics_endpoint_index` | Logistics runtime (EP7) |
| `inventory_store`, `item_instance_store` | Container contents and item identity |

New SA state (need definitions/weights, directive nudges, emergency state) will be added as **new
stores on `WorldData`**, following the same pattern: persistent config is authoritative; derived
analysis (pressure values, arbitration results) is recomputed.

---

## 3. Responsibilities

Each layer has a single responsibility. Overlap is the primary failure mode to avoid.

### 3.1 World

- Global simulation authority and the fixed tick orchestrator (`run_simulation_tick`, ADR-065).
- Time, terrain (heightfields + Terrain Fields), occupancy, spaces.
- Ownership of all persistent records (the stores above).
- Factions (identity, ownership, diplomacy state) as data.

World does **not** make settlement-level economic decisions.

### 3.2 Settlement

- Derives **weighted Needs** (computed pressure values per category; §6).
- Runs the single **Need Arbiter** that weighs needs and selects the most pressing unmet one (§4).
- Dispatches that need's **data-defined Response** (a mapping to operations / build templates /
  postures), writing intent downward (`BuildingOperationPolicy`, construction/settlement directives).
- Owns the **emergency** state that temporarily reweights needs (§9), and the **directive** inputs
  (player/faction/AI) that nudge need weights and targets (§5).
- Owns population, stock targets, defense posture, and expansion state — all expressed as needs, not
  as bespoke plans.

A settlement is defined today by `SettlementRecord` (a treasury anchor building + affiliation) plus
its membership index in `SettlementStore`. SA extends this with need definitions/weights, directive
nudges, and the arbiter — it does not replace the settlement identity model. The existing production
planner (ADR-114) becomes one response path invoked by the arbiter, not a standalone brain.

### 3.3 Building

Buildings **report** and **execute**; they never decide strategy.

- Report: capabilities (`supported_operations`), requirements (`inputs`, `terrain_requirements`),
  health (`BuildingVitals`), capacity (inventory bindings), operational status
  (`BuildingOperationState`, `is_building_operational`).
- Execute: production cycles via `step_workstation_operation` → `execute_production_cycle`.
- Express **local demand** through logistics routes (`OutputSurplus` / `InputDeficit`), which spawn
  `HaulingRequest`s.

Buildings do **not** decide to expand, research, trade, or go to war. The split between
`BuildingOperationPolicy` (intent, written from above) and `BuildingOperationState` (truth, written by
execution) is the enforced boundary and must be preserved by all future SA systems.

### 3.4 Worker (unit)

Workers are intentionally simple. **Units are workers** — there is no separate worker entity. A unit
becomes a worker when it holds a task (`TaskStore.unit_task` / `UnitState::Working { task_id }`).

A worker knows only:

- its current task (`task_id`) and that task's target,
- its reservations (interaction-point slot; inventory reservations for hauls),
- its inventory (`inventory_id`) and carried cargo,
- its location and space (`placement`, `current_space_id`).

A worker must **never** know settlement strategy, economic goals, production graphs, or expansion
plans. Worker capability is data (`UnitWorkCapabilities`: `can_construct`, `construction_speed`,
`can_operate_workstation`), resolved from the catalog, not settlement context.

---

## 4. Decision Pipeline

There is **one** decision mechanism, applied uniformly. It is not a stack of domain planners; it is a
single weigh-and-respond loop plus the generic layers that carry its output down to workers.

```
Directive nudges            (player / faction / future AI adjust need weights + targets — §5)
      │  feed into →
      ▼
Needs                       (computed weighted pressure per category: Food, Defense, Housing,
      │                      Growth, ... — §6)
      │  emergency reweighting (§9) →
      ▼
Need Arbiter (SINGLE)       (generic: sort needs by pressure; pick the most pressing unmet one(s)
      │                      within a per-cycle budget)
      │  looks up the need's data-defined Response →
      ▼
Response (DATA)             (per need: which operations satisfy it, which build template, which
      │                      posture — a data mapping, not per-need code)
      │  writes intent →
      ▼
Building Intent             (BuildingOperationPolicy; construction/settlement directives)
      │  buildings + directives emit →
      ▼
Task Generation             (TaskRecords into the shared TaskStore + HaulingRequests)
      │  matched by →
      ▼
Worker Assignment           (Assignment layer: eligible free worker ↔ highest-priority task)
      │  issues UnitOrder::Work →
      ▼
Execution                   (step_all_worker_tasks / step_haul_worker_tasks / movement)
```

Two invariants make this scale without conditional sprawl:

1. **The arbiter is generic and singular.** It contains no per-need logic — only "compute pressure,
   sort, service the top under a budget." A new need never changes the arbiter.
2. **A Response is data, not a system.** A need declares *what satisfies it* (e.g. Food → the set of
   operations whose outputs are food items; Defense → a guard posture + wall build template). Applying
   a response reuses generic machinery (set `BuildingOperationPolicy`, emit a construction directive).
   Adding Medicine or Research is a data entry, not a new planner.

Current reality: only the **production** slice exists (EP9 planner reads `StockGoal`s and writes
policy). It is retroactively *one response path*. Needs, the arbiter, emergencies, general task
generation, and autonomous worker assignment do not exist yet. **Worker Assignment is the central
missing layer**, filled by SA5/SA6.

---

## 5. Directives (not a separate Goals layer)

Deliberate decision: **there is no separate strategic "Goals" planning layer.** In a needs-weighing
settlement, a need with a target and a weight already *is* the goal. Introducing a parallel Goals
engine above Needs would reintroduce the conditional sprawl this architecture exists to avoid.

Instead, higher-level intent is expressed as **Directives: nudges to need weights and targets.** A
directive says "raise the weight/target of need X" — nothing more. It never selects buildings, never
generates tasks, never adds a decision branch.

Design rules:

- A directive adjusts a need's **weight** (how hard the arbiter competes for it) and/or its **target**
  (e.g. target housing capacity, defense level, stock quantity). That is its entire expressive power.
- Directives are **coarse and declarative**: "prioritize defense," "grow toward 40 population,"
  "maintain 200 grain." Not "run bakery #4."
- The **three sources of directives are identical in mechanism**: the **player** (player settlements,
  §13), a **Faction** (SA9), and any **future lightweight top-level AI**. All three do the same thing —
  write need-weight/target nudges. This is the single seam where "smarter" high-level intent plugs in
  later without touching any lower layer.
- Directives are **persistent** and change slowly (long horizon). The instantaneous pressures they
  influence are recomputed (§6).

`StockGoal` (ADR-114) is retroactively a **directive**: an authored target + priority on a food/economy
need. SA does not add a `SettlementGoal` decision engine; it generalizes `StockGoal`-style targets
across all need categories.

---

## 6. Needs

Needs are the heart of the system. Define **Settlement Needs**: Food, Water, Housing, Defense,
Construction, Research, Luxury, Medicine, Population, Economy, and a permanent **Growth/Improvement**
need (§6.3).

Each need is a small **data definition**: how its pressure is computed, its base weight, its target
(directive-adjustable), and its **Response mapping** (§4) — the operations / build templates / postures
that satisfy it. Everything specific about a need lives in this data, never in the arbiter.

### 6.1 First-class object vs computed value — decision

**Need *pressure* is a computed value, cached in a per-settlement `NeedState` snapshot — not a
heavyweight simulation object and never an ECS entity.** Need *definitions/weights/targets* are
authoritative data.

Rationale (consistent with ARCHITECTURE.md "Data First" and the Scalability Rule):

- A pressure is **derived**: `pressure = f(current world state, target) * weight`. Deriving it (food
  stock vs population, housing capacity vs population, threat vs defense) is cheap and must not be
  duplicated as authoritative state that can drift.
- What **is** authoritative and persisted is the need **definition** (compute rule, base weight,
  Response mapping), its **directive-adjusted target/weight**, and **slow-moving derived state** that
  must survive reload for continuity (active emergency, smoothed threat estimate). Instantaneous
  pressure is recomputed.

| Thing | Nature | Persisted? |
|---|---|---|
| Need *definition* (compute rule, base weight, Response mapping) | Authoritative data | Yes |
| Need *target / weight nudges* (from directives) | Authoritative config | Yes |
| Need *pressure values* (`NeedState` snapshot) | Derived, cached on cadence/event | No (recomputed) |
| Emergency flags / smoothed estimates | Slow derived state | Yes (continuity) |

`NeedState` is a plain data snapshot on the settlement (like `PlannerDiagnostics` is for the planner):
recomputed by Need Evaluation (SA2) on cadence or on a relevant event, then read by the arbiter (§4).
This keeps needs cheap, drift-free, and scalable to hundreds of settlements.

### 6.2 Needs produce pressure; the arbiter acts

A need never enables a building or spawns a task directly. It produces a **weighted pressure**; the
single arbiter (§4) picks the most pressing and applies that need's data-defined Response. This is what
makes shortages and recovery emergent: rising food pressure outweighs other needs, the arbiter applies
the food Response (enable food operations), tasks are generated, workers fall through to them — with no
scripted "if starving then farm" branch anywhere.

### 6.3 Slow improvement is a permanent low-weight need

"Get better over time" is not a special mode — it is a **permanent Growth/Improvement need with a low
base weight.** It is always present but almost always outweighed by survival needs, so it is only
serviced when nothing urgent competes. A comfortable settlement therefore drifts into expansion and
upgrades on its own; a struggling one automatically stops improving and handles the emergency. Slow,
natural, self-correcting progress emerges from the same weighing loop — no separate "growth AI."

---

## 7. Priority Propagation

Priorities flow **strictly downward** and are expressed as numeric values already present in the code,
extended consistently:

```
Need weighted pressure (Food shortage)
   ↑ arbiter ranks it top →
Category priority (ProductionPriorityCategory::Food = high)
   ↓ Response writes →
BuildingOperationPolicy.priority (per producing building)
   ↓ Task Generation stamps →
TaskPriority / HaulingRequestPriority
   ↓ Assignment orders by →
Worker claims highest-priority eligible task first
```

Existing seams that this reuses (no new priority concept needed):

- `ProductionPriorityCategory` with default bands (Food/Medicine 255, Construction 192, General 128,
  Luxury 64) — already in `planner/types.rs`.
- `BuildingOperationPolicy.priority: u8`.
- `TaskPriority` (`PlayerAssigned` > `High` > `Normal` > `Low`).
- `HaulingRequestPriority`.

**Rule: workers never generate settlement priorities.** A worker cannot raise the importance of its own
task. Priority only ever originates from Need pressure (whose weights/targets come from directives) and
from player orders. Player orders enter at the top (as directive nudges) or as
`TaskPriority::PlayerAssigned` at the task layer; they preempt but do not corrupt the settlement's own
priority model.

---

## 8. Interrupts

**Interruption is priority-based and resolved at the task/assignment layer, not a separate global
system.** This is deliberately the simplest model that scales.

- A higher-pressure need outranks a lower one. "Raid interrupts construction interrupts luxury" is
  expressed as: Defense need pressure spikes → the arbiter services Defense, whose Response generates
  defense tasks at a priority above construction, which is above luxury. The Assignment layer naturally
  pulls workers to the highest priority available.
- **Worker-level preemption** already exists: issuing a player `Idle`/`MoveTo`/`Attack` order calls
  `cancel_unit_task`, releasing the worker. SA generalizes this: the Assignment layer may **preempt**
  an in-progress low-priority task when a sufficiently higher-priority task appears and no free worker
  exists, subject to a preemption threshold to avoid thrashing.
- Interrupts are therefore **local (per worker/task)** in mechanism and **driven by global priority**
  in cause. There is no separate "interrupt manager."

---

## 9. Emergencies

**Emergencies are temporary changes to settlement priorities — not separate AI systems.** This is a
firm architectural rule.

Examples: Starvation, Fire, Attack, Disease, Population collapse.

Model:

- An emergency is a **flag + parameters** on the settlement (persisted for continuity) raised by the
  Need Evaluation / event layer when pressure crosses a threshold (e.g. food pressure critical →
  `Starvation`).
- While active, it **reweights need pressures** (e.g. Starvation multiplies Food, suppresses Luxury;
  Attack multiplies Defense, pauses non-essential production). It flows through the *same* pipeline:
  reweighted needs → arbiter → Response → policy → tasks → workers.
- It clears when the triggering condition resolves.

No emergency has its own worker-command path, its own task executor, or its own building control. If an
emergency ever needs behavior that the normal pipeline can't express through priority, that is a signal
the pipeline is missing a capability — extend the pipeline, don't add a parallel emergency AI.

---

## 10. Time Horizons and Simulation Frequency

Different decisions belong to different horizons, and each horizon reevaluates on its own cadence.
**Nothing that can be event-driven or interval-driven should run every frame settlement-wide** (per
ARCHITECTURE.md performance philosophy and the existing planner's dirty-flag + interval design).

| Horizon | Scale | Owns | Cadence |
|---|---|---|---|
| Immediate | seconds / per tick | Worker task execution, movement, hauling steps | Every simulation tick |
| Short-term | seconds–minutes | Production stepping; task generation; assignment | Every tick (stepping) / on demand (generation) |
| Medium-term | minutes | Need evaluation; arbiter re-run; production Response | Interval (e.g. 60 ticks) + dirty/event |
| Long-term | minutes–hours | Construction / expansion Responses; defense posture | Longer interval + event |
| Strategic | hours–days | Directive nudges; faction coordination | Rare interval + event |

Per-layer reevaluation frequency:

- **Worker / Task execution:** every tick (`step_all_worker_tasks`, `step_haul_worker_tasks`,
  `step_all_unit_movement`).
- **Building operation stepping:** every tick, but only for buildings with assigned workers and an
  enabled policy.
- **Need evaluation + arbiter:** interval + dirty. The production planner already uses
  `replan_interval_ticks` (default 60) plus a `dirty` flag; the arbiter runs on the same cadence.
- **Faction / directives:** longest interval.

**Staggering:** settlement-level reevaluation must be **spread across ticks** (e.g. by settlement id
modulo an offset), so 100 settlements never all replan on the same tick. This is required for the
scalability targets and is owned by the **Planning Scheduler (SA10 / ADR-125)** — not by each stage
polling independently.

**Scheduler ownership:** the scheduler determines *when* planner stages execute. Planner stages remain
responsible for their own evaluation logic. The scheduler never performs planning itself (ADR-125).

**Runtime stage order** (after SA9): Emergency → Needs → Responses → Intent → Building Intent →
(EP9) → **Construction Planning → Strategic Tasks** → Worker Assignment. ConstructionPlans must
exist before SA6 so construct tasks attach to reserved sites (ADR-124).

---

## 11. Event Model

Prefer **event-driven reevaluation** over continuous scanning. The mechanism is a **dirty-flag /
invalidation** model rather than a heavy event bus, matching the existing planner. Time (fallback
cadence) is secondary to events (ADR-125).

Events that should invalidate/wake SA layers:

| Event | Wakes |
|---|---|
| Inventory changed in a settlement building | Need evaluation, arbiter (`mark_settlement_planner_dirty`) |
| Building completed / destroyed / ruined | Membership, need evaluation, arbiter |
| Worker died | Assignment (reclaim its task), affected task |
| Production blocked / surplus | Logistics (already: `sync_logistics_requests_from_assessment`) |
| Raid / threat detected | Defense need + emergency evaluation |
| Population change | Housing/food needs |
| Directive changed (player/faction) | Need evaluation, arbiter |

### Invalidation rules (strict)

Planner invalidation is **strictly downstream**. Earlier stages may invalidate later stages; later
stages must **never** invalidate earlier stages. Worker Assignment must never invalidate planning.
This prevents circular invalidation (full rules in ADR-125).

Implementation guidance:

- A `dirty` flag per settlement per planner (already present on `SettlementProductionPlanner`) is the
  baseline. `mark_settlement_planner_dirty(world, building_id)` **exists but is not yet called** — SA
  should wire it into the inventory/building mutation paths so the planner reacts to change instead of
  only polling on interval.
- Where a single flag is too coarse, use per-category dirty bits (food dirty vs defense dirty) so a
  food change doesn't force a defense replan.
- Events are **hints to reevaluate**, not commands. The authoritative decision is always the
  recomputation from `WorldData`; events only decide *when* to recompute. This keeps the system
  correct even if an event is missed (the interval fallback still fires).

**Avoid** any settlement-wide per-frame scan (all inventories, all buildings, all workers). The
existing `LogisticsEndpointIndex` (O(1) endpoint lookup) and settlement membership index are the model:
localized, cached lookups instead of global iteration.

---

## 12. AI Knowledge

Determine what a settlement actually *knows*.

**Decision:** initial implementation uses **perfect knowledge** (need evaluation reads `WorldData`
directly, as EP9 does), but all reads go through a **settlement knowledge query seam** so uncertainty
can be introduced later without rewriting need evaluation or Responses.

- Today: `aggregate_settlement_stock`, producer discovery, etc. read authoritative state directly.
- SA introduces a thin **`SettlementKnowledge`** accessor (a query interface, per ARCHITECTURE.md
  Principle 4) through which need evaluation asks "what stock do I believe I have," "what buildings do I
  know about," "what threats do I perceive." Initially it is a pass-through to `WorldData`.
- Future fog-of-war / observed / delayed knowledge is then a matter of swapping the accessor's backing
  (snapshots, staleness, per-faction visibility) — **no change to needs, arbiter, or Responses.**

This keeps the near-term implementation simple (perfect knowledge) while satisfying the requirement
that future uncertainty remain possible without redesign.

---

## 13. Player Settlements

Player settlements use the **same architecture and the same simulation model**. There must never be two
settlement simulations.

- Everything from Building Intent downward (policy → tasks → assignment → execution) is **identical**
  for player and AI settlements.
- The only difference is at the top: **player intent is just another directive source.** The player
  sets need targets/weights (stock targets, "prioritize defense") and may pin specific building
  policies. The player, a faction, and any future top-level AI all write directives through the same
  seam (§5) — there is no separate player planner.
- The existing `ControlSource` (`PlayerControlled` vs `AIControlled`) and `planner_managed` flag on
  `BuildingOperationPolicy` are exactly the building-level seam: planner-managed buildings are
  auto-driven; a player-reclaimed building (`PlayerControlled`) is skipped by producer discovery. SA
  preserves this flag and layers the directive seam above it.

A player "taking direct control" of a worker is just a `TaskPriority::PlayerAssigned` task or a direct
order that preempts automation — already supported.

---

## 14. Scalability

The architecture must support 1 → 10 → 100 settlements and thousands of workers / tens of thousands of
buildings without redesign. This is achieved by construction, not by later optimization:

- **All authority on `WorldData`** as plain indexed data (`BTreeMap`/`HashMap`), not per-object ECS
  entities or actors.
- **Localized lookups only.** Settlement membership index, `LogisticsEndpointIndex`, per-building task
  index — no layer performs a global scan of all inventories/buildings/units. Any new SA layer must
  add an index rather than iterate the world.
- **Interval + staggered + dirty reevaluation** (§10, §11). Cost scales with *change*, not with world
  size per frame.
- **Level-of-detail simulation** (ARCHITECTURE.md Scalability Rule): distant settlements should be
  able to run at coarser cadence or abstracted planning (existing-but-abstractly-simulated) without a
  different code path — same needs/arbiter, longer intervals, cheaper knowledge snapshots.
- **Derived state is rebuildable**, never the save bottleneck: graphs, diagnostics, need snapshots,
  membership, occupancy, and terrain assessments are all recomputed, keeping saves small and load
  deterministic.

---

## 15. Save / Load

Authoritative (persisted) SA data:

- Settlement records, treasuries, and ID counters (existing, scene v6+).
- **Need definitions and directive-adjusted targets/weights** (new, SA2/SA3) — the persistent intent.
- **Directive nudges** from player/faction/AI (new, SA3/SA9) and slow derived continuity state (active
  emergency flags, smoothed threat estimates) (new, SA2/SA7).
- `SettlementProductionPlanner` config: `stock_goals`, `category_priorities`, `local_retentions`,
  `replan_interval_ticks`, `last_plan_tick` (existing, scene v11/v12).
- `BuildingOperationPolicy` including `planner_managed` and `control_source` (existing, scene v9+).
- Expansion state once introduced (new, SA8).

**Not persisted (derived, recomputed on load):**

- Need *pressure values* (`NeedState` snapshot) and arbiter results.
- `ProductionGraph` and all `PlannerDiagnostics`.
- Task↔worker assignments beyond what's needed to resume in-flight work, and settlement building
  **membership** (re-derived by `reconcile_settlement_building_membership`).
- Occupancy, terrain assessments, logistics endpoint index.

**Rule: never persist temporary analysis.** Persist intent and slow state; recompute everything
derivable. This matches ADR-114 (diagnostics/graphs skipped) and the inventory/occupancy rebuild model
(ADR-094, ADR-086).

---

## 16. Factions

Settlements remain **fully independent and fully functional with no faction present.** The Faction
layer (SA9) is a coordination layer *above* settlements:

- It coordinates trade, military, expansion, and technology by **nudging need weights/targets and
  constraints** on member settlements (e.g. "raise weapon-production need," "do not expand north,"
  "supply grain to settlement B") — the same directive seam the player and future AI use (§5).
- It **never** owns buildings, tasks, or workers, and never bypasses a settlement's own pipeline.
- Faction identity/ownership already exists as data (`ownership.affiliation` on records, faction
  concepts in ARCHITECTURE.md). SA9 adds the coordination behavior, not the identity.

Because factions only write directives (the top of the pipeline), adding factions later requires **no
change to any lower SA layer** — exactly the "integrate without rewriting the simulation" requirement.

---

## 17. Dev Mode Tools

Debuggability is a first-class requirement (the EP9 planner already exposes diagnostics + Shift+P force
replan in the dev inspector). SA should provide:

- **Settlement overview:** identity, affiliation, population, member buildings, treasury.
- **Needs view:** current pressure per category, weight, target, Response mapping, trend, active
  emergencies.
- **Directives view:** active nudges, source (player / faction / authored), which need weights/targets
  they adjust.
- **Arbiter output:** ranked need pressures this cycle, selected need(s), applied Response(s),
  shortages / blocked responses. Production Response reuses `PlannerDiagnostics` shape for stock vs
  target, propagated demand, chosen producers.
- **Task generation view:** tasks by type/state/priority, per building and settlement-wide.
- **Building utilization:** operation lifecycle, blocked reasons, active worker count, efficiency.
- **Worker utilization:** idle vs assigned, current task, fall-through behavior.
- **Priority propagation trace:** for a chosen need/item, show Need pressure → arbiter rank →
  Response → building policy priority → task priority (the single most valuable debugging tool for an
  emergent system).

All dev tools **read** authoritative/derived state; they never become a control path that bypasses the
pipeline (except explicit dev overrides like force-replan, which must go through the normal apply
path).

---

## 18. Architectural Principles (SA)

1. **Single responsibility.** Each system reads data and writes one narrow slice of intent.
2. **Authoritative simulation.** `WorldData` is truth; ECS/UI/renderer are representations.
3. **Downward authority.** Layers command only the layer beneath; nothing commands upward.
4. **Data first.** Persistent concepts are data; derived analysis is recomputed, never persisted.
5. **Event-driven reevaluation.** Dirty flags + intervals; no settlement-wide per-frame scans.
6. **One arbiter, data-defined Responses.** A single generic weigh-and-respond loop; need-specific
   behavior is data (weight, target, Response mapping), never a new decision engine. Emergence from
   needs competing over time.
7. **Worker simplicity.** Workers know only task, reservations, inventory, location.
8. **Generic tasks.** Every worker action is a `Task`; new behavior integrates by generating tasks, not
   by commanding workers directly.
9. **Priority is the universal currency.** Need pressure → arbiter → Response → policy → tasks;
   emergencies and interrupts are priority reweighting, not new subsystems.
10. **Policy vs state separation.** Intent (`BuildingOperationPolicy`) is written from above; truth
    (`BuildingOperationState`) is written by execution. Never cross this line.
11. **One simulation model.** Player and AI settlements differ only at the directive seam (who nudges
    need weights/targets).
12. **Query seams for knowledge.** Need evaluation reads through a knowledge accessor so
    uncertainty/scale can change behind it without rewrites.
13. **Avoid special cases.** No per-resource, per-building, or per-emergency bespoke systems; solve
    categories. Adding a need is a data entry.
14. **Scalable by construction.** Indexes over scans; staggered intervals; LOD cadence.

---

## 19. Future Phase Map (SA1–SA12)

Recommended implementation order. Adjusted so **Task Generation and Worker Assignment come early** —
they are the current gap that blocks *any* autonomous behavior (EP9 can decide production but no
worker acts without manual assignment). Also adjusted so **there is never a "Strategic Goals Planner"
phase**: directives and the arbiter replace that layer.

| Phase | Name | Delivers | Depends on |
|---|---|---|---|
| **SA1** | Settlement Runtime | Authoritative `SettlementState` on `WorldData` (ADR-116): policies, need targets, modifiers, emergencies, planner lifecycle, kind — storage only. Dirty/rebuild seams; scene v13. No evaluation. | EP9 |
| **SA2** | Need Evaluation | ADR-117: authored `NeedDefinition` catalog; transient `NeedSnapshot` / `NeedEvaluationStore`; independent Current→Desired→Pressure; dirty+cadence step; never persists; no actions. First needs: Food, Construction, Housing, Defense, Research, Expansion, Luxury. | SA1 |
| **SA3** | Response Engine | ADR-118: authored `ResponseDefinition` catalog; discover/score transient `CandidateResponse`s from NeedSnapshots; capability gates; no execution. Settlement knows options, not actions. | SA2 |
| **SA4** | Response Arbiter | ADR-119: evaluate/rank/select CandidateResponses into multi-intent `SettlementIntent` (transient, no execution). | SA3 |
| **SA5** | Building Intent Propagation | ADR-120: SettlementIntent → `BuildingOperationPolicy` via capability discovery; policy-only; no tasks/construction. | SA4 |
| **SA5b** | Directives | Directive store (player/authored need weight/target nudges) adjusting need targets/weights into the same pipeline. | SA4 |
| **SA6** | Strategic Task Generation | ADR-121: SettlementIntent → authored templates → `TaskStore` (construct/repair/recruit/expand/clear). Merge/cancel; no worker assignment; never emits production/haul. At runtime, runs **after** SA9 so construct tasks attach to `ConstructionPlan` sites. | SA5, SA9 |
| **SA7** | Worker Assignment | ADR-122: Task marketplace — idle workers claim Available construct/operate tasks + open hauls via reservations; preemption + hysteresis; no settlement worker picks. | SA6 |
| **SA8** | Emergency Pressure & Priority Reweighting | ADR-123: authored EmergencyDefinitions; severity + hysteresis; reweight SA2/SA3; SA7 interrupt policy; no parallel AI. | SA2, SA7 |
| **SA9** | Construction Response | Capability-mapped construction intents → persistent `ConstructionPlan` + site reservation → SA6 construct tasks (ADR-124). | SA5 |
| **SA10** | Planning Scheduler | ADR-125: centralized when-to-think orchestration (dirty graph, budgets, cadence fallback, stagger). Stages keep evaluation logic; scheduler never plans. Downstream-only invalidation. | SA2–SA9 |
| **SA11** | Expansion / Growth | Growth need Responses for new buildings / sites / population-driven expansion. | SA8, SA9, SA10 |
| **SA12** | Faction Coordination | Cross-settlement directive injection for trade/military/expansion/tech. Same seam as the player. | SA5b, SA11 |

Ordering notes:

- **SA5 policy propagation** is the first world-influencing SA step; **SA6/SA7** then make production
  intent drive workers end-to-end before construction/expansion Responses expand the surface area.
- **Runtime construct path:** SA5 → **SA9 → SA6** → SA7 (ConstructionPlan before strategic construct tasks).
- **SA10** consolidates per-stage dirty+cadence polling into one scheduler; it does not change decisions.
- **SA4 is the arbiter (intent only), not a Goals planner.** There is no `SettlementGoal` decision engine.
- Each SA phase gets its **own ADR** and conforms to this document. If a phase discovers a needed
  deviation, it amends this ADR rather than silently diverging.

---

## 20. Deliverable Summary (answers to the design questions)

- **What is a Settlement?** A first-class, persistent `WorldData` object (treasury anchor +
  affiliation-scoped building membership) that owns weighted needs, one generic arbiter,
  data-defined Responses, directive nudges, and emergency state. It is the *strategic* layer.
- **What decisions does it make?** Which unmet need is most pressing right now — then apply that
  need's data-defined Response (produce / build / defend / expand / …) as building policy and
  directives. Never execution. Never a pile of situation-specific conditionals.
- **What decisions do Buildings make?** Only local operational ones: run the selected operation when
  enabled and inputs/terrain allow; report capabilities/status; express local demand via logistics.
- **What decisions do Workers make?** None strategic. Execute the assigned task; travel; carry
  inventory; interact. Simple.
- **Where does authority live?** On `WorldData`, flowing strictly downward: World → (Faction) →
  Settlement → Building → Task → Worker. Intent down, reports/reads up, no upward commands.
- **How do future AI systems integrate without rewriting the simulation?** By writing **directives**
  (need-weight/target nudges) and/or generating tasks — never by commanding workers or mutating
  runtime state directly. Player, faction, and any future lightweight top-level AI all use the same
  directive seam. New knowledge models plug in behind the knowledge seam. New behaviors are new need
  definitions (data), not new decision engines. The pipeline and the policy/state boundary stay fixed.

## References

- ADR-072 (settlement automation philosophy), ADR-107–ADR-114 (EP1–EP9 production stack)
- ADR-065 (fixed tick), ADR-085/086 (tasks, building persistence), ADR-087–094 (items/inventory)
- ADR-101–106 (Terrain Fields), ADR-093 (settlement treasuries)
- ARCHITECTURE.md (Principles 1–6, Scalability Rule, Settlements, EP9 section)
- DESIGN.md (Settlement Automation), ROADMAP.md (Future Systems: Settlements, Factions)
