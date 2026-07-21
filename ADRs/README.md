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

## Combat implementation chain

054 → 055 → 056 → 057 → 058 → 059 → 060 → 062 (see individual files for C-phase scope)

## Client / simulation

038–041 (intent, commands), 064–065 (fixed tick), 066 (movement outcomes), 068 (environment)

## Terrain / world

001–013 (coordinates, terrain), 031–032 (obstacles, navigation), 067 (validation)
