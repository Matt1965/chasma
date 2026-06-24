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

# Future

Combat targeting, diplomacy matrix, AI ownership, multiplayer authority, economy
assignment — all consume `OwnerId` / `TeamId` / `Affiliation` on `UnitRecord`.

# Non-goals (O1)

No combat, diplomacy matrix, AI behavior, faction simulation, multiplayer sync.

# References

- ADR-027 Unit Data Ownership
- ADR-051 O1 implementation: `src/world/ownership/`
- P-UI1 Player HUD Foundation (consumer, not owner)
