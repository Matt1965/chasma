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

## Combat implementation chain

054 → 055 → 056 → 057 → 058 → 059 → 060 → 062 (see individual files for C-phase scope)

## Client / simulation

038–041 (intent, commands), 064–065 (fixed tick), 066 (movement outcomes), 068 (environment)

## Terrain / world

001–013 (coordinates, terrain), 031–032 (obstacles, navigation), 067 (validation)
