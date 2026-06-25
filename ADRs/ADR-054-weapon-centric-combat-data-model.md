# ADR-054: Weapon-Centric Combat Data Model (C1)

# Status

Accepted (C1 — combat data foundation only)

# Context

Combat architecture (C0) established a weapon-centric model: attack behavior belongs on
weapon definitions; units describe body/stat context and reference a default weapon.
C1 implements catalog data and import only — no combat runtime behavior.

# Decision

## Ownership

| Concern | Owner |
|---------|-------|
| Damage, range, timing, hit mode, projectile/animation keys | `WeaponDefinition` |
| Body stats, locomotion, render key, default weapon reference | `UnitDefinition` |
| Active/equipped weapon at runtime | Deferred (`UnitRecord.active_weapon_id`, later phase) |
| Combat state on instances | Deferred (`WorldData` combat fields, later phase) |

## Types (`src/world/weapon/`)

- `WeaponDefinitionId` — stable catalog key
- `WeaponDefinition` — authoritative attack description
- `DamageType`, `HitMode`, `TargetFilter` — typed enums
- `WeaponCatalog` — Bevy `Resource`; deterministic iteration, lookup, duplicate rejection

## Unit integration

`UnitDefinition.default_weapon_id: WeaponDefinitionId` references the innate/default weapon.
Damage/range/cooldown are **not** duplicated on `UnitDefinition`.

Validation:

- Enabled units must reference an existing **enabled** weapon
- Missing default weapon fails import/validation (no silent fallback)

## Workbook (`Weapons` sheet)

Authoritative columns include Attacks Per Second (not cooldown). Internal helper:

`attack_cooldown_seconds() = 1.0 / attacks_per_second`

Windup is required and weapon-specific. Recovery is stored separately.

`projectile_key` and `animation_key` are stored but unused in C1.
`stat_scaling` is stored/reserved but ignored in C1.

## Range interpretation

`range_meters` is stored on `WeaponDefinition`. Edge-to-edge interpretation using collision
radii is reserved for C4 combat behavior.

## Innate attacks

Natural attacks (fists, bite, claws) are normal `WeaponDefinition` rows — not special-case
unit fields.

## Dev startup

1. Load `Weapons` sheet → `WeaponCatalog` (starter catalog on import failure)
2. Load `Units` sheet → validate `default_weapon_id` against weapon catalog
3. Unit import referencing missing/disabled weapon → clear error (no fallback)

Production builds use `init_resource::<WeaponCatalog>()` with starter defaults outside dev
import paths.

## Starter fixtures

Weapons: `weapon_fists`, `weapon_wolf_bite`, `weapon_claws`

Units: wolf → bite, bandit → fists, deer → claws

# Non-goals (C1)

No attack orders, targeting, HP/vitals, damage resolution, death, projectiles, animation
playback, AI, movement changes, combat UI, `UnitRecord` combat fields, or active weapon
switching.

# Future

- C2+: vitals, attack orders, damage pipeline
- C4: edge-to-edge range using collision radii
- Equipment / `active_weapon_id` on `UnitRecord`

# References

- C0 combat architecture plan
- ADR-027 Unit Data Ownership
- ADR-051 Unit Ownership and Affiliation
- Implementation: `src/world/weapon/`, `src/data_import/weapon/`
