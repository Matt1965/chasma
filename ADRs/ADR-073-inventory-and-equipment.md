# ADR-073: Inventory and Equipment

## Status

Accepted (design direction — equipment seam only)

## Context

ADR-054 reserved `active_weapon_id` on `UnitRecord` for equipped weapons. Carried items,
grid sizing, and logistics gameplay were undocumented.

Full design narrative: [DESIGN.md](../DESIGN.md#inventory-and-equipment).

## Decision

### Dual model

| System | Model | Purpose |
|--------|-------|---------|
| **Equipment** | Traditional slots (weapon, armor, accessories) | Combat loadout clarity |
| **Inventory** | Kenshi-style **grid** with item footprints | Logistics, looting, caravan packing |

Physical item size matters. Equipment and grid inventory coexist — worn gear is not
necessarily stored in the grid.

### Quality-of-life (planned)

Reduce busywork without removing strategy:

- Auto-sort
- Smart item placement suggestions
- Automatic stack merging
- AI-friendly packing for haulers

### Data ownership (when implemented)

- Item instances and grid state on authoritative `WorldData` (ADR-027, Principle 6)
- Item **definitions** in catalogs (parallel to weapons, doodads)
- ECS renders equipped meshes; does not own item truth

### Combat integration

- `WeaponDefinition` remains attack authority (ADR-054)
- Equipped weapon selects `active_weapon_id`; default weapon is fallback
- Looting downed units (ADR-069) consumes grid inventory when downed state exists

## Non-goals (current phase)

- Grid UI, drag-drop, container nesting rules
- Item durability, crafting, or economy pricing (CHR attribute, ADR-070)
- Weight/speed penalties until carry-weight uses STR

## Consequences

- First implementation likely: equipment slots + single-container grid before multi-building storage
- Settlement haul tasks (ADR-072) depend on inventory move operations
- No Kenshi-style nested body loot until injury/downed systems exist

## References

- [DESIGN.md](../DESIGN.md)
- ADR-054, ADR-027, ADR-069, ADR-072
