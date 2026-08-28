# Architecture Decision Records

ADRs document **accepted technical decisions**. Game design direction lives in
[DESIGN.md](../DESIGN.md).

## Reading order

1. [DESIGN.md](../DESIGN.md) — game design goals (draft unless cited by ADR)
2. [ARCHITECTURE.md](../ARCHITECTURE.md) — system structure and principles
3. [ROADMAP.md](../ROADMAP.md) — implementation status

## Design direction ADRs (069+)

| ADR | Topic |
|-----|-------|
| [074](ADR-074-runtime-unit-animation-foundation.md) | Runtime unit animation (A1–A3 foundation) |
| [075](ADR-075-animation-layering.md) | Animation layering (A4) |
| [076](ADR-076-advanced-locomotion-animation-polish.md) | Locomotion polish (A5) |
| [077](ADR-077-animation-scaling-lod-and-validation.md) | Scaling, LOD, validation (A6); A1 audit fixes |
| [069](ADR-069-combat-design-philosophy.md) | Combat philosophy (WC3 tactical, responsiveness, collision, downed state) |
| [070](ADR-070-progression-and-attributes.md) | Use-based skills, attributes, crits |
| [071](ADR-071-creature-ai-architecture.md) | Species → behavior → personality → state → decision |
| [072](ADR-072-settlement-automation-and-production.md) | Professions, tasks, building requests |
| [073](ADR-073-inventory-and-equipment.md) | Grid inventory + equipment slots |
| [087](ADR-087-item-definitions-and-inventory-profiles.md) | Item definitions, categories, inventory profiles (I1) |
| [088](ADR-088-authoritative-inventory-grid-and-item-identity.md) | Authoritative inventory grid and item identity (I2) |
| [089](ADR-089-unit-inventories-corpse-ownership-and-item-survival.md) | Unit inventories, corpse ownership, item survival (I3) |
| [090](ADR-090-item-transfers-world-piles-dropping-and-looting.md) | Item transfers, world piles, drop/pickup/loot (I4) |
| [091](ADR-091-building-containers-and-inventory-ownership.md) | Building containers, access, destruction spill (I5) |
| [092](ADR-092-player-inventory-ui-and-transfer-interaction.md) | Player inventory UI, drag/drop, transfers (I6) |
| [093](ADR-093-settlement-treasuries-and-physical-gold.md) | Settlement treasuries, physical gold deposits (I7) |
| [094](ADR-094-inventory-persistence-validation-and-audit.md) | Inventory persistence, validation, audit (I8) |
| [095](ADR-095-building-runtime-asset-and-scene-integration.md) | Building runtime GLB assets and scene integration (BA1) |
| [096](ADR-096-building-placement-transform-and-dev-spawn-policy.md) | Placement transform, freeform anchoring, dev Complete spawn (BP-CLEANUP) |
| [097](ADR-097-metric-asset-sizing-and-authoring-transform-foundations.md) | Metric asset sizing foundations (DT1) |
| [126](ADR-126-asset-transform-standardization.md) | Asset transform standardization (AT0 design) |
| [127](ADR-127-asset-transform-catalog-authority.md) | Catalog sizing authority (AT1) |
| [128](ADR-128-asset-transform-composition.md) | Runtime transform composition (AT2) |
| [129](ADR-129-collision-gameplay-metric-sync.md) | Collision & gameplay metric sync (AT3) |
| [130](ADR-130-generic-world-item-representation.md) | Generic world item representation (IA0) |

## Combat implementation chain

054 → 055 → 056 → 057 → 058 → 059 → 060 → 062 (see individual files for C-phase scope)

## Client / simulation

038–041 (intent, commands), 064–065 (fixed tick), 066 (movement outcomes), 068 (environment)

## Environment / rendering

| ADR | Topic |
|-----|-------|
| [026](ADR-026-skybox-foundation.md) | Environment rendering layer (procedural sky foundation) |
| [052](ADR-052-time-of-day-visual-environment-system.md) | Time-of-day visual environment |
| [053](ADR-053-water-rendering-foundation.md) | Water rendering foundation |
| [068](ADR-068-environment-singleton-and-input-ownership.md) | Environment singleton and input ownership |
| [131](ADR-131-volumetric-cloud-rendering-architecture.md) | Volumetric cloud rendering architecture |

## Terrain / world

001–013 (coordinates, terrain), 031–032 (obstacles, navigation), 067 (validation)

## Settlement AI

| ADR | Topic |
|-----|-------|
| [072](ADR-072-settlement-automation-and-production.md) | Automation philosophy (professions, tasks) |
| [093](ADR-093-settlement-treasuries-and-physical-gold.md) | Treasuries (I7) |
| [114](ADR-114-settlement-production-planner.md) | EP9 production planner (now a service; 2026-08-28 amendment) |
| [115](ADR-115-settlement-ai-architecture.md) | SA foundation; 2026-08-28 status + deferred issues |
| [116](ADR-116-settlement-runtime-state.md) | SA1 SettlementState |
| [117](ADR-117-need-evaluation-runtime.md) | SA2 needs (CategoryStock + member food demand amendment) |
| [118](ADR-118-response-engine.md) | SA3 responses (quality-only scoring amendment) |
| [119](ADR-119-settlement-response-arbiter.md) | SA4 arbiter (pairing + weight amendment) |
| [120](ADR-120-building-intent-propagation.md) | SA5 policy (sole AI writer amendment) |
| [121](ADR-121-strategic-task-generation.md) | SA6 strategic tasks |
| [122](ADR-122-worker-assignment-marketplace.md) | SA7 marketplace (ordinary-work eligibility amendment) |
| [123](ADR-123-emergency-pressure-reweighting.md) | SA8 emergencies |
| [124](ADR-124-strategic-construction-planning.md) | SA9 construction planning |
| [125](ADR-125-planning-scheduler.md) | SA10 scheduler |
| [133](ADR-133-settlement-identity-membership-and-anchor.md) | Identity, membership, anchor, boundary |
| [134](ADR-134-individual-self-maintenance-and-hunger.md) | Individual hunger and self-maintenance |

## Relationships

[132](ADR-132-relationship-and-reputation-architecture.md) — directional relationships and reputation
(architecture; Map phases 1–7 implemented).
