# Item and Inventory Profile Authoring (I1)

## Workbook sheets

Add these sheets to the dev design workbook (`data/design/chasma_design.xlsx`):

### Item Categories

| Column | Required | Notes |
|--------|----------|-------|
| Category ID | yes | Stable machine id, e.g. `currency`, `raw_material` |
| Name | yes | Display name |
| Enabled | yes | `Y` / `N` |
| Description | no | |
| Sort Priority | no | Integer sort hint |

### Items

| Column | Required | Notes |
|--------|----------|-------|
| Item ID | yes | Stable machine id |
| Name | yes | |
| Category | yes | Must match an enabled category id |
| Width | yes | Grid footprint width (> 0, ≤ 64) |
| Height | yes | Grid footprint height (> 0, ≤ 64) |
| Stackable | yes | `Y` / `N` |
| Max Stack | yes | ≥ 1 when stackable; must be 1 for unique items |
| Mass Grams | yes | Integer grams per unit (> 0) |
| Enabled | yes | |
| Description | no | |
| Render Key | no | World mesh key — `items/{key}.glb` when set; generic sphere fallback when unset (IA0) |
| Icon Key | no | Future UI icon |
| Base Value | no | Gold value baseline (defaults to 1) |
| Tags | no | Comma-separated; normalized to lowercase sorted tags |
| Unique Instance Required | no | `Y` forces non-stackable unique item rules |

### Inventory Profiles

| Column | Required | Notes |
|--------|----------|-------|
| Inventory Profile ID | yes | Stable machine id |
| Name | yes | |
| Grid Width | yes | Container width (> 0, ≤ 64) |
| Grid Height | yes | Container height (> 0, ≤ 64) |
| Enabled | yes | |
| Reference Weight Grams | no | Soft encumbrance hint — not a hard capacity |
| Global Stack Cap | no | Optional per-container stack ceiling (≥ 1) |
| Access Type | no | `OwnerOnly`, `PartyShared`, `BuildingStorage`, `CorpseLoot` |

### Unit / Building optional column

| Column | Required | Notes |
|--------|----------|-------|
| Inventory Profile ID | no | Blank = no inventory on this definition |

## Physical gold

Author a stackable currency item (starter fallback id: `gold`):

- Category: `currency`
- Small footprint (e.g. 1×1)
- Integer mass and `max_stack` from data
- `base_value_gold` typically 1 per coin

Treasury balances remain a separate future system.

## Validation rules

- Stackable: `stackable = true`, `max_stack >= 1`, `unique_instance_required = false`
- Unique: `stackable = false`, `max_stack = 1`, `unique_instance_required = true`
- Contradictory combinations are rejected at import
- Referenced inventory profiles must exist and be enabled

## Dev browser

F12 → **Items** tab. Press **I** for item definitions, **P** for inventory profiles, **H** for the inventory harness (I2).

- **I / P** — read-only catalog browse (I1)
- **H** — authoritative detached inventory test harness (ADR-088 I2): create inventory, add stacks/uniques, split, merge, auto-sort, validate, delete

## Runtime inventory (I2)

Authoritative `InventoryStore` and `ItemInstanceStore` live on `WorldData`. See [ADR-088](../ADRs/ADR-088-authoritative-inventory-grid-and-item-identity.md).

## World item presentation (IA0)

Every item can exist as a **world pile** (`WorldItemPileRecord` on `WorldData`). Presentation is derived — quantity and item type remain authoritative on the world record, not on ECS.

| `ItemDefinition.render_key` | Runtime behavior |
|-----------------------------|------------------|
| Set + GLB loaded | Renders `assets/items/{key}.glb` |
| Unset or loading | Renders configurable generic sphere fallback |

Fallback settings live in `ItemPilePresentationSettings` (`fallback_sphere_radius`, colors, dev label offset).

In **dev mode** (F12), floating labels show pile contents (e.g. `Iron Ore x37`, `Unknown Item`) above each visible pile.

## Generated assets

Dev import exports:

- `assets/items/catalog.ron`
- `assets/inventory/profiles.ron`

Production builds do not depend on Excel at runtime.
