# ADR-051: Unit Ownership and Affiliation (O1)

# Status

Accepted (O1 — runtime ownership foundation)

# Context

ADR-027 established unit instances on `WorldData` with catalog `faction_tag` as design
metadata only. Player HUD (P-UI1), selection, commands, and future combat/AI/economy
need authoritative **runtime ownership** — who controls this unit **right now**.

# Decision

## Definition vs runtime

| Concern | Owner |
|---------|-------|
| Type stats, Excel `Faction` column | `UnitDefinition.faction_tag` (metadata) |
| Who controls instance | `UnitRecord.owner_id`, `team_id`, `affiliation` |

**Never** derive runtime ownership from `faction_tag`.

## Types (`src/world/ownership/`)

- `OwnerId(u64)` — direct controller
- `TeamId(u64)` — ally/enemy grouping (future combat/diplomacy)
- `Affiliation` — `Player`, `Neutral`, `Hostile`, `Wildlife`, `Dev`, `Unknown`
- `UnitOwnership` — spawn bundle for authoring API

## Default player ids

- `DEFAULT_PLAYER_OWNER_ID = OwnerId(1)`
- `DEFAULT_PLAYER_TEAM_ID = TeamId(1)`
- Neutral units: `owner_id = None`, `team_id = None`, `affiliation = Neutral`

## Authoring

- `create_unit_with_ownership(..., ownership)` — explicit assignment
- `create_unit(...)` — safe defaults from `UnitSource` only (not faction tag)

## Controllability

Player-controllable when:

- `affiliation == Player`
- `owner_id == Some(DEFAULT_PLAYER_OWNER_ID)`

## Selection / commands

- Picking and box select filter non-selectable units (gameplay)
- Dev mode enabled → inspect/select any unit (dev override)
- Move/Stop/Hold issue only to commandable units; selection pruned on dispatch

## Query helpers

`player_units`, `units_by_owner`, `units_by_affiliation`, `is_player_controllable`

## HUD integration

Squad panel uses `player_units()` instead of `UnitSource` filtering.

# Relationship identity vs ownership (2026-08-24, accepted direction — not implemented)

[ADR-132](ADR-132-relationship-and-reputation-architecture.md) introduces **relationship
identity facets** (Faction, Species, Individual). These are a separate axis from ownership and
do **not** replace `OwnerId` / `TeamId` / `Affiliation`.

| Concern | Authority | Derived from authored unit data? |
|---------|-----------|----------------------------------|
| Runtime ownership (`owner_id`, `team_id`, `affiliation`) | `UnitRecord` | **Never** — rule above is unchanged |
| Relationship identity (faction, species) | `UnitRecord` (runtime-authoritative) | **Spawn-time default only** |
| Design-time label (`faction_tag`) | `UnitDefinition` | Content metadata, gameplay-inert |

A definition-supplied relationship **default** is not ownership derivation, provided all three hold:

1. The seeded value lands in dedicated relationship-identity state on `UnitRecord` — never in
   `owner_id`, `team_id`, or `affiliation`. No control or controllability decision reads it.
2. Nothing reads the *definition* for runtime membership decisions. The record is the sole
   authority, so faction membership can change at runtime later.
3. The key is the Factions catalog's stable slug, not the display-label `faction_tag`.

This mirrors the existing precedent: `default_ownership_for_source` already derives spawn-time
ownership defaults from `UnitSource`. This ADR prohibits **ongoing derivation** of ownership from
content metadata, not spawn defaults — and relationship identity is not ownership.

`Affiliation` is **not** extended to express relationships. Variants such as `HostileAnimal` or
`NeutralAnimal` are explicitly forbidden: `Affiliation` equality is load-bearing for settlement
building membership and worker task eligibility, so new variants would silently re-partition
unrelated systems.

Phase 6 removed `Affiliation`'s role as the **AI proactive hostility authority** (ADR-056, ADR-062,
ADR-132 Phase 6–7). Reactive retaliation and ownership semantics are unchanged; `Affiliation`
remains load-bearing for settlement membership and worker eligibility.

# Future

Combat targeting, AI ownership, multiplayer authority, economy assignment — all consume
`OwnerId` / `TeamId` / `Affiliation` on `UnitRecord`. Graded social relationships are owned by
ADR-132, not by this ADR.

# Non-goals (O1)

No combat, diplomacy matrix, AI behavior, faction simulation, multiplayer sync.

# References

- ADR-027 Unit Data Ownership
- ADR-051 O1 implementation: `src/world/ownership/`
- ADR-132 Relationship and Reputation Architecture (relationship identity boundary)
- P-UI1 Player HUD Foundation (consumer, not owner)
