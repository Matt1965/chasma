# ADR-130: Generic World Item Representation (IA0)

## Status

Accepted

## Context

Items already exist authoritatively in inventories and as world piles (`WorldItemPileRecord`, ADR-090). `ItemDefinition.render_key` was reserved at I1 but unused. Pile ECS entities existed with transform + name only — invisible in the world.

Testing, loot drops, mining output, and dev inventory tools (DV0) all need items to be physically visible without requiring authored art for every item type.

## Decision

Extend the existing **item pile presentation layer** (`src/item_piles/`) rather than introducing a parallel dropped-item system.

1. **Asset resolution** — `ItemSceneAssets` loads `assets/items/{render_key}.glb` when `ItemDefinition.render_key` is set (pattern: units/doodads).
2. **Automatic fallback** — When render key is unset or the GLB is not yet loaded, spawn a small configurable sphere (`ItemPilePresentationSettings`, `ItemPileFallbackAssets`). Unique-instance piles use a distinct tint.
3. **Upgrade path** — When a keyed asset finishes loading, sync replaces fallback with the authored scene on the next frame.
4. **Dev labels** — With `dev` feature + F12 enabled, billboard `Text2d` labels show pile contents (e.g. `Iron Ore x37`). Not player-facing UI.
5. **Authority unchanged** — Quantity and item identity remain on `WorldData.item_pile_store`; ECS is presentation only (ADR-090).

## Consequences

- Every item type is immediately visible in the world for testing.
- Authoring a render key + GLB upgrades presentation without code changes.
- Future loot/corpse/harvest systems continue to use `WorldItemPileRecord` and existing transfer APIs.
- Item asset sizing (ADR-097/127) is out of scope; piles use definition scale 1.0 until sized.

## References

- ADR-087 (item definitions, `render_key`)
- ADR-090 (world piles, presentation boundary)
- DV0 dev inventory tools
